//! What a screen reader or a thumb would trip on, read from a screen's elements alone (decision D90): a control
//! with nothing to say, and a tap target smaller than the platform's guidelines ask for.

use crate::element::{Actions, Element, Role};
use crate::geometry::Rect;
use crate::render::Rendered;

/// Apple's Human Interface Guidelines ask for 44 by 44 points.
const IOS_TARGET: f32 = 44.0;
/// Android's accessibility guidelines ask for 48 by 48 dp.
const ANDROID_TARGET: f32 = 48.0;

/// Says what the size of a small target is measured from, for the reader to judge the finding.
pub const MEASURED_FROM: &str = "sizes are the bounds accessibility reports; a touch area the app enlarges (React Native `hitSlop`) is not seen";

/// One problem on the screen, as its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Unnamed(String),
    SmallTarget(String),
}

impl Finding {
    pub fn line(&self) -> &str {
        match self {
            Finding::Unnamed(l) | Finding::SmallTarget(l) => l,
        }
    }
}

/// One finding for each problem on the screen, in screen order.
pub fn audit(r: &Rendered) -> Vec<Finding> {
    let device = r.header.device.as_ref();
    let target = match device {
        Some(d) if d.id.starts_with("android:") => ANDROID_TARGET,
        _ => IOS_TARGET,
    };
    let screen = device.map(|d| Rect {
        x: 0.0,
        y: 0.0,
        w: d.screen.w,
        h: d.screen.h,
    });
    let mut out = Vec::new();
    for (i, e) in r.elements.iter().enumerate() {
        if !acts(e) || e.bounds.w <= 0.0 || e.bounds.h <= 0.0 {
            continue;
        }
        if !named(r, i) {
            out.push(Finding::Unnamed(format!(
                "{}: no name a screen reader can say",
                r.line(e)
            )));
        } else if (e.bounds.w < target || e.bounds.h < target)
            && e.role != Role::Link
            && !cut_off(r, i, screen)
        {
            out.push(Finding::SmallTarget(format!(
                "{}: {:.0}x{:.0}pt, under {target:.0}x{target:.0}pt",
                r.line(e),
                e.bounds.w,
                e.bounds.h
            )));
        }
    }
    out
}

fn acts(e: &Element) -> bool {
    e.actions.intersects(Actions::TAP | Actions::EDIT)
        || matches!(
            e.role,
            Role::Button
                | Role::Switch
                | Role::Checkbox
                | Role::Radio
                | Role::Slider
                | Role::TextField
                | Role::SearchField
                | Role::SecureField
                | Role::Tab
                | Role::MenuItem
        )
}

/// A screen reader says an element's label, its hint, or the text inside it.
fn named(r: &Rendered, i: usize) -> bool {
    let says = |e: &Element| {
        [&e.label, &e.hint]
            .iter()
            .any(|t| t.as_deref().is_some_and(|t| !t.trim().is_empty()))
    };
    says(&r.elements[i])
        || (i + 1..r.elements.len()).any(|j| inside(r, j, i) && says(&r.elements[j]))
}

fn inside(r: &Rendered, mut j: usize, i: usize) -> bool {
    while let Some(p) = r.parents[j] {
        if p == i {
            return true;
        }
        j = p;
    }
    false
}

/// An element at the edge of the screen or of a list around it may be larger than the part on screen.
fn cut_off(r: &Rendered, i: usize, screen: Option<Rect>) -> bool {
    let b = r.elements[i].bounds;
    let at_edge = |c: Rect| {
        b.x <= c.x + 0.5
            || b.y <= c.y + 0.5
            || b.x + b.w >= c.x + c.w - 0.5
            || b.y + b.h >= c.y + c.h - 0.5
    };
    screen.is_some_and(at_edge)
        || (0..r.elements.len()).any(|a| {
            r.elements[a].actions.intersects(Actions::SCROLL)
                && inside(r, i, a)
                && at_edge(r.elements[a].bounds)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Scale, Size};
    use crate::prune::prune;
    use crate::raw::RawTree;
    use crate::refs::RefTable;
    use crate::render::{DeviceLine, Header};

    fn screen(id: &str, nodes: &str) -> Rendered {
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","bounds":[0,0,400,800]}}],"nodes":[{nodes}]}}"#
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let nodes = prune(&tree);
        let refs = RefTable::default().assign(&tree, &nodes);
        let header = Header {
            device: Some(DeviceLine {
                id: id.into(),
                model: "Fake".into(),
                os: "Fake 1".into(),
                screen: Size { w: 400.0, h: 800.0 },
                orientation: crate::render::Orientation::Portrait,
            }),
            ..Header::default()
        };
        Rendered::build(nodes, &refs, header)
    }

    const NODES: &str = r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.ImageButton","b":[20,40,68,88],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Save","b":[100,40,300,88],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.ImageButton","desc":"Close","b":[340,52,364,76],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.FrameLayout","b":[20,120,380,180],"f":"vVe","a":["tap"]},
        {"p":4,"w":1,"cls":"android.widget.TextView","text":"Wi-Fi","b":[40,130,200,170],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.EditText","b":[20,200,380,260],"f":"vVeE","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"android.widget.EditText","hint":"Email","b":[20,280,380,340],"f":"vVeE","a":["tap","set_text"]},
        {"p":0,"w":1,"cls":"androidx.recyclerview.widget.RecyclerView","b":[20,400,380,700],"f":"vVeS","a":["scroll_down"]},
        {"p":8,"w":1,"cls":"android.widget.Button","text":"Last row","b":[20,690,380,700],"f":"vVe","a":["tap"]}"#;

    #[test]
    fn a_control_with_nothing_to_say_and_a_small_target_are_found_and_the_rest_is_not() {
        let r = screen("android:emulator-5554", NODES);
        assert_eq!(
            audit(&r).iter().map(Finding::line).collect::<Vec<_>>(),
            [
                "@e1 button: no name a screen reader can say",
                "@e3 button \"Close\": 24x24pt, under 48x48pt",
                "@e5 textbox: no name a screen reader can say",
            ],
            "{}",
            r.text(&Default::default()).unwrap()
        );
    }

    #[test]
    fn an_iphone_asks_for_44_points() {
        let nodes = r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,400,800],"f":"vVe"},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"OK","b":[100,100,146,146],"f":"vVe","a":["tap"]},
            {"p":0,"w":1,"cls":"android.widget.Button","text":"Help","b":[200,100,240,140],"f":"vVe","a":["tap"]}"#;
        assert_eq!(
            audit(&screen("sim:E02F", nodes))
                .iter()
                .map(Finding::line)
                .collect::<Vec<_>>(),
            ["@e2 button \"Help\": 40x40pt, under 44x44pt"]
        );
    }
}
