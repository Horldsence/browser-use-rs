//! DOM Service - Main entry point for DOM operations
//!
//! Equivalent to Python's `DomService` class in browser_use/dom/service.py
//!
//! This handles:
//! - CDP integration (parsing CDP JSON responses)
//! - DOM tree construction from CDP data
//! - Accessibility tree integration
//! - Snapshot data merging
//! - Visibility calculations

use crate::arena::DomArena;
use crate::error::{DomError, Result};
use crate::types::*;
use crate::utils;
use serde_json::Value;
use std::collections::HashMap;

/// Configuration for DOM service
#[derive(Debug, Clone)]
pub struct DomServiceConfig {
    pub cross_origin_iframes: bool,
    pub paint_order_filtering: bool,
    pub max_iframes: usize,
    pub max_iframe_depth: usize,
}

impl Default for DomServiceConfig {
    fn default() -> Self {
        Self {
            cross_origin_iframes: false,
            paint_order_filtering: true,
            max_iframes: 100,
            max_iframe_depth: 5,
        }
    }
}

/// Main DOM service
pub struct DomService {
    config: DomServiceConfig,
    arena: DomArena,
}

impl DomService {
    /// Create new DOM service with default config
    pub fn new() -> Self {
        Self::with_config(DomServiceConfig::default())
    }

    /// Create DOM service with custom config
    pub fn with_config(config: DomServiceConfig) -> Self {
        Self {
            config,
            arena: DomArena::new(),
        }
    }

    /// Get reference to internal arena
    pub fn arena(&self) -> &DomArena {
        &self.arena
    }

    /// Get mutable reference to internal arena
    pub fn arena_mut(&mut self) -> &mut DomArena {
        &mut self.arena
    }

    /// Parse CDP DOM tree response and build arena
    ///
    /// This is the main entry point that takes CDP JSON and constructs
    /// the internal DOM tree representation.
    ///
    /// Input format matches CDP's DOM.getDocument response:
    /// ```json
    /// {
    ///   "root": {
    ///     "nodeId": 1,
    ///     "backendNodeId": 1,
    ///     "nodeType": 9,
    ///     "nodeName": "#document",
    ///     "children": [...]
    ///   }
    /// }
    /// ```
    pub fn parse_cdp_dom_tree(&mut self, cdp_response: &Value) -> Result<NodeId> {
        let root = cdp_response
            .get("root")
            .ok_or_else(|| DomError::CdpError("Missing 'root' in CDP response".to_string()))?;

        self.arena.clear();
        let root_id = self.parse_node(root, None, &TargetId::from("default"))?;
        self.arena.set_root(root_id)?;

        Ok(root_id)
    }

    /// Recursively parse a CDP node
    fn parse_node(
        &mut self,
        cdp_node: &Value,
        parent_id: Option<NodeId>,
        target_id: &TargetId,
    ) -> Result<NodeId> {
        let node_id = cdp_node["nodeId"]
            .as_u64()
            .ok_or_else(|| DomError::CdpError("Missing nodeId".to_string()))?
            as u32;

        let backend_node_id = cdp_node["backendNodeId"]
            .as_u64()
            .ok_or_else(|| DomError::CdpError("Missing backendNodeId".to_string()))?
            as u32;

        let node_type_val = cdp_node["nodeType"]
            .as_u64()
            .ok_or_else(|| DomError::CdpError("Missing nodeType".to_string()))?
            as u8;

        let node_type =
            NodeType::from_u8(node_type_val).ok_or_else(|| DomError::InvalidNodeType {
                expected: "valid NodeType".to_string(),
                actual: format!("{}", node_type_val),
            })?;

        let node_name = cdp_node["nodeName"].as_str().unwrap_or("").to_string();

        let node_value = cdp_node["nodeValue"].as_str().unwrap_or("").to_string();

        // Parse attributes
        let mut attributes = HashMap::new();
        if let Some(attrs) = cdp_node["attributes"].as_array() {
            let mut i = 0;
            while i + 1 < attrs.len() {
                if let (Some(key), Some(value)) = (attrs[i].as_str(), attrs[i + 1].as_str()) {
                    attributes.insert(key.to_string(), value.to_string());
                }
                i += 2;
            }
        }

        // Create node
        let mut node = DomNode::new(
            node_id,
            backend_node_id,
            node_type,
            node_name,
            target_id.clone(),
        );

        node.node_value = node_value;
        node.attributes = attributes;
        node.parent_id = parent_id;
        node.frame_id = cdp_node["frameId"].as_str().map(String::from);
        node.is_scrollable = cdp_node.get("isScrollable").and_then(|v| v.as_bool());

        // Parse shadow root type
        if let Some(shadow_type) = cdp_node.get("shadowRootType").and_then(|v| v.as_str()) {
            node.shadow_root_type = match shadow_type {
                "user-agent" => Some(ShadowRootType::UserAgent),
                "open" => Some(ShadowRootType::Open),
                "closed" => Some(ShadowRootType::Closed),
                _ => None,
            };
        }

        // Add node to arena
        let current_node_id = self.arena.add_node(node);

        // Parse children
        if let Some(children) = cdp_node["children"].as_array() {
            let mut child_ids = smallvec::SmallVec::new();

            for child in children {
                let child_id = self.parse_node(child, Some(current_node_id), target_id)?;
                child_ids.push(child_id);
            }

            // Update parent's children list
            if let Ok(node) = self.arena.get_mut(current_node_id) {
                node.children_ids = child_ids;
            }
        }

        // Parse content document (iframe)
        if let Some(content_doc) = cdp_node.get("contentDocument") {
            let doc_id = self.parse_node(content_doc, Some(current_node_id), target_id)?;
            if let Ok(node) = self.arena.get_mut(current_node_id) {
                node.content_document_id = Some(doc_id);
            }
        }

        // Parse shadow roots
        if let Some(shadow_roots) = cdp_node["shadowRoots"].as_array() {
            let mut shadow_ids = smallvec::SmallVec::new();

            for shadow in shadow_roots {
                let shadow_id = self.parse_node(shadow, Some(current_node_id), target_id)?;
                shadow_ids.push(shadow_id);
            }

            if let Ok(node) = self.arena.get_mut(current_node_id) {
                node.shadow_root_ids = Some(shadow_ids);
            }
        }

        Ok(current_node_id)
    }

    /// Calculate visibility for all nodes
    ///
    /// This implements the visibility algorithm from Python's
    /// `is_element_visible_according_to_all_parents`
    pub fn calculate_visibility(&mut self) -> Result<()> {
        // Get all node IDs first (to avoid borrowing issues)
        let node_ids: Vec<NodeId> = self.arena.node_ids().collect();

        for node_id in node_ids {
            let is_visible = self.is_node_visible(node_id)?;
            if let Ok(node) = self.arena.get_mut(node_id) {
                node.is_visible = Some(is_visible);
            }
        }

        Ok(())
    }

    /// Check if a node is visible (internal implementation)
    fn is_node_visible(&self, node_id: NodeId) -> Result<bool> {
        let node = self.arena.get(node_id)?;

        // Basic CSS visibility check
        if !utils::is_element_visible_by_css(node) {
            return Ok(false);
        }

        // Check bounds
        let bounds = match &node.snapshot_node {
            Some(snapshot) => snapshot.bounds.as_ref(),
            None => return Ok(false),
        };

        if bounds.is_none() {
            return Ok(false);
        }

        // TODO: Implement full frame hierarchy visibility check
        // For now, simplified version
        Ok(true)
    }

    /// Merge accessibility tree data
    ///
    /// Takes CDP Accessibility.getFullAXTree response and merges it into nodes
    pub fn merge_ax_tree(&mut self, ax_tree: &Value) -> Result<()> {
        let nodes = ax_tree["nodes"]
            .as_array()
            .ok_or_else(|| DomError::CdpError("Missing 'nodes' in AX tree".to_string()))?;

        for ax_node in nodes {
            if let Some(backend_id) = ax_node["backendDOMNodeId"].as_u64() {
                if let Some(node_id) = self.arena.get_node_id_by_backend(backend_id as u32) {
                    let ax_data = self.parse_ax_node(ax_node)?;
                    if let Ok(node) = self.arena.get_mut(node_id) {
                        node.ax_node = Some(Box::new(ax_data));
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse accessibility node from CDP
    fn parse_ax_node(&self, ax_node: &Value) -> Result<AXNode> {
        Ok(AXNode {
            ax_node_id: ax_node["nodeId"].as_str().unwrap_or("").to_string(),
            ignored: ax_node["ignored"].as_bool().unwrap_or(false),
            role: ax_node["role"]["value"].as_str().map(String::from),
            name: ax_node["name"]["value"].as_str().map(String::from),
            description: ax_node["description"]["value"].as_str().map(String::from),
            properties: None, // TODO: Parse properties
            child_ids: None,  // TODO: Parse child IDs
        })
    }

    /// Merge snapshot data from DOMSnapshot.captureSnapshot
    pub fn merge_snapshot(&mut self, _snapshot: &Value, _device_pixel_ratio: f64) -> Result<()> {
        // TODO: Implement full snapshot merging
        // This requires parsing the complex DOMSnapshot format
        Ok(())
    }

    /// Get serialized DOM state for LLM
    pub fn serialize_for_llm(&self) -> Result<String> {
        // TODO: Implement serialization
        // This will use the serializer module
        Ok(String::new())
    }
}

impl Default for DomService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dom() {
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
                    "attributes": []
                }]
            }
        });

        let mut service = DomService::new();
        let root_id = service.parse_cdp_dom_tree(&cdp_json).unwrap();

        assert_eq!(root_id, 0);
        assert_eq!(service.arena().len(), 2);
    }
}
