//! DOM Serializer - Convert DOM tree to LLM-friendly format
//!
//! This module handles:
//! - Filtering visible/interactive elements
//! - Paint order optimization
//! - Generating compact representation for LLMs
//! - XPath generation for element identification

use crate::arena::DomArena;
use crate::error::Result;
use crate::types::*;

/// Serializer configuration
#[derive(Debug, Clone)]
pub struct SerializerConfig {
    pub paint_order_filtering: bool,
    pub include_attributes: Vec<String>,
    pub max_text_length: usize,
}

impl Default for SerializerConfig {
    fn default() -> Self {
        Self {
            paint_order_filtering: true,
            include_attributes: DEFAULT_INCLUDE_ATTRIBUTES
                .iter()
                .map(|s| s.to_string())
                .collect(),
            max_text_length: 200,
        }
    }
}

/// DOM Tree Serializer
pub struct DomSerializer {
    config: SerializerConfig,
}

impl DomSerializer {
    pub fn new() -> Self {
        Self::with_config(SerializerConfig::default())
    }

    pub fn with_config(config: SerializerConfig) -> Self {
        Self { config }
    }

    /// Serialize DOM tree to string for LLM consumption
    pub fn serialize(&self, arena: &DomArena) -> Result<String> {
        let mut output = String::with_capacity(4096);

        if let Some(root_id) = arena.root_id() {
            self.serialize_node(arena, root_id, 0, &mut output)?;
        }

        Ok(output)
    }

    /// Serialize a single node recursively
    fn serialize_node(
        &self,
        arena: &DomArena,
        node_id: NodeId,
        depth: usize,
        output: &mut String,
    ) -> Result<()> {
        let node = arena.get(node_id)?;

        // Skip invisible nodes (but allow None visibility to pass through)
        if node.is_visible == Some(false) {
            return Ok(());
        }

        // Add indentation
        let indent = "  ".repeat(depth);

        match node.node_type {
            NodeType::Element => {
                // Format: <tag id="123" class="foo">
                output.push_str(&indent);
                output.push('<');
                output.push_str(&node.node_name);

                // Add relevant attributes
                for attr_name in &self.config.include_attributes {
                    if let Some(attr_value) = node.attr(attr_name) {
                        output.push_str(&format!(" {}=\"{}\"", attr_name, attr_value));
                    }
                }

                output.push_str(">\n");

                // Serialize children
                for &child_id in &node.children_ids {
                    self.serialize_node(arena, child_id, depth + 1, output)?;
                }

                // Closing tag
                output.push_str(&indent);
                output.push_str("</");
                output.push_str(&node.node_name);
                output.push_str(">\n");
            }
            NodeType::Text => {
                let text = node.node_value.trim();
                if !text.is_empty() {
                    output.push_str(&indent);
                    output.push_str(text);
                    output.push('\n');
                }
            }
            NodeType::Document => {
                // For document nodes, just serialize children
                for &child_id in &node.children_ids {
                    self.serialize_node(arena, child_id, depth, output)?;
                }
            }
            _ => {
                // Skip other node types for now
            }
        }

        Ok(())
    }

    /// Generate XPath for a node
    pub fn generate_xpath(&self, arena: &DomArena, node_id: NodeId) -> Result<String> {
        let mut path_parts = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            let node = arena.get(id)?;

            if node.node_type == NodeType::Element {
                // Get position among siblings with same tag name
                let position = if let Some(parent_id) = node.parent_id {
                    let parent = arena.get(parent_id)?;
                    parent
                        .children_ids
                        .iter()
                        .filter_map(|&child_id| arena.get(child_id).ok())
                        .filter(|child| {
                            child.node_type == NodeType::Element
                                && child.node_name == node.node_name
                        })
                        .position(|child| child.node_id == node.node_id)
                        .map(|p| p + 1) // XPath is 1-indexed
                        .unwrap_or(1)
                } else {
                    1
                };

                path_parts.push(format!("{}[{}]", node.node_name.to_lowercase(), position));
            }

            current_id = node.parent_id;
        }

        path_parts.reverse();
        Ok(format!("/{}", path_parts.join("/")))
    }

    /// Filter elements by paint order (optimization)
    pub fn filter_by_paint_order(&self, arena: &DomArena) -> Result<Vec<NodeId>> {
        // TODO: Implement paint order filtering
        // For now, return all visible elements
        Ok(arena.find_visible())
    }
}

impl Default for DomSerializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::DomService;

    #[test]
    fn test_serialize_simple_dom() {
        let cdp_json = serde_json::json!({
            "root": {
                "nodeId": 1,
                "backendNodeId": 1,
                "nodeType": 9,
                "nodeName": "#document",
                "nodeValue": "",
                "children": [{
                    "nodeId": 2,
                    "backendNodeId": 2,
                    "nodeType": 1,
                    "nodeName": "HTML",
                    "nodeValue": "",
                    "attributes": [],
                    "children": [{
                        "nodeId": 3,
                        "backendNodeId": 3,
                        "nodeType": 3,
                        "nodeName": "#text",
                        "nodeValue": "Hello",
                        "attributes": []
                    }]
                }]
            }
        });

        let mut service = DomService::new();
        service.parse_cdp_dom_tree(&cdp_json).unwrap();

        // Set visibility for all nodes (required for serialization)
        for node_id in service.arena().node_ids().collect::<Vec<_>>() {
            if let Ok(node) = service.arena_mut().get_mut(node_id) {
                node.is_visible = Some(true);
            }
        }

        let serializer = DomSerializer::new();
        let output = serializer.serialize(service.arena()).unwrap();

        assert!(
            output.contains("HTML"),
            "Output should contain HTML tag. Got: {}",
            output
        );
    }
}
