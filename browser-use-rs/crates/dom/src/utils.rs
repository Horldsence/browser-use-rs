//! Utility functions for DOM processing

use crate::arena::DomArena;
use crate::error::Result;
use crate::types::{DomNode, DomRect, NodeType};

/// Cap text length to avoid token explosion
pub fn cap_text_length(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

/// Check if element is visible according to CSS
pub fn is_element_visible_by_css(node: &DomNode) -> bool {
    if let Some(snapshot) = &node.snapshot_node {
        if let Some(styles) = &snapshot.computed_styles {
            let display = styles.get("display").map(|s| s.as_str()).unwrap_or("");
            let visibility = styles.get("visibility").map(|s| s.as_str()).unwrap_or("");
            let opacity = styles.get("opacity").map(|s| s.as_str()).unwrap_or("1");

            if display == "none" || visibility == "hidden" {
                return false;
            }

            if let Ok(opacity_val) = opacity.parse::<f64>() {
                if opacity_val <= 0.0 {
                    return false;
                }
            }
        }
    }

    true
}

/// Check if element intersects with viewport/frame
pub fn check_frame_intersection(
    element_bounds: &DomRect,
    frame_client_rect: &DomRect,
    frame_scroll_rect: &DomRect,
) -> bool {
    // Viewport boundaries
    let viewport_left = 0.0;
    let viewport_top = 0.0;
    let viewport_right = frame_client_rect.width;
    let viewport_bottom = frame_client_rect.height;

    // Adjust element position by scroll offset
    let adjusted_x = element_bounds.x - frame_scroll_rect.x;
    let adjusted_y = element_bounds.y - frame_scroll_rect.y;

    // Check intersection (with 1000px tolerance for below-fold content)
    adjusted_x < viewport_right
        && adjusted_x + element_bounds.width > viewport_left
        && adjusted_y < viewport_bottom + 1000.0
        && adjusted_y + element_bounds.height > viewport_top - 1000.0
}

/// Get all text content from node and its children
pub fn get_text_content(arena: &DomArena, node_id: u32) -> Result<String> {
    let mut text = String::new();

    arena.traverse_df(node_id, |node| {
        if node.node_type == NodeType::Text {
            text.push_str(&node.node_value);
        }
        Ok(())
    })?;

    Ok(text.trim().to_string())
}

/// Detect if button is pagination button based on text/attributes
pub fn is_pagination_button(node: &DomNode) -> Option<PaginationType> {
    if !node.is_clickable() {
        return None;
    }

    let text = node.node_value.to_lowercase();
    let aria_label = node.attr("aria-label").unwrap_or("").to_lowercase();
    let title = node.attr("title").unwrap_or("").to_lowercase();
    let class = node.attr("class").unwrap_or("").to_lowercase();

    let all_text = format!("{} {} {} {}", text, aria_label, title, class);

    // Check for disabled state
    let is_disabled = node.attr("disabled") == Some("true")
        || node.attr("aria-disabled") == Some("true")
        || class.contains("disabled");

    if is_disabled {
        return None;
    }

    // Pattern matching
    const NEXT_PATTERNS: &[&str] = &["next", ">", "»", "→", "siguiente", "suivant", "weiter"];
    const PREV_PATTERNS: &[&str] = &["prev", "previous", "<", "«", "←", "anterior"];
    const FIRST_PATTERNS: &[&str] = &["first", "⇤", "primera", "première", "erste"];
    const LAST_PATTERNS: &[&str] = &["last", "⇥", "última", "dernier", "letzte"];

    for pattern in NEXT_PATTERNS {
        if all_text.contains(pattern) {
            return Some(PaginationType::Next);
        }
    }

    for pattern in PREV_PATTERNS {
        if all_text.contains(pattern) {
            return Some(PaginationType::Previous);
        }
    }

    for pattern in FIRST_PATTERNS {
        if all_text.contains(pattern) {
            return Some(PaginationType::First);
        }
    }

    for pattern in LAST_PATTERNS {
        if all_text.contains(pattern) {
            return Some(PaginationType::Last);
        }
    }

    // Check for numeric page buttons
    if text.chars().all(|c| c.is_numeric()) && text.len() <= 2 {
        let role = node.attr("role").unwrap_or("");
        if role == "button" || role == "link" || role.is_empty() {
            return Some(PaginationType::PageNumber);
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaginationType {
    Next,
    Previous,
    First,
    Last,
    PageNumber,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cap_text_length() {
        assert_eq!(cap_text_length("hello", 10), "hello");
        assert_eq!(cap_text_length("hello world", 5), "hello...");
    }

    #[test]
    fn test_frame_intersection() {
        let element = DomRect::new(10.0, 10.0, 100.0, 100.0);
        let client = DomRect::new(0.0, 0.0, 800.0, 600.0);
        let scroll = DomRect::new(0.0, 0.0, 0.0, 0.0);

        assert!(check_frame_intersection(&element, &client, &scroll));
    }
}
