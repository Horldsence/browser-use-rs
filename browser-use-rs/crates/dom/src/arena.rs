//! Arena-based DOM tree storage
//!
//! ## Linus Philosophy Applied
//!
//! "Bad programmers worry about the code. Good programmers worry about
//! data structures and their relationships."
//!
//! This arena eliminates:
//! - Rc/Arc overhead (16 bytes per pointer)
//! - Recursive function calls (stack overflow risk)
//! - Cache misses (nodes stored sequentially)
//! - GC pressure (single Vec allocation)
//!
//! ## Memory Layout
//!
//! ```text
//! Arena: Vec<DomNode>
//!        [Node0][Node1][Node2]...
//!         ↑ 4-byte index, not 8-byte pointer
//! ```

use crate::error::{DomError, Result};
use crate::types::{DomNode, NodeId, NodeType};
use ahash::AHashMap;

/// Arena allocator for DOM nodes
///
/// Design:
/// - Single Vec<DomNode> for sequential allocation
/// - HashMap for backend_node_id → NodeId lookup (CDP uses backend IDs)
/// - No Rc/Arc: use indices everywhere
#[derive(Debug)]
pub struct DomArena {
    /// All nodes stored sequentially (cache-friendly)
    nodes: Vec<DomNode>,

    /// Backend node ID → NodeId lookup (for CDP integration)
    backend_id_map: AHashMap<u32, NodeId>,

    /// Root node ID (if set)
    root_id: Option<NodeId>,
}

impl DomArena {
    /// Create a new empty arena
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(1024), // Pre-allocate for typical page
            backend_id_map: AHashMap::with_capacity(1024),
            root_id: None,
        }
    }

    /// Create arena with specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            backend_id_map: AHashMap::with_capacity(capacity),
            root_id: None,
        }
    }

    /// Add a node to the arena, returns its ID
    pub fn add_node(&mut self, node: DomNode) -> NodeId {
        let node_id = self.nodes.len() as NodeId;
        self.backend_id_map.insert(node.backend_node_id, node_id);
        self.nodes.push(node);
        node_id
    }

    /// Get node by ID (immutable)
    pub fn get(&self, node_id: NodeId) -> Result<&DomNode> {
        self.nodes
            .get(node_id as usize)
            .ok_or(DomError::NodeNotFound(node_id))
    }

    /// Get node by ID (mutable)
    pub fn get_mut(&mut self, node_id: NodeId) -> Result<&mut DomNode> {
        self.nodes
            .get_mut(node_id as usize)
            .ok_or(DomError::NodeNotFound(node_id))
    }

    /// Get node by backend node ID (from CDP)
    pub fn get_by_backend_id(&self, backend_id: u32) -> Result<&DomNode> {
        let node_id = self
            .backend_id_map
            .get(&backend_id)
            .ok_or(DomError::NodeNotFound(backend_id))?;
        self.get(*node_id)
    }

    /// Get node ID by backend node ID
    pub fn get_node_id_by_backend(&self, backend_id: u32) -> Option<NodeId> {
        self.backend_id_map.get(&backend_id).copied()
    }

    /// Set root node
    pub fn set_root(&mut self, node_id: NodeId) -> Result<()> {
        // Verify node exists
        self.get(node_id)?;
        self.root_id = Some(node_id);
        Ok(())
    }

    /// Get root node ID
    pub fn root_id(&self) -> Option<NodeId> {
        self.root_id
    }

    /// Get root node
    pub fn root(&self) -> Result<&DomNode> {
        let root_id = self
            .root_id
            .ok_or_else(|| DomError::CdpError("No root node set".to_string()))?;
        self.get(root_id)
    }

    /// Total number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if arena is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterator over all nodes
    pub fn iter(&self) -> impl Iterator<Item = &DomNode> {
        self.nodes.iter()
    }

    /// Iterator over all node IDs
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.nodes.len()).map(|i| i as NodeId)
    }

    /// Get children of a node
    pub fn children(&self, node_id: NodeId) -> Result<Vec<&DomNode>> {
        let node = self.get(node_id)?;
        node.children_ids
            .iter()
            .map(|&child_id| self.get(child_id))
            .collect()
    }

    /// Get parent of a node
    pub fn parent(&self, node_id: NodeId) -> Result<Option<&DomNode>> {
        let node = self.get(node_id)?;
        match node.parent_id {
            Some(parent_id) => Ok(Some(self.get(parent_id)?)),
            None => Ok(None),
        }
    }

    /// Traverse tree depth-first (iterative, no recursion)
    ///
    /// This is the "good taste" version - no special cases for leaf nodes
    pub fn traverse_df<F>(&self, start_id: NodeId, mut visit: F) -> Result<()>
    where
        F: FnMut(&DomNode) -> Result<()>,
    {
        let mut stack = vec![start_id];

        while let Some(node_id) = stack.pop() {
            let node = self.get(node_id)?;
            visit(node)?;

            // Push children in reverse order (so they're visited left-to-right)
            for &child_id in node.children_ids.iter().rev() {
                stack.push(child_id);
            }
        }

        Ok(())
    }

    /// Traverse tree breadth-first
    pub fn traverse_bf<F>(&self, start_id: NodeId, mut visit: F) -> Result<()>
    where
        F: FnMut(&DomNode) -> Result<()>,
    {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start_id);

        while let Some(node_id) = queue.pop_front() {
            let node = self.get(node_id)?;
            visit(node)?;

            for &child_id in node.children_ids.iter() {
                queue.push_back(child_id);
            }
        }

        Ok(())
    }

    /// Find nodes matching predicate
    pub fn find<F>(&self, predicate: F) -> Vec<NodeId>
    where
        F: Fn(&DomNode) -> bool,
    {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| {
                if predicate(node) {
                    Some(idx as NodeId)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find first node matching predicate
    pub fn find_one<F>(&self, predicate: F) -> Option<NodeId>
    where
        F: Fn(&DomNode) -> bool,
    {
        self.nodes.iter().enumerate().find_map(|(idx, node)| {
            if predicate(node) {
                Some(idx as NodeId)
            } else {
                None
            }
        })
    }

    /// Find all elements by tag name
    pub fn find_by_tag(&self, tag: &str) -> Vec<NodeId> {
        let tag_upper = tag.to_uppercase();
        self.find(|node| {
            node.node_type == NodeType::Element && node.node_name.eq_ignore_ascii_case(&tag_upper)
        })
    }

    /// Find element by ID attribute
    pub fn find_by_id(&self, id: &str) -> Option<NodeId> {
        self.find_one(|node| node.node_type == NodeType::Element && node.attr("id") == Some(id))
    }

    /// Find all visible elements
    pub fn find_visible(&self) -> Vec<NodeId> {
        self.find(|node| node.is_visible == Some(true))
    }

    /// Find all clickable elements
    pub fn find_clickable(&self) -> Vec<NodeId> {
        self.find(|node| node.is_clickable())
    }

    /// Clear arena (reuse allocation)
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.backend_id_map.clear();
        self.root_id = None;
    }
}

impl Default for DomArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_basic() {
        let mut arena = DomArena::new();

        let node = DomNode::new(
            0,
            100,
            NodeType::Element,
            "div".to_string(),
            "target1".to_string(),
        );

        let id = arena.add_node(node);
        assert_eq!(id, 0);

        let retrieved = arena.get(id).unwrap();
        assert_eq!(retrieved.node_name, "div");
        assert_eq!(retrieved.backend_node_id, 100);
    }

    #[test]
    fn test_backend_lookup() {
        let mut arena = DomArena::new();

        let node = DomNode::new(
            0,
            100,
            NodeType::Element,
            "div".to_string(),
            "target1".to_string(),
        );

        arena.add_node(node);

        let found = arena.get_by_backend_id(100).unwrap();
        assert_eq!(found.node_name, "div");
    }

    #[test]
    fn test_traverse_df() {
        let mut arena = DomArena::new();

        // Create tree: root -> [child1, child2]
        let mut root = DomNode::new(
            0,
            100,
            NodeType::Element,
            "div".to_string(),
            "target1".to_string(),
        );

        let child1 = DomNode::new(
            1,
            101,
            NodeType::Element,
            "span".to_string(),
            "target1".to_string(),
        );

        let child2 = DomNode::new(
            2,
            102,
            NodeType::Element,
            "span".to_string(),
            "target1".to_string(),
        );

        let id1 = arena.add_node(child1);
        let id2 = arena.add_node(child2);

        root.children_ids.push(id1);
        root.children_ids.push(id2);

        let root_id = arena.add_node(root);

        let mut visited = Vec::new();
        arena
            .traverse_df(root_id, |node| {
                visited.push(node.node_name.clone());
                Ok(())
            })
            .unwrap();

        assert_eq!(visited, vec!["div", "span", "span"]);
    }
}
