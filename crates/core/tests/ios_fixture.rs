use phonedriver_core::element::Role;
use phonedriver_core::geometry::{Rect, Size};
use phonedriver_core::prune::prune;
use phonedriver_core::raw::{NodeFlags, RawTree};
use phonedriver_core::refs::RefTable;
use phonedriver_core::render::{AppLine, DeviceLine, Filters, Header, Orientation, Rendered};

const TESTBED_MAIN: &str = include_str!("fixtures/ios/testbed_main.json");

fn tree() -> RawTree {
    RawTree::from_ios_json(TESTBED_MAIN).unwrap()
}

fn header() -> Header {
    Header {
        device: Some(DeviceLine {
            id: "ios:280D26B7-7201-4907-877C-7101DE5FE4AE".into(),
            model: "iPhone18,3".into(),
            os: "iOS 26.5".into(),
            screen: Size { w: 402.0, h: 874.0 },
            orientation: Orientation::Portrait,
        }),
        app: Some(AppLine {
            id: "dev.phonedriver.testbed".into(),
            name: Some("Testbed".into()),
        }),
        keyboard: None,
        settle: None,
        secure_hidden: false,
        alerts: Vec::new(),
    }
}

fn rendered() -> Rendered {
    let tree = tree();
    let nodes = prune(&tree);
    let refs = RefTable::default().assign(&tree, &nodes);
    Rendered::build(nodes, &refs, header())
}

#[test]
fn runner_snapshot_parses_in_points_with_xcui_roles() {
    let tree = tree();
    assert_eq!(tree.nodes.len(), 59);
    assert_eq!(tree.nodes[0].role, Role::Other);
    assert_eq!(tree.nodes[0].label.as_deref(), Some("Testbed"));
    let b17 = tree
        .nodes
        .iter()
        .find(|n| n.resource_id.as_deref() == Some("taps.b17"))
        .unwrap();
    assert_eq!(b17.role, Role::Button);
    assert_eq!(b17.label.as_deref(), Some("B17"));
    assert_eq!(
        b17.bounds,
        Rect {
            x: 110.67,
            y: 511.0,
            w: 197.0 - 110.67,
            h: 56.0
        }
    );
    assert!(
        b17.flags
            .contains(NodeFlags::VISIBLE | NodeFlags::VISIBLE_TO_USER | NodeFlags::ENABLED)
    );
}

#[test]
fn pruning_keeps_every_grid_button_and_tab() {
    let tree = tree();
    let kept: Vec<String> = prune(&tree).into_iter().filter_map(|n| n.id).collect();
    for i in 0..24 {
        assert!(kept.contains(&format!("taps.b{i}")), "taps.b{i} pruned");
    }
    for tab in [
        "taps", "text", "list", "gestures", "busy", "secure", "dialogs", "web", "nav",
    ] {
        assert!(kept.contains(&format!("tab.{tab}")), "tab.{tab} pruned");
    }
}

#[test]
fn golden_testbed_main() {
    insta::assert_snapshot!(
        "ios_testbed_main",
        rendered().text(&Filters::default()).unwrap()
    );
}

fn safari_form() -> String {
    let tree = RawTree::from_ios_json(include_str!("fixtures/ios/safari_form.json")).unwrap();
    let nodes = prune(&tree);
    let refs = RefTable::default().assign(&tree, &nodes);
    Rendered::build(nodes, &refs, header())
        .text(&Filters::default())
        .unwrap()
}

#[test]
fn a_safari_radio_is_a_radio_and_a_label_that_repeats_its_control_is_not_printed() {
    let text = safari_form();
    assert!(text.contains("radio \"Small\" off"), "{text}");
    assert!(text.contains("radio \"Medium\" off"), "{text}");
    assert!(text.contains("switch \"Bacon\" on"), "{text}");
    for echo in [
        "Small",
        "Medium",
        "Bacon",
        "Onion",
        "Customer name:",
        "Telephone:",
        "Preferred delivery time:",
    ] {
        assert!(
            !text.contains(&format!("text \"{echo}\"")),
            "{echo} printed twice: {text}"
        );
    }
    assert!(text.contains("textbox \"Customer name:\""), "{text}");
    assert!(!text.contains("\"0\"") && !text.contains("\"1\""), "{text}");
    assert!(!text.contains("scroll bar"), "{text}");
}
