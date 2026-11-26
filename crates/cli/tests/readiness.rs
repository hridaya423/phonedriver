//! Decision D127: a replayed step ends on the next step's target, once that target has appeared and is still,
//! instead of waiting for the whole screen to go quiet. Every case where it must not is here too.

use crate::fakes;

use std::sync::Arc;
use std::time::Duration;

use fakes::{OneDevice, tree};
use phonedriver::daemon::{Config, Daemon};
use phonedriver_core::device::FrameRef;
use phonedriver_core::geometry::{Rect, Scale};
use phonedriver_core::raw::RawTree;
use phonedriver_core::testing::FakeDevice;
use serde_json::{Value, json};

const THIRD_ROW: Rect = Rect {
    x: 0.0,
    y: 300.0,
    w: 360.0,
    h: 60.0,
};

const BANNER: Rect = Rect {
    x: 0.0,
    y: 600.0,
    w: 360.0,
    h: 150.0,
};

fn setup(first: RawTree) -> (Arc<Daemon>, Arc<FakeDevice>, tempfile::TempDir) {
    let fake = Arc::new(FakeDevice::new(vec![first]));
    let home = tempfile::tempdir().unwrap();
    let d = Daemon::new(
        Arc::new(OneDevice(fake.clone())),
        Config {
            home: home.path().to_path_buf(),
        },
    );
    (d, fake, home)
}

thread_local! {
    static FRAMES: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

fn frame(over: Rect) -> FrameRef {
    let seq = FRAMES.with(|n| n.replace(n.get() + 1));
    FrameRef {
        seq,
        changed_area_pt2: over.w * over.h,
        changed_box: Some(over),
    }
}

fn moving(fake: &Arc<FakeDevice>, place: Rect, from: u64, until: u64) {
    let fake = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(from)).await;
        let mut at = from;
        while at < until {
            fake.push_frame(frame(place));
            tokio::time::sleep(Duration::from_millis(50)).await;
            at += 50;
        }
    });
}

fn arrives(fake: &Arc<FakeDevice>, next: RawTree, place: Option<Rect>, after: u64) {
    let fake = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(after)).await;
        fake.set_tree(next);
        if let Some(place) = place {
            fake.push_frame(frame(place));
        }
    });
}

fn tap_then(next: Value) -> Value {
    json!({"action": {"kind": "tap", "target": {"point": {"x": 10.0, "y": 10.0}}}, "next": next})
}

fn third() -> Value {
    json!({"target": {"text": "Three"}, "on": {"app": "com.example", "over": false}})
}

async fn act(d: &Daemon, params: Value) -> (Value, Duration) {
    let began = tokio::time::Instant::now();
    let r = d
        .call("default", "act", params)
        .await
        .unwrap_or_else(|e| panic!("act failed: {e:?}"));
    (r, began.elapsed())
}

async fn seen(d: &Daemon) {
    d.call("default", "snapshot", json!({})).await.unwrap();
}

const WHOLE_WAIT: Duration = Duration::from_secs(5);

#[tokio::test(start_paused = true)]
async fn a_step_ends_on_its_next_target_while_a_banner_elsewhere_keeps_moving() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two", "Three"]), Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took < Duration::from_secs(1), "{took:?} {r}");
    assert!(took >= Duration::from_millis(350), "{took:?} {r}");
    assert_eq!(r["settled"], true, "{r}");
    assert_eq!(r["changed"], true, "{r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("\"Three\""),
        "{r}"
    );
}

#[tokio::test(start_paused = true)]
async fn without_a_next_target_the_banner_costs_the_whole_wait_as_before() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two", "Three"]), Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let mut plain = tap_then(third());
    plain.as_object_mut().unwrap().remove("next");
    let (r, took) = act(&d, plain).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
    assert_eq!(r["settled"], false, "{r}");
}

/// "Settings" on one page and "Settings" on the next: a target that was there before the step says nothing about
/// whether the step's own effect has arrived.
#[tokio::test(start_paused = true)]
async fn a_next_target_that_was_on_screen_before_the_step_ends_nothing_early() {
    let (d, fake, _home) = setup(tree(&["One", "Two", "Three"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two!", "Three"]), Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

/// The target is in the tree and then goes on sliding: a tap sent now would land where it no longer is.
#[tokio::test(start_paused = true)]
async fn a_next_target_that_appears_and_goes_on_moving_is_waited_for_until_it_is_still() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    let lower = Rect {
        y: 400.0,
        ..THIRD_ROW
    };
    arrives(&fake, tree(&["One", "Two", "", "Three"]), Some(lower), 100);
    let both = Rect {
        y: 300.0,
        h: 160.0,
        ..THIRD_ROW
    };
    moving(&fake, both, 160, 900);
    arrives(&fake, tree(&["One", "Two", "Three"]), Some(both), 900);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= Duration::from_millis(1150), "{took:?} {r}");
    assert!(took < Duration::from_secs(2), "{took:?} {r}");
    let (tapped, _) = act(
        &d,
        json!({"action": {"kind": "tap", "target": {"text": "Three"}}}),
    )
    .await;
    assert!(
        tapped["summary"].as_str().unwrap().contains("\"Three\""),
        "{tapped}"
    );
    let landed = fake
        .calls()
        .iter()
        .rev()
        .find_map(|c| match c {
            phonedriver_core::testing::Call::Gesture(g) => g.fingers[0].first().map(|p| p.at.y),
            _ => None,
        })
        .unwrap();
    assert!((300.0..=360.0).contains(&landed), "tapped at y {landed}");
}

fn dialog_over(labels: &[&str]) -> RawTree {
    let nodes: Vec<String> = std::iter::once(
        r#"{"p":-1,"w":2,"cls":"android.widget.FrameLayout","b":[20,280,340,500],"f":"vVe"}"#
            .to_string(),
    )
    .chain(labels.iter().enumerate().map(|(i, l)| {
        let top = 300 + 60 * i as i32;
        format!(
            r#"{{"p":0,"w":2,"cls":"android.widget.Button","text":"{l}","b":[40,{top},320,{}],"f":"vVe","a":["tap"]}}"#,
            top + 50
        )
    }))
    .collect();
    let json = format!(
        r#"{{"windows":[{{"id":2,"type":"application","package":"com.example","bounds":[20,280,340,500]}}],"nodes":[{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

/// The next step was saved with no dialog in front. A dialog that holds a button of the same name is not its screen.
#[tokio::test(start_paused = true)]
async fn a_dialog_that_was_not_there_when_the_next_step_was_saved_ends_nothing_early() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, dialog_over(&["Three"]), Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

/// Issue #53: Android reports a page that is still sliding in as a window short of one edge. A step saved on such
/// a page keeps no screen, so nothing but the window's own shape says the page has not arrived.
#[tokio::test(start_paused = true)]
async fn a_page_that_is_still_sliding_in_ends_nothing_early() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    let mut sliding = tree(&["One", "Two", "Three"]);
    sliding.windows[0].bounds.x = 36.0;
    sliding.windows[0].bounds.w = 324.0;
    arrives(&fake, sliding, Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let saved_with_no_screen = json!({"target": {"text": "Three"}});
    let (r, took) = act(&d, tap_then(saved_with_no_screen)).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

/// The tree can run ahead of the pixels: the row is listed and nothing has been drawn where it will be.
#[tokio::test(start_paused = true)]
async fn a_next_target_the_tree_lists_and_no_frame_has_drawn_ends_nothing_early() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two", "Three"]), None, 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

/// Rows are still being added: the tree of one read is longer than the one before it.
#[tokio::test(start_paused = true)]
async fn a_list_that_is_still_growing_is_read_again_before_the_step_ends() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two", "Three"]), Some(THIRD_ROW), 100);
    for (i, at) in [(4, 300u64), (5, 400), (6, 500), (7, 600)] {
        let labels: Vec<String> = ["One", "Two", "Three"]
            .iter()
            .map(|s| s.to_string())
            .chain((4..=i).map(|n| format!("Row {n}")))
            .collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        arrives(&fake, tree(&labels), None, at);
    }
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= Duration::from_millis(700), "{took:?} {r}");
    assert!(took < Duration::from_secs(2), "{took:?} {r}");
    assert!(
        r["observation"]["text"]
            .as_str()
            .unwrap()
            .contains("\"Row 7\""),
        "{r}"
    );
}

/// A screen that goes quiet as a whole ends as it always did, and no sooner for the next target being there.
#[tokio::test(start_paused = true)]
async fn on_a_screen_that_goes_quiet_the_step_takes_what_it_took_before() {
    let mut times = Vec::new();
    for with_next in [false, true] {
        let (d, fake, _home) = setup(tree(&["One", "Two"]));
        seen(&d).await;
        arrives(&fake, tree(&["One", "Two", "Three"]), Some(THIRD_ROW), 100);
        let mut params = tap_then(third());
        if !with_next {
            params.as_object_mut().unwrap().remove("next");
        }
        let (r, took) = act(&d, params).await;
        assert_eq!(r["settled"], true, "{r}");
        times.push(took);
    }
    assert!(times[1] <= times[0], "{times:?}");
    assert!(
        times[0] - times[1] < Duration::from_millis(150),
        "{times:?}"
    );
}

/// The frame channel holds one frame. One that was replaced before the daemon looked may have drawn anywhere.
#[tokio::test(start_paused = true)]
async fn a_frame_the_daemon_never_saw_counts_as_drawn_over_the_next_target() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    arrives(&fake, tree(&["One", "Two", "Three"]), Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 290);
    let unseen = fake.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        frame(THIRD_ROW);
        unseen.push_frame(frame(BANNER));
    });
    moving(&fake, BANNER, 325, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= Duration::from_millis(550), "{took:?} {r}");
    assert!(took < Duration::from_secs(1), "{took:?} {r}");
}

/// Clock's stopwatch on the API 37 emulator, 2026-11-25: its digits differ in every read, two trees never agree,
/// and each step paid the whole second the reply waits for them (`stabilize` 1,022 to 1,062 ms a step).
#[tokio::test(start_paused = true)]
async fn a_text_that_ticks_does_not_hold_up_a_step_whose_next_target_is_in_place() {
    let ticking = |from: u32| -> Vec<RawTree> {
        (from..from + 60)
            .rev()
            .map(|n| tree(&["One", "Two", "Three", &format!("00:{n:02}")]))
            .collect()
    };
    let mut times = Vec::new();
    for with_next in [false, true] {
        let fake = Arc::new(FakeDevice::new(ticking(0)));
        let home = tempfile::tempdir().unwrap();
        let d = Daemon::new(
            Arc::new(OneDevice(fake.clone())),
            Config {
                home: home.path().to_path_buf(),
            },
        );
        seen(&d).await;
        let drawn = fake.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drawn.push_frame(frame(THIRD_ROW));
        });
        let mut params = tap_then(third());
        if !with_next {
            params.as_object_mut().unwrap().remove("next");
        }
        let (r, took) = act(&d, params).await;
        assert_eq!(r["settled"], true, "{r}");
        times.push(took);
    }
    assert!(times[0] >= Duration::from_millis(1300), "{times:?}");
    assert!(times[1] <= Duration::from_millis(600), "{times:?}");
}

/// The third row is in the tree and the keyboard is over it: a tap there would type a key.
#[tokio::test(start_paused = true)]
async fn a_next_target_the_keyboard_covers_ends_nothing_early() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    let json = r#"{"windows":[{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]},
        {"id":2,"type":"input_method","package":"com.google.android.inputmethod.latin","bounds":[0,250,360,800]}],
      "nodes":[
        {"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"One","b":[0,100,360,160],"f":"vVe","a":["tap"]},
        {"p":0,"w":1,"cls":"android.widget.Button","text":"Three","b":[0,300,360,360],"f":"vVe","a":["tap"]},
        {"p":-1,"w":2,"cls":"android.inputmethodservice.SoftInputWindow","b":[0,250,360,800],"f":"vVe"}]}"#;
    let under = RawTree::from_android_json(json, Scale { density: 1.0 }).unwrap();
    arrives(&fake, under, Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(json!({"target": {"text": "Three"}}))).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

fn screen(nodes: &[String]) -> RawTree {
    let json = format!(
        r#"{{"windows":[{{"id":1,"type":"application","package":"com.example","bounds":[0,0,360,800]}}],"nodes":[{{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}},{}]}}"#,
        nodes.join(",")
    );
    RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap()
}

fn button(label: &str, top: i32) -> String {
    format!(
        r#"{{"p":0,"w":1,"cls":"android.widget.Button","text":"{label}","b":[0,{top},360,{}],"f":"vVe","a":["tap"]}}"#,
        top + 60
    )
}

const SPINNER: &str =
    r#"{"p":0,"w":1,"cls":"android.widget.ProgressBar","b":[160,700,200,740],"f":"vVe"}"#;

fn last_tap_y(fake: &FakeDevice) -> f32 {
    fake.calls()
        .iter()
        .rev()
        .find_map(|c| match c {
            phonedriver_core::testing::Call::Gesture(g) => g.fingers[0].first().map(|p| p.at.y),
            _ => None,
        })
        .unwrap()
}

/// A spinner says the page is not all there: what loads next can push the row somewhere else. The reply waits for
/// it as it did, and the next step taps the row where it ended up.
#[tokio::test(start_paused = true)]
async fn a_spinner_on_the_page_is_waited_for_and_the_next_tap_lands_where_the_row_went() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    let loading = screen(&[
        button("One", 100),
        button("Three", 300),
        SPINNER.to_string(),
    ]);
    arrives(&fake, loading, Some(THIRD_ROW), 100);
    let loaded = screen(&[
        button("One", 100),
        button("Two", 200),
        button("New", 300),
        button("Three", 400),
    ]);
    arrives(&fake, loaded, None, 2_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= Duration::from_secs(2), "{took:?} {r}");
    act(
        &d,
        json!({"action": {"kind": "tap", "target": {"text": "Three"}}}),
    )
    .await;
    let landed = last_tap_y(&fake);
    assert!((400.0..=460.0).contains(&landed), "tapped at y {landed}");
}

/// The same spinner with a banner that never stops: the target is in place and still, and the page is loading.
#[tokio::test(start_paused = true)]
async fn a_spinner_on_the_page_ends_nothing_early_under_a_moving_banner() {
    let (d, fake, _home) = setup(tree(&["One", "Two"]));
    seen(&d).await;
    let loading = screen(&[
        button("One", 100),
        button("Three", 300),
        SPINNER.to_string(),
    ]);
    arrives(&fake, loading, Some(THIRD_ROW), 100);
    moving(&fake, BANNER, 25, 20_000);
    let (r, took) = act(&d, tap_then(third())).await;
    assert!(took >= WHOLE_WAIT, "{took:?} {r}");
}

fn form(value: &str) -> RawTree {
    screen(&[
        format!(
            r#"{{"p":0,"w":1,"cls":"android.widget.EditText","text":"{value}","desc":"Field","b":[0,100,360,160],"f":"vVeEF","a":["tap","set_text"]}}"#
        ),
        button("Three", 300),
    ])
}

/// Typed text is checked on the tree the reply is built from. The field fills a letter a read while the next
/// step's button sits still, so two reads that agree on the button do not say the text has landed.
#[tokio::test(start_paused = true)]
async fn typed_text_is_verified_on_reads_that_agree_as_a_whole_whatever_the_next_step_is() {
    let (d, fake, _home) = setup(form(""));
    seen(&d).await;
    let drawn = fake.clone();
    fake.on_text(move |_| {
        drawn.push_frame(frame(THIRD_ROW));
        Some(vec![
            form("h"),
            form("he"),
            form("hel"),
            form("hell"),
            form("hello"),
        ])
    });
    let (r, _) = act(
        &d,
        json!({
            "action": {"kind": "type", "into": {"text": "Field"}, "text": "hello"},
            "next": third()
        }),
    )
    .await;
    assert_eq!(r["verified"], true, "{r}");
}

/// An iPhone sends no frames, so nothing says when the screen last drew: a step there waits as it did.
#[tokio::test(start_paused = true)]
async fn a_device_that_sends_no_frames_waits_as_it_did() {
    let mut times = Vec::new();
    for with_next in [false, true] {
        let ticking: Vec<RawTree> = (0..60u32)
            .rev()
            .map(|n| tree(&["One", "Two", "Three", &format!("00:{n:02}")]))
            .collect();
        let fake = Arc::new(
            FakeDevice::new(ticking).lacking(phonedriver_core::device::Capabilities::FRAMES),
        );
        let home = tempfile::tempdir().unwrap();
        let d = Daemon::new(
            Arc::new(OneDevice(fake.clone())),
            Config {
                home: home.path().to_path_buf(),
            },
        );
        seen(&d).await;
        let mut params = tap_then(third());
        if !with_next {
            params.as_object_mut().unwrap().remove("next");
        }
        let (_, took) = act(&d, params).await;
        times.push(took);
    }
    assert_eq!(times[0], times[1], "{times:?}");
}

/// A batch knows its next step as a replay does, so its steps start from the same two reads.
#[tokio::test(start_paused = true)]
async fn a_batch_step_before_a_tap_does_not_wait_for_a_ticking_text_either() {
    let mut times = Vec::new();
    for second in [
        json!({"point": {"x": 10.0, "y": 330.0}}),
        json!({"text": "Three"}),
    ] {
        let ticking: Vec<RawTree> = (0..90u32)
            .rev()
            .map(|n| tree(&["One", "Two", "Three", &format!("00:{n:02}")]))
            .collect();
        let fake = Arc::new(FakeDevice::new(ticking));
        let home = tempfile::tempdir().unwrap();
        let d = Daemon::new(
            Arc::new(OneDevice(fake.clone())),
            Config {
                home: home.path().to_path_buf(),
            },
        );
        seen(&d).await;
        let drawn = fake.clone();
        let draws = tokio::spawn(async move {
            let mut taps = 0;
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let now = drawn.calls().len();
                if now > taps {
                    taps = now;
                    tokio::time::sleep(Duration::from_millis(90)).await;
                    drawn.push_frame(frame(THIRD_ROW));
                }
            }
        });
        let began = tokio::time::Instant::now();
        let steps = json!([
            {"kind": "tap", "target": {"point": {"x": 10.0, "y": 10.0}}},
            {"kind": "tap", "target": second}
        ]);
        let r = d
            .call("default", "batch", json!({"steps": steps}))
            .await
            .unwrap_or_else(|e| panic!("batch failed: {e:?}"));
        times.push(began.elapsed());
        draws.abort();
        assert_eq!(r["steps"].as_array().map(Vec::len), Some(2), "{r}");
        assert_eq!(fake.calls().len(), 2, "{r}");
    }
    assert!(
        times[0] >= times[1] + Duration::from_millis(600),
        "{times:?}"
    );
}
