# Phase 0: Foundation

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task.

**Goal:** A Rust workspace with CI and the pure domain core (`phone-use-core`) fully tested before any device code exists.

**Architecture:** `docs/architecture.md` sections 3 and 4. Core has no IO. Everything here is testable on the MacBook alone.

**Tech Stack:** Rust stable (rustc 1.99.1 on this Mac), `blaze` for build/test, `serde`, `serde_json`, `thiserror`, `bitflags`, `proptest`, `insta`, `assert_cmd`.

Use `blaze build`, `blaze test`, `blaze check` instead of cargo (user rule). Use `cargo add` to add dependencies.

---

### Task 0.1: Workspace skeleton

**Files:**
- Create: `Cargo.toml` (workspace, `resolver = "3"`, members `crates/*`, shared `[workspace.package] version = "0.1.0"`, `edition = "2024"`, `[workspace.lints]`)
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`
- Create: `crates/video/…`, `crates/android/…`, `crates/ios/…` as empty libs (`lib.rs` with nothing)
- Create: `crates/cli/Cargo.toml` (`[[bin]] name = "phone-use"`), `crates/cli/src/main.rs`
- Create: `crates/cli/tests/cli.rs`
- Create: `.gitignore` (`/target`, `/spikes/*/build`, `*.jar`, `.DS_Store`, `/testbeds/android/build`, `/testbeds/android/.gradle`, `/ios-runner/build`)

**Step 1: Write the failing test.** `crates/cli/tests/cli.rs`: run the binary with `version`; assert stdout is exactly `phone-use 0.1.0\n` and exit 0. Second test: no args prints usage to stderr and exits 2.

**Step 2: Run it.** `blaze test -p phone-use` → FAIL (binary prints nothing).

**Step 3: Minimal implementation.** `main.rs` matches `std::env::args().nth(1)`: `"version"` prints `phone-use {CARGO_PKG_VERSION}`; otherwise usage to stderr, exit 2. No arg-parsing crate yet; `clap` arrives in Phase 2 when there are real verbs.

**Step 4: Run it.** Same command → PASS.

**Step 5:** `blaze pin` in the repo root. Commit `chore: workspace skeleton`.

### Task 0.2: Lints, format, CI

**Files:**
- Modify: `Cargo.toml` `[workspace.lints.rust] unsafe_code = "deny"`; `[workspace.lints.clippy] all = "deny"`. Each crate: `[lints] workspace = true`.
- Create: `rustfmt.toml` (`max_width = 100`)
- Create: `.github/workflows/ci.yml`: matrix macOS, Ubuntu, Windows; steps `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`. CI uses cargo because blaze is a local daemon.

**Step 1:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings` → PASS. CI cannot run until a remote exists (one-way door); note that in `progress.md`. Also add to the root `Cargo.toml` `[workspace.metadata.blaze.sandbox] unsandboxed-build-scripts = ["phone-use-android"]` now: Phase 2's `crates/android/build.rs` runs `javac` and `d8` from outside the workspace, which blaze's sandbox blocks.

**Step 2:** Commit `chore: lints and CI`.

### Task 0.3: Geometry

**Files:** Create `crates/core/src/geometry.rs`; test in the same file under `#[cfg(test)]`.

**Step 1: Failing tests.**
- `Rect::center()` of `{x:10,y:20,w:100,h:40}` is `(60,40)`.
- `Rect::clip(screen)` of a rect half off-screen returns the visible half; fully off-screen returns `None`.
- `Scale::to_device(point)` with density 2.625 maps `(100,200)` to `(262.5,525)` and `to_points` inverts it (property test: round-trip within 0.01 for any point in 0..10_000).
- `Scale::to_hid(point, screen)` maps `(0,0)` to `(0,0)` and `(screen.w, screen.h)` to `(65535,65535)`.

**Step 2:** `blaze test -p phone-use-core geometry` → FAIL (types missing).

**Step 3: Implement.** `Point { x: f32, y: f32 }`, `Size`, `Rect`, `Scale { density: f32 }` with `to_device`, `to_points`, `to_hid`. `#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]`.

**Step 4:** Same command → PASS. Commit.

### Task 0.4: Element model and RawNode parsing

**Files:** Create `crates/core/src/element.rs`, `crates/core/src/raw.rs`, `crates/core/tests/fixtures/android/handmade_settings.json`.

**Step 1: Failing tests.**
- Parse `handmade_settings.json` into `RawTree`. Write it by hand from the `RawNode` table in `docs/protocol.md` section 2, shaped like a real Settings screen: one window; a RecyclerView (`coll` 12×1, `scroll_down` action); four clickable LinearLayout rows with **no text of their own**, each holding a title TextView and a summary TextView; one row that also contains a `Switch` with `sd: "On"`; a search box with `hint`. Assert node count, parent links, every flag letter and every key decoded.
- Unknown flag letters fail with an error naming the letter (parse, don't validate).
- `Role::from_android_class("android.widget.Switch")` is `Switch`; `"androidx.recyclerview.widget.RecyclerView"` is `List`; an unknown class is `Other`. Table-driven test with 25 classes.
- `Role::from_xcui("XCUIElementTypeButton")` is `Button`. 20 types.

**Step 2:** `blaze test -p phone-use-core element raw` → FAIL.

**Step 3: Implement.** Types from architecture 4.2. `RawTree { windows: Vec<RawWindow>, nodes: Vec<RawNode> }`. The role tables are two `match` blocks, one per platform. No trait for "platform mapper".

**Step 4:** PASS. Commit.

### Task 0.5: Ref assignment

**Files:** Create `crates/core/src/refs.rs`.

**Step 1: Failing tests.**
- First tree: refs `e1..eN` in preorder of kept nodes.
- Same tree again: identical refs.
- Tree with one row inserted at the top of a list: every old row keeps its ref; the new row gets `eN+1`.
- Tree where an element's label changes but its Android source id and bounds are the same (a counter label updating): keeps its ref.
- Recycled row: before, node id `n:7` is "Row 12" at y=300; after a scroll, `n:7` is "Row 20" at y=900. "Row 20" gets a **new** ref and "Row 12"'s ref is gone (or follows "Row 12" if it is still on screen under another id). This is the RecyclerView case; getting it wrong taps the wrong row silently.
- iOS tree (no source ids) with rows reordered: refs follow labels.
- Property (`proptest`): for random trees and random edit scripts (insert, delete, move, relabel), no ref is attached to two live elements; unchanged elements keep refs; refs are never reused after removal.

**Step 2:** FAIL.

**Step 3: Implement.** `RefTable { next: u32, by_identity: HashMap<Identity, Ref>, last: Vec<(Ref, StructuralKey, Rect)> }`. `assign(&mut self, tree: &RawTree) -> Vec<Ref>`. Matching order from architecture 4.3. Ties on structural key: the candidate with the smallest center distance.

**Step 4:** PASS. Commit.

### Task 0.6: Snapshot rendering

**Files:** Create `crates/core/src/render.rs`; snapshots under `crates/core/src/snapshots/`.

**Step 1: Failing tests.** `insta::assert_snapshot!` on the handmade fixture for: default, `--interactive`, `--within @e2`, `--max-lines 3`. Unit tests:
- Label merging (architecture 4.4 rule 2): each unlabeled clickable row renders as one line `@eN button "Network & internet" "Mobile, Wi‑Fi, hotspot"`; its TextViews do not render separately; the Switch inside a row renders as its own `switch … on` line.
- Target resolution: text `"Network & internet"` resolves to the row's ref, not the TextView. Text that matches only an inert label with no actionable ancestor resolves to the label's own center.
- A 300-char label truncates to 200 + `…(+100 chars)`; a `SecureField` value renders `•••`; a single-child unlabeled container collapses; a zero-area node is dropped; header line includes `loading` when a `ProgressIndicator` is visible; a list with `scroll_down` only renders `scrollable↓`.

**Step 2:** FAIL. **Step 3:** Implement the pruning rules and line grammar from architecture 4.4. **Step 4:** `cargo insta review` and read every snapshot yourself before accepting. PASS. Commit.

### Task 0.7: Diff

**Files:** Create `crates/core/src/diff.rs`.

**Step 1: Failing tests.** Toggle: one `~ @e10 switch "Dark theme" off → on` line. Navigation (70% refs changed): returns `Full`. Removals of `@e1..@e5` collapse to `- @e1 @e2 @e3 @e4 @e5`. Value edit on a text field shows old and new values, except `SecureField`, which shows `~ @e3 secure field changed`.

**Step 2:** FAIL. **Step 3:** `diff(prev: &Rendered, next: &Rendered) -> DiffOrFull`. **Step 4:** PASS. Commit.

### Task 0.8: Settle detector

**Files:** Create `crates/core/src/settle.rs`.

**Step 1: Failing tests.** The detector takes `feed(Input, now)` and `poll(now) -> State`. Defaults from architecture 4.6: `expect_change = 500ms`, `quiet = 250ms`, `significant_area = 4096 pt²`, `timeout = 5s`.
- After an action, no inputs at all: state at 500 ms is `Done { settled: true, changed: false }`. Not at 250 ms: the first-change window must elapse.
- After an action, a frame with 20,000 pt² changed at 400 ms, then quiet: `Done { settled: true, changed: true }` at 650 ms.
- `launch` mode (`expect_change = 3000ms`): first change at 1,800 ms (cold start), settled at 2,050 ms.
- Snapshot mode (no first-change phase): no inputs, settled at 250 ms.
- `WindowChanged` at 100 and 300 ms: settled at 550 ms.
- There is no content-change input type at all (architecture 4.6 explains why); a ticker cannot block settle by construction.
- Frames every 33 ms with 1,000 pt² changed (caret, spinner): never significant; settled as if quiet.
- Frames with 50,000 pt² changed forever: at 5 s, `Done { settled: false, still_changing: <union of boxes> }`, which is a value, not an error.
- Property: a settled result is never earlier than the last significant input + quiet, and never later than timeout.

**Step 2:** FAIL. **Step 3:** Pure struct, no clock reads inside; `now: Duration` is passed in. **Step 4:** PASS. Commit.

### Task 0.9: Errors

**Files:** Create `crates/core/src/error.rs`.

**Step 1: Failing tests.** Every variant serializes to `{code, code_name, message, hint}` with the codes in `docs/protocol.md` section 1. `StaleRef` hint names `phone-use snapshot` and the closest element line. `NeedsUser(UnlockDevice)` message tells the user to unlock and says phone-use never enters passcodes. Table test over all `Error` variants and all `UserAction` variants (architecture 4.7) checks no message is empty and every hint starts with a verb.

**Step 2:** FAIL. **Step 3:** `thiserror` enum from architecture 4.7, plus `fn rpc_code(&self) -> i32` and `fn hint(&self) -> String`. **Step 4:** PASS. Commit.

### Task 0.10: The Device port and the fake

**Files:** Create `crates/core/src/device.rs`, `crates/core/src/testing.rs` (behind `features = ["testing"]`).

**Step 1: Failing test.** In `testing.rs` tests: a `FakeDevice` loaded with a sequence of `RawTree`s returns them in order from `tree()`; `gesture()` records the gesture; `events()` delivers scripted events. This is the double that daemon tests use in Phase 2.

**Step 2:** FAIL. **Step 3:** The trait exactly as architecture 4.8; `Capabilities` bitflags; `async-trait`; `tokio::sync::{watch, broadcast}`. **Step 4:** PASS. Commit.

---

## Phase gate

- `blaze test` green, including property tests with the default 256 cases.
- `cargo clippy --all-targets -- -D warnings` clean.
- `progress.md` updated with the commands and outputs.
