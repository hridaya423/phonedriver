//! Text found in pixels, and its merge into a snapshot taken from the accessibility tree.

use serde::{Deserialize, Serialize};

use crate::element::{Actions, Element, ElementState, Ref, Role, Source};
use crate::geometry::Rect;
use crate::render::Rendered;

/// Overlap above which an OCR box and a tree element with the same text are one thing.
const SAME_IOU: f32 = 0.5;

/// A line of text recognized in a screenshot, in logical points of the current orientation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    pub text: String,
    pub bounds: Rect,
    pub confidence: f32,
}

fn iou(a: Rect, b: Rect) -> f32 {
    let Some(i) = a.clip(b) else { return 0.0 };
    let inter = i.area();
    inter / (a.area() + b.area() - inter)
}

fn same_text(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A tree element says what the box says when its label or value is the box's text.
fn already_in_tree(tree: &[Element], b: &TextBox) -> bool {
    tree.iter().any(|e| {
        iou(e.bounds, b.bounds) > SAME_IOU
            && [e.label.as_deref(), e.value.as_deref()]
                .into_iter()
                .flatten()
                .any(|t| same_text(t, &b.text))
    })
}

/// True when the tree says too little to act on: nothing at all, or a web view with no children.
pub fn tree_is_hollow(rendered: &Rendered) -> bool {
    rendered.elements.is_empty()
        || rendered
            .elements
            .iter()
            .any(|e| e.role == Role::WebView && e.children.is_empty())
}

/// Appends the boxes the tree does not already show as `Source::Ocr` text elements, in reading order.
/// `new_ref` names each one; it is called once per kept box.
pub fn merge(
    rendered: &mut Rendered,
    boxes: Vec<TextBox>,
    mut new_ref: impl FnMut(&TextBox) -> Ref,
) {
    let mut kept: Vec<TextBox> = boxes
        .into_iter()
        .filter(|b| !b.text.trim().is_empty() && !already_in_tree(&rendered.elements, b))
        .collect();
    kept.sort_by(|a, b| {
        (a.bounds.y, a.bounds.x)
            .partial_cmp(&(b.bounds.y, b.bounds.x))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for b in kept {
        rendered.elements.push(Element {
            r#ref: new_ref(&b),
            role: Role::Text,
            label: Some(b.text),
            value: None,
            hint: None,
            id: None,
            bounds: b.bounds,
            state: ElementState::ENABLED,
            actions: Actions::TAP,
            position: None,
            collection: None,
            source: Source::Ocr,
            children: Vec::new(),
        });
        rendered.parents.push(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Filters, Header};

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn tree_with(label: &str, bounds: Rect) -> Rendered {
        Rendered {
            header: Header::default(),
            elements: vec![Element {
                r#ref: Ref(1),
                role: Role::Button,
                label: Some(label.into()),
                value: None,
                hint: None,
                id: None,
                bounds,
                state: ElementState::ENABLED,
                actions: Actions::TAP,
                position: None,
                collection: None,
                source: Source::Tree,
                children: Vec::new(),
            }],
            parents: vec![None],
        }
    }

    fn text_box(text: &str, bounds: Rect) -> TextBox {
        TextBox {
            text: text.into(),
            bounds,
            confidence: 0.9,
        }
    }

    fn merged(mut r: Rendered, boxes: Vec<TextBox>) -> Rendered {
        let mut next = 10;
        merge(&mut r, boxes, |_| {
            next += 1;
            Ref(next)
        });
        r
    }

    #[test]
    fn a_tree_is_hollow_when_empty_or_a_web_view_has_no_children() {
        let mut r = tree_with("Wi-Fi", rect(0.0, 0.0, 10.0, 10.0));
        assert!(!tree_is_hollow(&r));
        r.elements[0].role = Role::WebView;
        assert!(tree_is_hollow(&r));
        r.elements[0].children.push(0);
        assert!(!tree_is_hollow(&r));
        r.elements.clear();
        assert!(tree_is_hollow(&r));
    }

    #[test]
    fn a_box_over_a_tree_element_with_the_same_text_is_dropped() {
        let r = merged(
            tree_with("Wi-Fi", rect(10.0, 10.0, 100.0, 40.0)),
            vec![text_box("WI-FI", rect(12.0, 14.0, 90.0, 30.0))],
        );
        assert_eq!(r.elements.len(), 1);
    }

    #[test]
    fn a_box_with_other_text_over_a_tree_element_is_kept() {
        let r = merged(
            tree_with("Wi-Fi", rect(10.0, 10.0, 100.0, 40.0)),
            vec![text_box("Bluetooth", rect(12.0, 14.0, 90.0, 30.0))],
        );
        assert_eq!(r.elements.len(), 2);
    }

    #[test]
    fn the_same_text_elsewhere_is_kept() {
        let r = merged(
            tree_with("Wi-Fi", rect(10.0, 10.0, 100.0, 40.0)),
            vec![text_box("Wi-Fi", rect(10.0, 300.0, 100.0, 40.0))],
        );
        assert_eq!(r.elements.len(), 2);
    }

    #[test]
    fn a_box_that_barely_overlaps_is_kept() {
        let r = merged(
            tree_with("Wi-Fi", rect(0.0, 0.0, 100.0, 40.0)),
            vec![text_box("Wi-Fi", rect(60.0, 0.0, 100.0, 40.0))],
        );
        assert_eq!(r.elements.len(), 2);
    }

    #[test]
    fn kept_boxes_become_ocr_elements_in_reading_order_with_refs() {
        let r = merged(
            Rendered {
                header: Header::default(),
                elements: vec![],
                parents: vec![],
            },
            vec![
                text_box("Second", rect(10.0, 200.0, 80.0, 20.0)),
                text_box("   ", rect(10.0, 50.0, 80.0, 20.0)),
                text_box("First", rect(10.0, 100.0, 80.0, 20.0)),
            ],
        );
        let text = r.text(&Filters::default()).unwrap();
        assert!(
            text.ends_with("@e11 text \"First\" (ocr)\n@e12 text \"Second\" (ocr)"),
            "{text}"
        );
        assert!(r.elements.iter().all(|e| e.source == Source::Ocr));
    }
}
