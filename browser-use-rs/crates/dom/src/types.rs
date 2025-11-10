//! Core type definitions matching Python's browser_use/dom/views.py
//!
//! Key design principles:
//! 1. Use u32 for indices (4 bytes vs 8 bytes pointer)
//! 2. Use borrowed strings where possible (&str)
//! 3. Use SmallVec for small arrays (avoid heap allocation)
//! 4. Use Option<Box<T>> for large optional fields (reduce struct size)

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;

/// Node identifier (index into arena)
/// u32 allows 4 billion nodes, enough for any webpage
pub type NodeId = u32;

/// Frame identifier from CDP
pub type FrameId = String;

/// Target identifier from CDP
pub type TargetId = String;

/// Session identifier from CDP
pub type SessionId = String;

/// Node type matching DOM specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum NodeType {
    Element = 1,
    Attribute = 2,
    Text = 3,
    CdataSection = 4,
    EntityReference = 5,
    Entity = 6,
    ProcessingInstruction = 7,
    Comment = 8,
    Document = 9,
    DocumentType = 10,
    DocumentFragment = 11,
    Notation = 12,
}

impl NodeType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(NodeType::Element),
            2 => Some(NodeType::Attribute),
            3 => Some(NodeType::Text),
            4 => Some(NodeType::CdataSection),
            5 => Some(NodeType::EntityReference),
            6 => Some(NodeType::Entity),
            7 => Some(NodeType::ProcessingInstruction),
            8 => Some(NodeType::Comment),
            9 => Some(NodeType::Document),
            10 => Some(NodeType::DocumentType),
            11 => Some(NodeType::DocumentFragment),
            12 => Some(NodeType::Notation),
            _ => None,
        }
    }
}

/// Shadow root type from CDP
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowRootType {
    UserAgent,
    Open,
    Closed,
}

/// Rectangle with coordinates
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DomRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl DomRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Check if rectangle intersects with another
    pub fn intersects(&self, other: &DomRect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Apply offset (for iframe coordinate transformation)
    pub fn offset(&self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// Accessibility property name (subset of AXPropertyName from CDP)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AXPropertyName {
    Checked,
    Selected,
    Expanded,
    Pressed,
    Disabled,
    Invalid,
    ValueMin,
    ValueMax,
    ValueNow,
    ValueText,
    KeyShortcuts,
    HasPopup,
    Multiselectable,
    Required,
    Level,
    Busy,
    Live,
    Other(String),
}

/// Accessibility property value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXProperty {
    pub name: AXPropertyName,
    pub value: AXPropertyValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AXPropertyValue {
    String(String),
    Bool(bool),
    Null,
}

/// Accessibility node data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXNode {
    pub ax_node_id: String,
    pub ignored: bool,
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub properties: Option<Vec<AXProperty>>,
    pub child_ids: Option<SmallVec<[String; 4]>>,
}

/// Snapshot data from DOMSnapshot.captureSnapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotNode {
    pub is_clickable: Option<bool>,
    pub cursor_style: Option<String>,
    /// Document coordinates (top-left of page, ignores scroll)
    pub bounds: Option<DomRect>,
    /// Viewport coordinates (visible scrollport)
    pub client_rects: Option<DomRect>,
    /// Scrollable area
    pub scroll_rects: Option<DomRect>,
    /// Computed CSS styles
    pub computed_styles: Option<HashMap<String, String>>,
    /// Paint order (for z-index calculation)
    pub paint_order: Option<i32>,
    /// Stacking contexts
    pub stacking_contexts: Option<i32>,
}

/// The main DOM tree node structure
///
/// Design philosophy:
/// - Small fixed-size fields first (better packing)
/// - Use indices instead of pointers
/// - Use Option<Box<T>> for large optional data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomNode {
    // IDs (12 bytes)
    pub node_id: NodeId,
    pub backend_node_id: u32,
    pub node_type: NodeType, // 1 byte

    // Navigation indices (24 bytes with padding)
    pub parent_id: Option<NodeId>,
    pub children_ids: SmallVec<[NodeId; 4]>, // Most nodes have <4 children

    // Optional large data (8 bytes each pointer)
    pub node_name: String,
    pub node_value: String,
    pub attributes: HashMap<String, String>,

    // Frame/session info
    pub frame_id: Option<FrameId>,
    pub target_id: TargetId,
    pub session_id: Option<SessionId>,

    // Special DOM structures
    pub content_document_id: Option<NodeId>,
    pub shadow_root_type: Option<ShadowRootType>,
    pub shadow_root_ids: Option<SmallVec<[NodeId; 2]>>,

    // State
    pub is_scrollable: Option<bool>,
    pub is_visible: Option<bool>,

    // Position (only for visible elements)
    pub absolute_position: Option<DomRect>,

    // Enhanced data (boxed to reduce struct size)
    pub ax_node: Option<Box<AXNode>>,
    pub snapshot_node: Option<Box<SnapshotNode>>,

    // UUID for tracking
    pub uuid: String,
}

impl DomNode {
    /// Create a new node with required fields
    pub fn new(
        node_id: NodeId,
        backend_node_id: u32,
        node_type: NodeType,
        node_name: String,
        target_id: TargetId,
    ) -> Self {
        Self {
            node_id,
            backend_node_id,
            node_type,
            node_name,
            node_value: String::new(),
            attributes: HashMap::new(),
            parent_id: None,
            children_ids: SmallVec::new(),
            frame_id: None,
            target_id,
            session_id: None,
            content_document_id: None,
            shadow_root_type: None,
            shadow_root_ids: None,
            is_scrollable: None,
            is_visible: None,
            absolute_position: None,
            ax_node: None,
            snapshot_node: None,
            uuid: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Get tag name for element nodes
    pub fn tag_name(&self) -> Option<&str> {
        if self.node_type == NodeType::Element {
            Some(&self.node_name)
        } else {
            None
        }
    }

    /// Check if node is an element
    pub fn is_element(&self) -> bool {
        self.node_type == NodeType::Element
    }

    /// Check if node is text
    pub fn is_text(&self) -> bool {
        self.node_type == NodeType::Text
    }

    /// Get attribute value
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(|s| s.as_str())
    }

    /// Check if element is clickable
    pub fn is_clickable(&self) -> bool {
        self.snapshot_node
            .as_ref()
            .and_then(|s| s.is_clickable)
            .unwrap_or(false)
    }
}

/// Simplified node for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplifiedNode {
    pub node_id: NodeId,
    pub should_display: bool,
    pub is_interactive: bool,
    pub is_new: bool,
    pub ignored_by_paint_order: bool,
    pub excluded_by_parent: bool,
    pub is_shadow_host: bool,
    pub is_compound_component: bool,
}

/// Default attributes to include in serialization
pub const DEFAULT_INCLUDE_ATTRIBUTES: &[&str] = &[
    "title",
    "type",
    "checked",
    "id",
    "name",
    "role",
    "value",
    "placeholder",
    "data-date-format",
    "alt",
    "aria-label",
    "aria-expanded",
    "data-state",
    "aria-checked",
    "aria-valuemin",
    "aria-valuemax",
    "aria-valuenow",
    "aria-placeholder",
    "pattern",
    "min",
    "max",
    "minlength",
    "maxlength",
    "step",
    "accept",
    "multiple",
    "inputmode",
    "autocomplete",
    "data-mask",
    "data-inputmask",
    "data-datepicker",
    "format",
    "expected_format",
    "contenteditable",
    "pseudo",
    "selected",
    "pressed",
    "disabled",
    "invalid",
    "keyshortcuts",
    "haspopup",
    "multiselectable",
    "required",
    "valuetext",
    "level",
    "busy",
    "live",
    "ax_name",
];

/// Static attributes (for caching/optimization)
pub const STATIC_ATTRIBUTES: &[&str] = &[
    "class",
    "id",
    "name",
    "type",
    "placeholder",
    "aria-label",
    "title",
    "role",
    "data-testid",
    "data-test",
    "data-cy",
    "data-selenium",
    "for",
    "required",
    "disabled",
    "readonly",
    "checked",
    "selected",
    "multiple",
    "accept",
    "href",
    "target",
    "rel",
    "aria-describedby",
    "aria-labelledby",
    "aria-controls",
    "aria-owns",
    "aria-live",
    "aria-atomic",
    "aria-busy",
    "aria-disabled",
    "aria-hidden",
    "aria-pressed",
    "aria-checked",
    "aria-selected",
    "tabindex",
    "alt",
    "src",
    "lang",
    "itemscope",
    "itemtype",
    "itemprop",
    "pseudo",
    "aria-valuemin",
    "aria-valuemax",
    "aria-valuenow",
    "aria-placeholder",
];
