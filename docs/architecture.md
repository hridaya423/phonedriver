# phone-use architecture

phone-use lets an AI agent see and operate a real or virtual phone or tablet: Android and iOS, USB or network, device or emulator. This document is the design. `docs/decisions.md` records why each choice beat its alternatives. `docs/protocol.md` defines every wire format. Terms are defined in `docs/GLOSSARY.md`.

## 1. What the agent experiences

The design starts here. Everything below exists to make this loop fast, exact, and hard to misuse.

```console
$ phone-use snapshot
device android:emulator-5554 · Pixel 9 · Android 17 · 412x915pt · portrait
app com.android.settings (Settings) · keyboard hidden · settled in 212ms
@e1 searchbox "Search settings"
@e2 list scrollable↓
  @e3 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  @e4 button "Connected devices" "Bluetooth, pairing"
  @e5 button "Apps" "Recent apps, default apps"
  @e6 button "Display & touch" "Dark theme, font size, brightness"

$ phone-use tap @e6
ok tapped @e6 "Display & touch" at 206,488 · settled in 340ms · new screen
app com.android.settings (Settings) · keyboard hidden
@e9 heading "Display & touch"
@e10 switch "Dark theme" off
@e11 button "Brightness level" "64%"

$ phone-use tap @e10
ok tapped @e10 "Dark theme" · settled in 410ms
~ @e10 switch "Dark theme" off → on
```

Properties of that loop:

1. **One call per step.** Every action returns its own outcome and what changed, after the screen settles. The agent never has to poll or sleep.
2. **Refs, not guesses.** `@e10` names an element. The daemon re-locates it at action time, so a list that shifted by a row still gets the right tap. A ref that no longer exists fails with the closest current match.
3. **Diffs, not dumps.** Within a screen, the agent sees `+`, `-`, `~` lines: 1 to 20 lines instead of 100+. A new screen comes back as a full snapshot.
4. **Honest results.** `type` reports whether the field now holds the text. `back` on iOS reports how it went back. Unsupported operations say so with a reason.
5. **Same commands on every device.** Platform differences surface as capabilities and as `via` fields, never as different verbs.

## 2. System shape

```
 agent ──CLI──┐                                ┌── Android device ── phone-use-agent (app_process, shell uid)
 agent ──MCP──┼─ JSON-RPC over local socket ─ phone-use daemon ──┼── iOS 27+ device ── CoreDevice services (no app)
 SDK  ────────┘                                │                 └── iOS ≤26 device / Simulator ── PhoneUseRunner (XCTest)
                                               └── per device: DeviceActor { connection, FrameStore, Session state }
```

One Rust binary, `phone-use`, plays three roles selected by subcommand:

- `phone-use <verb>`: the CLI. A thin client. It connects to the daemon, sends one request, prints the reply.
- `phone-use daemon`: the long-lived process that owns every device connection. Started on demand by the CLI.
- `phone-use mcp`: an MCP server on stdio. Also a thin client of the daemon.

The daemon exists because the expensive things are persistent: the on-device agent, the iOS tunnel, the video decoder, the tree cache, ref identity. A per-call process (the competitor's model) pays all of that every time.

## 3. Repository layout

```
phone-use/
  Cargo.toml                    workspace
  crates/
    core/      phone-use-core   pure domain: elements, refs, snapshot render, diff, settle, geometry, errors
    video/     phone-use-video  H.264/HEVC decode to FrameStore (RGBA + luma thumbnail)
    android/   phone-use-android adb server client, agent deploy, agent protocol client, AndroidDevice
    ios/       phone-use-ios    idevice transport, CoreDevice tier, runner client, simulator, IosDevice
    cli/       phone-use        the binary: cli, daemon, mcp, rpc
  android-agent/                Java sources for the on-device agent, built to phone-use-agent.jar
  ios-runner/                   XcodeGen project for PhoneUseRunner (XCTest UI test bundle)
  testbeds/android/             Gradle app with ground-truth UI and event logging
  testbeds/ios/                 XcodeGen SwiftUI app with ground-truth UI and event logging
  bench/                        workspace member `phone-use-bench` (publish = false): latency, scenarios, baselines
  NOTICE                        attribution for code copied from scrcpy and WebDriverAgent
  docs/
```

Why these crate boundaries (each must pay rent):
- `core` is pure and has no IO. It holds the logic that decides correctness (ref matching, diff, settle, rendering) so it can be property-tested without devices.
- `video` has two consumers (Android and iOS streams).
- `android` and `ios` are the two implementations of the `Device` port. Their dependency trees are disjoint (`idevice` is large).
- `cli` is the only binary. Daemon, CLI and MCP share request types, so they live together.

No other shipped crates (`bench` is a dev-only binary). A new crate needs a second consumer or a disjoint dependency tree. OCR engines live in `cli` because they are IO; the pure merge logic lives in `core`.

## 4. Domain model (`phone-use-core`)

### 4.1 Coordinates

One coordinate space everywhere: **logical points** of the current orientation, origin top-left. iOS points; Android dp (`px / density`). Screenshots default to 1 pixel per point, so a vision model's pixel guess is directly tappable. `--native` returns device pixels with the scale in the reply.

Types: `Point { x: f32, y: f32 }`, `Rect { x, y, w, h }`, `Size`. Conversions to device units happen only inside each device implementation (Android px, iOS HID 0..65535 normalized).

### 4.2 Elements

```rust
pub struct Element {
    pub r#ref: Ref,                 // "e12"; stable for the life of the session while the element exists
    pub role: Role,                 // closed enum, see below
    pub label: Option<String>,      // accessible name: text, content-desc, accessibilityLabel
    pub value: Option<String>,      // field text, slider value, switch state text
    pub hint: Option<String>,       // placeholder (hintText / placeholderValue) or merged subtitle text (4.4)
    pub id: Option<String>,         // resource-id / accessibilityIdentifier
    pub bounds: Rect,               // logical points, clipped to the screen
    pub state: ElementState,        // bitflags: ENABLED FOCUSED SELECTED CHECKED CHECKABLE EXPANDED COLLAPSED PASSWORD
    pub actions: Actions,           // bitflags: TAP LONG_PRESS EDIT SCROLL_UP SCROLL_DOWN SCROLL_LEFT SCROLL_RIGHT
    pub position: Option<ItemPos>,  // row/col of this item and its list's size, from collection info
    pub source: Source,             // Tree | Ocr
    pub children: Vec<ElementIdx>,
}
```

`Role` is a closed enum: `Button, Link, Text, Heading, TextField, SearchField, SecureField, Switch, Checkbox, Radio, Slider, Image, List, Cell, Tab, Menu, MenuItem, Dialog, Keyboard, WebView, Alert, ProgressIndicator, Other`. Each platform maps its class names and traits once, in one table: `Role::from_android_class` and `Role::from_xcui` in `core::element`. They live in `core` so fixtures from both platforms parse and render in core tests without device crates. `core::raw` parses each platform's wire tree into the neutral `RawTree` (label, value and hint already chosen per platform, bounds in points). Unknown classes become `Other` and are kept only if they carry text or actions.

### 4.3 Refs

A ref is assigned the first time an element is seen in a session and is never reused for a different element in that session (monotonic counter).

Matching a new tree to the previous one (`core::refs::assign`), in order:
1. Platform identity, only with continuity: Android `(windowId, sourceNodeId)` matches only if the role is the same **and** either the label is unchanged or the bounds moved less than 8 points. Recycled list rows (RecyclerView, Compose lazy lists) reuse the same view and node id for a different item after a scroll; without this check `@e12` "Row 12" would silently become "Row 20" and a tap would hit the wrong row. `sourceNodeId` comes from the hidden `AccessibilityNodeInfo.getSourceNodeId()` via reflection; if spike A1 shows it unavailable, step 1 is skipped. iOS snapshots have no stable id.
2. Structural key: `(role, id, label)` plus nearest-ancestor key, with the closest bounds when several match; then the same key without the ancestor, so a wrapper group that appears or disappears around an element (iOS Settings inserts a `Toolbar` group a moment after launch, which made `@e26` stale at once in the agent-experience audit) does not change its ref.
3. App id alone: an element whose app-given id (`accessibilityIdentifier`, resource id) is unique in both trees keeps the ref of the element with that id and role, even when its label changed. iOS snapshots have no platform identity, so without this a status text that changes its label ("Last tap: none" to "Last tap: b17") would print as a removal and an addition instead of a `~` line. Ids shared by several elements (every row of a list named `cell`) are skipped, because matching them by place would hand a row's ref to the next item scrolled into that place.
4. Otherwise a new ref.

Invariants, each a property test:
- An element whose identity, role, id and label are unchanged keeps its ref.
- A node id whose label changed and whose bounds jumped is a new element and gets a new ref (recycling).
- No ref ever refers to two different live elements.
- `assign` is deterministic for the same input.

At action time, `@eN` resolves against a fresh tree. If the element moved, the action uses the new bounds. If it is gone, the call fails with `StaleRef` and names the best current candidate.

### 4.4 Snapshot rendering

Pruning rules (`core::render`), applied in this order:
1. Drop invisible nodes: zero area, fully off-screen, or `isVisibleToUser == false`.
2. **Label actionable containers from their text.** On Android the clickable node is usually an unlabeled layout whose text sits in non-actionable `TextView` children (every Settings row). For a node with actions and no label of its own: collect the text of descendants that have no actions of their own, in preorder; the first becomes `label`, the rest join into `hint` with " · ". Those absorbed descendants are not rendered separately. A descendant with its own actions (a switch inside a row) stays a separate element. A node with actions **and** a label of its own absorbs, without merging, the inert descendants that are images or repeat its label (an iOS tab button lists its icon and its title as children; a screen reader reads the button as one element).
2b. **One control, one line.** (An unlabeled tappable `Cell`/`Other` wrapper that holds exactly one enabled labeled `Button` and no text of its own is dissolved the same way: iOS Settings rows are an unlabeled cell around one button.) An iOS row is a `cell` with a button of the same label inside it, and both take the tap (observed in the agent-experience audit: `tap "Display & Text Size"` was ambiguous between `@e451 cell` and `@e452 button`, and every row cost two or three lines). A labeled, tappable `Cell` or unclassified (`Other`) node that has a tappable `Button` descendant with the identical label is dropped and the button stays (innermost wins, and a text target resolves to it); any other control (a switch, a button) keeps its own line and absorbs the same-label button inside it. Two separate buttons that share a label are left alone.
3. Collapse containers that have no label, no actions, and one child.
4. Keep a node if it has actions, has text, is a `TextField`/`SecureField`, or is a scrollable container.

Reading order (task 2.7): siblings print top to bottom by vertical center in 8-point bands, then left to right, because the agent lists a window's views in z order (an app bar comes after the list it overlaps); windows keep the agent's order, topmost first (a dialog before the app under it), except the keyboard window, which prints last.

Text-target resolution follows the same rule: `tap "Network & internet"` matches the row whose merged label is that text, so the tap goes to the actionable ancestor, never to an inert text node. If text only matches a non-actionable node, the tap goes to its nearest actionable ancestor; if there is none, to the text's own center.

The keyboard window is one `keyboard` node without its keys (task 3.2): an agent types with `type`, and about sixty key nodes made the first typed character a "new screen" in the diff (more than 60% of refs changed). The same holds on iOS since the agent-experience audit (it measured 26 to 40 `button "q"` lines, about 190 tokens, per keyboard appearance): `ios::map::tidy` keeps only the `Keyboard` node of the window that holds the keyboard, as a child of the application, and drops that window's keys and its Emoji and Dictate buttons. A key is therefore not a tap target by name; `press enter`, `type --submit` and `press hide-keyboard` cover what keys were used for.

Covered and scrollable (agent-experience audit): a tap on an element whose center lies inside a floating `Toolbar` group that is neither its ancestor nor its child fails `OffScreen` with `covered_by: "Toolbar"` and a scroll hint (down for a bar on the lower half of the screen), instead of tapping the toolbar (the iOS 26 search bar covers the last Settings rows and `tap Developer` reported success twice); `scroll --until` keeps scrolling while its target is under such a bar. An XCUI scroll view has no scroll actions, but its indicator (`Vertical scroll bar, 2 pages`, value `48%`) says where it is: `ios::map::tidy` turns it into `scroll_up`/`scroll_down` (left and right for a horizontal one) on its owner, only when it has more than one page, and into the value `2 pages`, so an iOS list prints `@e25 list "2 pages" scrollable↓`. iOS does not report row counts; the page count is what it knows.

Compose and WebView (task 3.10): both build their accessibility tree on the first query, about a second after the view appears, and show as a childless node until then, so the agent's first read of each `WebView` or `AndroidComposeView` waits up to 2.5 s for children. A Compose toggle (`Switch`, `Checkbox`, `RadioButton`) is an `android.view.View` that is checkable and takes the tap, with its role in the `AccessibilityNodeInfo.roleDescription` extra of a child node (`rd` on the wire, protocol section 2); the parser gives the checkable parent that role. A Compose `testTag` is the node's `id` when the app sets `testTagsAsResourceId`. `isHeading` exists from API 28, so a Compose title is a `heading` only from there. A text target that matches an inert element and exactly one element with actions (a web label `Name` next to its input `Name`) resolves to the one with actions.

Text limits: truncate text above 200 chars as `…(+N chars)`. `SecureField` values render as `•••` and are never logged.

Line grammar: `@eN role "label" ["value"] [state…]`, indented two spaces per kept depth. The second quoted string is the value, or the hint when there is no value (a row's merged subtitle, a field's placeholder). A checkable element's value is omitted when its `on`/`off` word already says it. Role words: `button link text heading textbox searchbox password switch checkbox radio slider image list cell tab menu menuitem dialog keyboard webview alert progress group` (`group` is `Other`). An unlabeled `Other` with tap or long-press renders as `button`. States render as words: `off`, `on`, `disabled`, `focused`, `selected`, `scrollable↓`, `scrollable↑↓`, `under-keyboard`.

Header lines: device, app, keyboard state, orientation, settle time, and `loading` when a visible `ProgressIndicator` exists or a `WebView` has no children yet (a childless `WebView` is kept as a line for that reason).

Filters: `--interactive` (only nodes with actions), `--within @eN`, `--max-lines N` (summarizes the rest as `… N more`).

Golden tests (`insta`) render fixture trees captured from real devices in Phases 2 and 5.

### 4.5 Diff

`core::diff(prev, next) -> Vec<Change>` where `Change` is `Added(ref)`, `Removed(ref)`, `Changed { ref, field, from, to }`. Rendered as `+`, `-`, `~` lines; consecutive removals collapse to one line. When more than 60% of refs changed ((added + removed) / refs in either observation), the reply is a full snapshot instead of a diff, because the agent is on a new screen. Line shapes: `- @e1 @e2` (removed refs only), `+ <element line>`, `~ @eN role "label" <from> → <to>` per changed field (label, value, hint, state words). A secure field's value change renders `~ @eN secure field changed` and the `Change` value carries no text.

### 4.6 Settle

"Settled" means the UI stopped changing in a way that matters. Inputs, each timestamped:
- `WindowChanged`: Android `TYPE_WINDOW_STATE_CHANGED` and `TYPE_WINDOWS_CHANGED` (a new activity, dialog, popup, or IME window), debounced on the device.
- `Frame { changed_area_pt2, changed_box }`: from the FrameStore, comparing each frame's luma thumbnail (one sample per 4×4 points, so the measure is the same on a phone and a tablet) with the previous one; a sample counts as changed above 12 luma levels; `changed_area_pt2` is the changed sample count × 16.

Content-change accessibility events are deliberately not an input. Apps emit them for invisible changes and for tickers (a clock label every 100 ms would never settle). Pixels are the ground truth for "the screen moved". Every snapshot fetches a fresh tree, so no event is needed to invalidate a cache.

`core::settle::Detector` is a pure state machine with two phases:
1. **Wait for the first change.** After an action, a tap that starts a cold activity or a network load can show nothing for hundreds of milliseconds. Returning "settled, nothing changed" then would make the agent tap again. So the detector first waits up to `expect_change` (default 500 ms; 3,000 ms for `launch` and `open`) for a first input: a `WindowChanged`, or a frame with `changed_area_pt2 > first_change_area` (default 128 pt²). If none arrives, the reply says `changed: false` and the hint suggests `wait`. `launch` and `open` first wait up to 3 s for an application window with children, a stable node count (on a polled device without `FRAMES`, the same count on three reads in a row: iOS Settings adds a `Toolbar` group 300 ms to 1 s after its first frame), no `WebView` without children (`open https://example.com` returned a blank `webview` before) and input focus (observed in the M2 scenarios: a cold-started Settings on API 37 reported an empty tree for about a second after its first frame, so the next `tap` found nothing).
2. **Wait for quiet.** Then settled when no `WindowChanged` and no frame with `changed_area_pt2 > significant_area` (default 4,096 pt², a 64×64 point region) for `quiet` (default 250 ms). A visible `ProgressIndicator` is reported as `loading` in the header instead.

Why two thresholds (spike A3, 20 runs per case on API 26 and 37, `docs/spikes/android.md`): a blinking caret changes at most 96 pt², a 48 dp indeterminate spinner at most 480 pt², a Material switch toggle at most 432 pt², and a screen transition 13,000 to 254,000 pt². A single 4,096 pt² threshold would never see a toggle at all and report `changed: false` for a tap that worked. 128 pt² is just above the caret, so a toggle counts as a first change; 4,096 pt² for quiet ignores the spinner and the caret.

`--expect-change <ms>` widens the first-change window for one call, for slow links and slow apps (the emulators behind an SSH tunnel show a frame 300 to 500 ms after an input). An action sent without settling remembers when it was sent; the next `wait` or settling `snapshot` that comes within that window waits for the action's first change instead of treating the screen as already quiet.

Known limit: the detector reads a gap of `quiet` with no significant frame as quiet. A video stream that delivers frames in bursts (the software encoder on the emulators during sustained full-screen motion, or a congested link) can show gaps over 250 ms in the middle of an animation, and the screen is then reported settled early. Phones with hardware encoders have not been measured (no phone provided); requiring a frame after the quiet period as proof of a live stream would cost up to 330 ms on a static screen.

At `timeout` (default 5 s for actions, 2 s for `snapshot`) the detector stops with `settled: false` and `still_changing` (the union of recent `changed_box`es). This is a normal reply with the observation attached, not an error: looping video and carousels never settle and the agent still needs to see the screen. Only `wait settled` turns it into the `NotSettled` error.

`snapshot` also waits for quiet (phase 2 only) unless `--no-settle`. The actor watches frames and window events for as long as it holds a device, so a snapshot of a screen that has already been quiet for `quiet` returns at once; only recent activity is replayed into the detector.

Tested with a simulated clock and scripted event sequences. No sleeps in tests.

Devices without a frame stream (iOS ≤26, Simulator) feed the detector by polling screenshots. As built (task 5.5), the polling lives in the device, not the session: `IosDevice` asks the runner for a gray `luma` screenshot at 0.25 pixels per point (one sample per 4×4 points, the same measure as the video thumbnail) and publishes it into a `FrameStore`, so `frames()` returns `Some` and the session code is the same for both platforms. Each poll costs about 75 ms on the Simulator (61 ms of it is XCTest's capture, whatever the output size). It polls back to back for 3 s after any input or visible change (above 128 pt²) and every 500 ms otherwise; it never polls between the requests of one input (the tree read and the tap of `back`), because the runner serves one request at a time and an extra poll there pushed the first frame after the input past the 500 ms first-change window (3 of 10 `press back` replies before, 0 of 10 after).

On such a device the first-change window starts when the input call returns, not when the request arrives. An XCTest event is confirmed about 230 ms after the app already received it, so a window counted from the request was half spent before the device could report anything: `tap Developer` in Settings settled at 501 ms with the push transition half drawn. Devices with a frame stream (Android, CoreDevice tier) keep counting from the request, because their frames arrive while the input is in flight. The capability that decides it is `FRAMES`: `IosDevice` publishes polled frames but does not claim it. On a polled device, `settle_ms` counts from delivery.

### 4.7 Errors

One enum, every variant written for an agent to act on:

```rust
pub enum Error {
    NoDevice { connected: Vec<DeviceSummary> },
    AmbiguousDevice { connected: Vec<DeviceSummary> },
    DeviceGone { id: DeviceId },
    DeviceBusy { id: DeviceId, held_by: SessionName },
    NeedsUser { action: UserAction },          // see UserAction below
    StaleRef { r#ref: Ref, closest: Option<ElementLine> },
    NotFound { query: String, similar: Vec<ElementLine> },
    Ambiguous { query: String, matches: Vec<ElementLine> },
    OffScreen { r#ref: Ref, scroll_hint: Direction },
    Unsupported { op: Op, reason: &'static str },
    NotSettled { waited_ms: u32, still_changing: Rect },
    Timeout { waited_ms: u32, condition: String },
    NotReached { query: String, scrolls: u32, end: ScrollEnd, last: Option<String> }, // a scroll search stopped: Ended, Stuck or Limit
    AutomationConflict { holder: String },     // another UiAutomation client owns the bridge
    Transport { detail: String },              // the only catch-all, for IO failures we have not classified
}
```

`UserAction` is closed: `UnlockDevice`, `AcceptUsbDebugging`, `EnableUsbDebugging`, `AllowXiaomiInjection`, `TrustComputer`, `EnableDeveloperMode`, `MountDeveloperImage`, `ResignRunner`, `ApproveSystemPrompt { text }` (a biometric or OS prompt only a person may answer). Each has one message that tells the user exactly what to do on which device.

Every variant serializes as `{ code, message, hint }`. `hint` names the next command to run.

### 4.8 The `Device` port

The seam between the domain and each platform. Two production implementations plus a test fake.

```rust
#[async_trait]
pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;                       // id, model, os, screen, capabilities
    async fn tree(&self) -> Result<RawTree>;              // Unsupported in pixels-only tier
    fn frames(&self) -> Option<watch::Receiver<FrameRef>>;// None when no stream (polling tier)
    async fn screenshot(&self, q: Quality) -> Result<Image>;
    fn events(&self) -> broadcast::Receiver<DeviceEvent>; // WindowChanged, DisplayChanged, Locked, Gone
    async fn gesture(&self, g: Gesture) -> Result<()>;    // fingers with timed paths, played on or near the device
    async fn key(&self, k: Key) -> Result<()>;
    async fn button(&self, b: Button) -> Result<()>;      // Home, Back, Recents, Notifications, Lock, VolumeUp, VolumeDown
    async fn open_url(&self, url: &Url) -> Result<()>;
    async fn text(&self, t: TextInput) -> Result<()>;     // Paste | PasteSecret | SetValue | Keystrokes
    async fn clipboard(&self, op: ClipboardOp) -> Result<Option<String>>;
    async fn apps(&self) -> Result<Vec<App>>;
    async fn launch(&self, app: &AppId) -> Result<()>;
    async fn foreground(&self) -> Result<Option<AppId>>;
    async fn capture_tree(&self) -> Result<Option<String>>; // default None: the platform's own tree JSON, for fixtures
    async fn back(&self) -> Result<Option<&'static str>>;   // default: button(Back), no `via`; iOS says nav-button or edge-swipe
}
```

`Capabilities` is a bitflag set in `DeviceInfo`. The session checks it before calling and returns `Unsupported` with a reason. No method fakes a capability.

`RawTree` is platform-neutral (`Vec<RawNode>` with role already mapped). Ref assignment and pruning happen in `core`, once, for both platforms.

## 5. Daemon (`crates/cli/src/daemon`)

- **Socket.** `$XDG_RUNTIME_DIR/phone-use/daemon.sock`, else `~/.phone-use/run/daemon.sock` (Unix), or the named pipe `\\.\pipe\phone-use-<user>` (Windows), via the `interprocess` crate. Directory mode 0700; on Windows the pipe's security descriptor grants only the current user. No TCP listener. `PHONE_USE_SOCKET` overrides the path. Remote use goes through `ssh -L` socket forwarding (zero network code, README). The `screenshot` method always saves the PNG on the daemon's machine and replies with its path; a client that asks with `inline: true` also gets the bytes in `data_b64`. The MCP tool always asks inline, so it returns the image over a forwarded socket (observed failing before: the MCP server read the daemon's path on its own disk). The CLI's `screenshot -o file` asks inline and writes the file locally; without `-o` it prints the path on the daemon's machine.
- **Start.** The CLI connects; on `ENOENT`/`ECONNREFUSED` it spawns `phone-use daemon --detach`, then waits for the daemon's readiness line on an inherited pipe (not a sleep), up to 3 s.
- **Version.** First message is `hello { version }`. On mismatch the CLI sends `shutdown` and starts its own version.
- **Idle exit.** No clients and no leases for 15 minutes.
- **DeviceActor.** One tokio task per device owns the connection, FrameStore, and the sessions on that device. Requests arrive on an mpsc channel. No locks around device state.
- **Sessions outlive connections.** Every CLI call is a new process and a new socket connection, so session state cannot belong to the connection. Each request names a session: `--session <name>`, else `PHONE_USE_SESSION`, else `default`. The session holds the ref table, the last rendered observation (the diff baseline), and the settle detector. An MCP server process uses one session named `mcp-<pid>` over one connection that it keeps for its whole life, and says `client: "mcp"` in `hello`. The daemon releases every session an `mcp` connection used on every device when that connection closes, so a client that exits or is killed before it can send `release` (observed with `claude -p`: the next agent was locked out for ten minutes) frees the device at once. CLI sessions are not released on close, since every CLI call is its own connection.
- **Leases belong to sessions.** A session acquires a device on its first device call. Another session gets `DeviceBusy` unless it passes `--share`. A lease ends on `release`, when an `mcp` connection closes, or after 10 minutes without calls. `DeviceBusy` and `devices` say how long the holder has been idle (`idle 3m, lease expires in 7m`); an agent cannot take a lease over, it waits or asks the user. Two agents on one machine should use different `--session` names; the default session is for one agent at a time.
- **Discovery.** Android: adb `host:track-devices-l` push stream. iOS: usbmuxd listen. Simulators: `xcrun simctl list devices -j booted` on demand.
- **Device ids.** `android:<serial>`, `ios:<udid>`, `sim:<udid>`, plus aliases in `~/.phone-use/config.toml`. With exactly one device connected, it is the default. Otherwise `AmbiguousDevice` lists them.
- **Artifacts.** Screenshots go to `~/.phone-use/tmp/<device>/<ulid>.png`. Unique names. Cleared on daemon start when older than 24 h.
- **Traces.** Per session, `~/.phone-use/traces/<ulid>.jsonl` (directory mode 0700, files 0600): one line per request with `method`, `params`, the reply's summary line, `ms` and the settle time. Typed text is replaced by its length, `{"secret_len": n}` with `--secret` and `{"text_len": n}` otherwise (D16). Text targets are replaced by their length, and observation text (which can hold messages or balances) is added, only when the daemon runs with `--trace-full` or `PHONE_USE_TRACE_FULL=1`. Each `screenshot` is copied next to the trace as `<ulid>-<n>.png` only with `--trace-screens` or `PHONE_USE_TRACE_SCREENS=1`. A running daemon keeps the options it started with, so a harness that wants them starts its own daemon under its own `PHONE_USE_HOME`. Traces older than 7 days are deleted on daemon start. Local only; nothing is uploaded.

## 6. Android

### 6.1 Talking to adb

`crates/android/src/adb.rs` speaks the adb server protocol on TCP (default `127.0.0.1:5037`, overridable with `ADB_SERVER_SOCKET=tcp:host:port`, the variable adb itself honors). No `adb` process per call. Services used:

| Service | Use |
|---|---|
| `host:track-devices-l` | Discovery |
| `host:transport:<serial>` then `shell,v2,raw:<cmd>` | Start the agent, one-off commands |
| `host:transport:<serial>` then `sync:` | Push the agent jar |
| `host:transport:<serial>` then `localabstract:<name>` | Open streams straight to the agent's socket. No port forwarding to manage. |

If the default local server is not running, the daemon runs `adb start-server` once and retries. It never does this when `ADB_SERVER_SOCKET` is set: a dead tunnel would otherwise start an empty local server on the tunnel's port and hide the real problem. The remote-server override is also the development loop for the Windows-hosted emulator (Section 11).

### 6.2 The on-device agent

`android-agent/` builds `phone-use-agent.jar` (dex inside a jar) with `javac` against `android.jar` and `d8`, the way `scrcpy/server/build_without_gradle.sh` does. Minimum API 26 (decision D8).

Launch: push with mode 0444 to `/data/local/tmp/phone-use-agent-<sha8>.jar`, where `<sha8>` is the first 8 hex digits of the jar's SHA-256 computed on the host. The jar is already deployed when that path exists (`ls`); no hashing on the device. The push goes to `<path>.part` and is `mv`ed into place, so an interrupted push cannot leave a truncated jar at the final path. Then, over a `shell,v2,raw:` stream:
`CLASSPATH=<jar> app_process / dev.phoneuse.agent.Main <version> <socket-name>` and write a 32-byte random token to its stdin.

- The host holds that shell stream open for the agent's whole life and drains its stdout and stderr (logs go to the daemon's log). adbd kills the child when the stream closes, which is what guarantees a crashed daemon never leaves an agent running.
- The agent listens on abstract socket `phone-use-<version>-<rand>`. Abstract sockets have no permissions: any app on the phone can connect. So the first request on each stream must carry the token, and the agent drops the connection otherwise. Without this, any installed app would get shell-uid input injection and a full screen reader.
- The host opens two streams: `control` and `video`. The agent also exits when both streams close or when nothing connects within 10 s.
- Before launch, the host kills agents of other versions with `pkill -f 'dev.phoneuse.agent.[M]ain'` (the jar path is in the environment, not the command line, so matching on the jar name would never match). The bracket matters: `shell,v2,raw:` runs the command as `sh -c "<command>"`, whose own command line contains the pattern, so the unbracketed form kills the shell that runs it and its `echo $?` never prints (spike A5, API 26 and 37).
- Measured (spike A5): closing the shell stream ends the agent in about 1.0 s (1054 and 1137 ms with our polling), not instantly; a jar pushed with mode 0444 loads on API 26 and 37 (0666 also loads for the shell user on the API 37 emulator, so 0444 is hygiene, not a requirement); a connection with a missing or wrong token is closed with no reply; the agent exits by itself 10 s after start when nothing connects.

Components (one Java file each where possible):

- **Main.** Version check, Looper thread, socket accept, shutdown.
- **Control.** Length-prefixed JSON requests and replies plus pushed events (`docs/protocol.md`). Uses `org.json`, which ships with Android; no dependencies.
- **Video.** Mirror the display into a MediaCodec H.264 encoder input Surface, configured as scrcpy's `video/SurfaceEncoder.java:318-332` does: `KEY_REPEAT_PREVIOUS_FRAME_AFTER = 100 ms` (so the first frame of a static screen is delivered and quality recovers after motion; repeated frames have zero change and do not disturb settle), `KEY_PRIORITY = 0`, `KEY_LATENCY = 1`, I-frame interval 10 s, 30 fps cap, 8 Mbps. Copy scrcpy's size alignment and `MAX_SIZE_FALLBACK` / downsize-on-error logic (`SurfaceEncoder.java:36,79-95,201`): hardware encoders reject some sizes, and 4K tablets exceed most encoders. Spike A3: both emulators' software encoder (`c2.android.avc.encoder`) emits Constrained Baseline by default (profile_idc 66, constraint flags 0xC0; level 5.0 at 1080x2424 on API 37, 4.1 at 1080x1920 on API 26) and `openh264` decoded every frame with no errors, so `KEY_PROFILE` stays unset. Hardware encoders on physical phones are not measured yet (Phase 3, if a phone is provided). A static screen yields about 3 frames per second (the 100 ms repeat is honored at roughly 330 ms by this encoder), so a stall is `no frame for 1 s`, not for 200 ms. A sync-frame request is honored only when a new frame is rendered (10 of 10 while the screen changed, 2 to 3 of 10 on a static screen), so a sharp image of a settled screen comes from the repeat frames, not from `sync_frame`. Restart on display size or rotation change (`display/DisplayMonitor.java`; the mechanism differs below API 34; the spike polls `DisplayInfo` every 100 ms and restarts the encoder, which gave a new codec header and the first frame at the new size 0.5 to 0.9 s after rotation, `wm size` and `wm size reset` on both APIs; screen off keeps frames flowing and the first new frame came 0.5 s after wake). A sync frame can still be requested (`PARAMETER_KEY_REQUEST_SYNC_FRAME`) to resynchronize a decoder, not to sharpen an image.
- **Input.** `InputManager.injectInputEvent` through reflection, `INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH` for the last event of a gesture so the reply means "delivered". Multi-pointer MotionEvents (`PointersState` pattern from scrcpy). Gestures arrive as one message with full timed paths and are played on the device with `SystemClock.uptimeMillis` scheduling, so network jitter cannot distort a fling.
- **Accessibility.** UiAutomation connected from the shell process (`UiAutomationShellWrapper` pattern) with `UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` (API 24). Without it, connecting silently turns off the user's own TalkBack, Switch Access, and password-manager autofill services while we run. Service flags: `FLAG_RETRIEVE_INTERACTIVE_WINDOWS | FLAG_INCLUDE_NOT_IMPORTANT_VIEWS | FLAG_REPORT_VIEW_IDS`. Construction (spike A1): `new UiAutomation(Looper, IUiAutomationConnection)` is public on API 26 and 37 and works on both (API 37 also has `(Context, conn)` and a private `(int displayId, Looper, conn)`); then the hidden `connect(int flags)` by reflection. Connect takes 12 to 14 ms on API 26 and 24 to 43 ms on API 37, and the first `getWindows()` can be empty for about 20 ms. Tree: call `UiAutomation.clearCache()` first (API 33+; on older levels `AccessibilityInteractionClient.getInstance().clearCache()` by reflection), because the node cache is invalidated by content-change events and we deliberately subscribe only to window events, so without it a tree read after an action returns the old text (A2 saw a touch counter stuck at its old value). Then every window from `getWindows()`, walked with `getChild(i, FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST)` on API 33+ (plain `getChild(i)` below), serialized as a flat array with parent indexes. `FLAG_PREFETCH_DESCENDANTS_HYBRID` on the window root was not better than the default: roughly one binder round trip per 2 to 4 nodes whatever the mode (a prefetch call is capped at 50 nodes), and depth-first on every `getChild` cut the round trips by about 45% on the Settings main screen (99 to 52) and gave the best time (A1 table). Task 2.10 added `FLAG_PREFETCH_UNINTERRUPTIBLE` to those flags: without it the next `getChild` cancels the prefetch in flight, and on win-pc Settings main (155 nodes) spent 78 to 103 ms in `getChild` against 41 to 46 ms with it (`getChild(i)` with no flag: 118 to 137 ms). The agent leaves out full-width system windows of at most a tenth of the screen height on the top or bottom edge (the status and navigation bars): Settings main went from 192 to 155 nodes and the tree from p50 104 ms to 84 ms over the tunnel (task 2.2, API 37). A read that comes back hollow (no window other than the edge bars, or at most one node per window) is repeated every 20 ms for up to 800 ms: right after a navigation `getWindows()` lists the status bar only, or the closing activity's window with no children (API 37, Back from a second activity, task 3.1). On API 26 a connection made while a screen animates forever sees no window until the first window change (neither waiting, nor `setServiceInfo` again, nor a tap or key changes that), so once per connection the agent opens and closes the notification shade, which is a window change and repairs it; 16 of 16 connections on the Busy tab were repaired, 4 of 16 had been broken. The keyguard state comes from `IWindowManager` directly, because `KeyguardManager` cannot be created from `app_process` on API 37 (`ApplicationSharedMemory not initialized`). Events: window-state and windows-changed only, pushed as `window_changed { window_id, package, t }`. Spike A1 saw one navigation produce 14 to 21 `TYPE_WINDOWS_CHANGED` and 5 `TYPE_WINDOW_STATE_CHANGED` events over about 2 s, so a 16 ms coalescing window does not turn them into one event; the settle detector treats every one as activity and `quiet` ends the storm. A pure scroll produced none of them (it produced only `TYPE_VIEW_SCROLLED` and `TYPE_WINDOW_CONTENT_CHANGED`, which we do not subscribe to), which is why scrolling is seen only through frames. Never calls `waitForIdle`. A second UiAutomation client is detected by the exception text `UiAutomationService ... already registered!` (an `IllegalStateException` from `registerUiTestAutomationService`), which is what `AutomationConflict` matches; `uiautomator dump` hit by the same conflict dies with exit 137 and prints nothing on stdout (the text is in logcat only).
- **Text.** Three modes:
  - `Paste` (default): keep the current `ClipData` object in agent memory (any type: text, URI, image), set ours, inject `KEYCODE_PASTE` with WAIT_FOR_FINISH, then restore the saved `ClipData` only after the field's text is seen to change in the tree or 500 ms pass, whichever comes first. WebView and some Compose fields read the clipboard asynchronously, so restoring right after injection can paste the old clip (spike A4, Chrome on API 37: restoring at 0 ms pasted the old clip into the page's input; restoring at 50, 150 and 500 ms pasted the new text). The agent thread that touches `ClipboardManager` needs a Looper on API 26 (`Looper.prepare()`), or the call throws "Can't create handler inside thread that has not called Looper.prepare()". On API 33+ the system draws a clipboard preview chip for about 1 to 4 seconds after the clip is set; marking the clip with `android.content.extra.IS_SENSITIVE` masks its text in the chip as dots but does not suppress it, and Android 13+ also shows a "<app> pasted from your clipboard" toast when the focused app reads it. Both are visible in frames and screenshots and are not errors. Full Unicode. Respects the app's own input handling.
  - `SetValue`: `ACTION_SET_TEXT` on the target node. Replaces the content exactly; some apps ignore it (no IME events).
  - `Keystrokes`: `KeyCharacterMap.load(VIRTUAL_KEYBOARD).getEvents(chars)` injected one by one. That call returns `null` for characters it cannot map (spike A4: emoji and also `é`; plain ASCII works); the agent checks every character first and fails the whole request with the first unmappable character, before typing anything. It takes 1.6 to 2.4 s for 100 characters (about 16 ms each with WAIT_FOR_FINISH), so it is a fallback, not a speed path.

  After any mode, the session reads the node's text back and reports `verified: true|false`. A password field exposes masked text, so for `SecureField` targets `verified` compares lengths only. A `false` from `Paste` (some OTP and PIN fields block paste) carries the hint to retry with `--mode keys`. `--submit` sends Enter (Android `KEYCODE_ENTER`, iOS Return) after the text.
- **Apps.** Launcher activities with labels (scrcpy `DeviceApp` pattern). Launch by package through the ActivityManager wrapper. Foreground app from the active window's package.
- **State.** Screen on, keyguard locked, display size and rotation, IME visible.

### 6.3 Android-specific states

| State | Behavior |
|---|---|
| Screen off | Before any `act` (except `press lock`) the daemon reads the screen state (agent op `power`), injects `KEYCODE_WAKEUP`, and reports `woke: true`. After a wake or unlock it waits up to 3 s for the keyguard flag to settle and for an application window with input focus and a tree with content, because input sent earlier is rejected ("the system rejected an input event", API 26 and 37) and API 26 lists the app's window with an empty root until the next window change (the agent treats that as a hollow tree and kicks the shade once per wake) |
| Keyguard with PIN/pattern | `NeedsUser(UnlockDevice)`. Never enter credentials |
| Keyguard swipe-only | Swipe up from 90% of the screen height (a drag that starts on API 26's notification card does not unlock), wait for `locked` to clear, continue; the reply says `unlocked: true` |
| `FLAG_SECURE` window | Frames are black in that window; tree still works. Detected when the window's region in the frame is uniformly black (luma < 4) while the tree shows visible text in it; spike A3 confirmed it on API 26 and 37: the secure activity's whole window area was black (luma 16, the limited-range black level, mean 16.00 and max 16 over 115,584 samples) while the tree still listed its text and the status and navigation bars still rendered. Header shows `secure content hidden` |
| Another UiAutomation client | `AutomationConflict { holder }` with how to stop it |
| Xiaomi/HyperOS | Injection fails until "USB debugging (Security settings)" is on: `NeedsUser(AllowXiaomiInjection)`. Unverified until tested on Xiaomi hardware |
| Rotation | Agent restarts the encoder; session maps coordinates with the new size |
| `wm size` change | Same as rotation |
| `wm density` change | The agent's `display_changed` event carries the density; a different density becomes `DeviceEvent::DensityChanged`, the actor drops the connection and the next call starts a new agent with the new density (a few seconds). Not testable on the API 26 emulator: `wm density` restarts its system server and drops adb |
| IME shown | Header `keyboard shown`; elements under it get `under-keyboard`. The platform calls a view the keyboard covers invisible to the user, so `prune` keeps an on-screen node that lies inside the keyboard window's bounds (flag `v` without `V`). `tap` on an element marked `under-keyboard` fails `OffScreen` with `behind_keyboard: true` and the hint to `press hide-keyboard` or `scroll`, and sends no input |

Only display 0 is supported in v1.

## 7. iOS

Three tiers, chosen automatically per device and reported in `phone-use devices`:

| Tier | When | Screen | Input | Tree | Apps | Install on phone |
|---|---|---|---|---|---|---|
| **CoreDevice** | iOS 27+ physical device, Developer Mode on | RTP H.264 stream (displayservice) | HID: multitouch, buttons, keyboard | Runner if installed; else OCR | app_service | Nothing (runner optional) |
| **Runner** | iOS 17 to 26 physical device | Runner screenshots, DVT screenshot | Runner (`XCSynthesizedEventRecord`) | Runner | app_service or runner | PhoneUseRunner (signed) |
| **Simulator** | Booted simulator on a Mac | Runner screenshots | Runner | Runner | `simctl` | PhoneUseRunner (unsigned) |

Honest consequence: without the runner, a CoreDevice-tier phone has no accessibility tree, so snapshots are OCR text only, with no roles, no switches' states, and no icon-only buttons. The full product on iOS needs the runner, which needs code signing (a free Apple ID re-signs every 7 days). Spike I5 (Phase 6) tests whether the Accessibility Inspector audit service (go-ios `ios/accessibility/`) can give a signing-free tree fast enough; if it can, it replaces OCR as the CoreDevice tree source.

Not available on iOS in v1: `press recents` (no API; returns `Unsupported`).

### 7.1 Transport

`idevice`: usbmuxd discovery and lockdown pairing (the user taps Trust once), then the CoreDevice tunnel over the userspace TCP stack, then RSD service lookup. Works on macOS, Linux, Windows. DDI: on macOS, use Xcode's personalized DDI files; elsewhere, `phone-use ios setup` downloads them only after the user confirms the source. Mounting needs Apple's TSS server.

### 7.2 CoreDevice tier

- Start the display media stream first. HID input is silently discarded without it (idevice `hid.rs:1-11`). The stream stays up for the life of the lease.
- Codec: the offer lists only the H.264 bank (RTP payload type 123). Spike I3 (iPad, iOS 27.0) showed the device accepts it and sends H.264 (High, level 5.2, 2736 x 2064). With idevice's default two-bank offer the device sends HEVC on payload type 100, so idevice's labels are right and go-ios `ios/display/offer.go:35-38` is wrong. The video crate depacketizes RFC 6184 (single NAL, STAP-A, FU-A) and the device's parameter packet (a 4-byte header, then an `avc1` sample entry whose `avcC` holds SPS and PPS).
- Touch: points → normalized 0..65535 per axis. Multitouch up to 5 contacts.
- Buttons: Home, Lock, Volume via `send_button` (button usage codes confirmed in spike I1). `back` taps the navigation bar's back button when the tree shows one (`via: "nav-button"`), else performs a left-edge swipe (`via: "edge-swipe"`).
- Text: pasteboard `set_text`, then HID keyboard Cmd+V. Observed on the iPad (iOS 27.0, spike I1): Cmd+V raises the system prompt "Testbed would like to paste from dtpasteboardd" every time, and the text arrives (accents, emoji, CJK intact) only after someone taps "Allow Paste"; so Paste mode reports `ApproveSystemPrompt` and Keystrokes (US layout, no prompt) is the prompt-free path. Creating the HID keyboard makes iOS treat a hardware keyboard as attached: the software keyboard does not show (the runner's snapshot says "keyboard hidden" with a field focused). A modifier sent in the same report as its key is ignored (paste arrived as "V", Shift+A as "a"), so keys go down one report at a time, modifiers first, and up in reverse, 25 ms apart (10 ms dropped the character after a Shift).
- Frames: the stream is the panel's landscape frame with a portrait UI rotated a quarter turn counterclockwise, so the decoder output is rotated clockwise (2736 x 2064 becomes 2064 x 2736, 1032 x 1368 pt at 2x; the real screen is 1032 x 1376 pt, so a tap at a point of the frame lands 0.6% further down on the glass: the frame's normalized coordinates are the screen's). Orientation other than portrait is unobserved. Open problem (2026-11-06): after the first seconds the stream stops showing changes of the app area while the status bar keeps updating; ffmpeg and openh264 decode the captured bitstream to the same stale picture, a keyframe request (PLI and FIR) is ignored, and per-frame acknowledgments and receiver-control reports (as idevice's `rtcp.rs` describes Apple's Device Hub sending) did not change it. Until that is understood an iOS 27 device is driven through the Runner tier unless `PHONE_USE_IOS_TIER=coredevice`.
- Keyframe on demand with RTCP (idevice `display_stream/rtcp.rs`) when the session settles, for a sharp screenshot.

### 7.3 PhoneUseRunner

An XCTest UI test bundle (`ios-runner/`), Objective-C for direct use of private headers copied from WebDriverAgent. One test method that never returns: it opens a TCP listener on device port 27753 and serves our framing.

Requests: `snapshot { max_depth }` of the foreground application (first of `XCAXClient_iOS activeApplications`, `requestSnapshotForElement` with bounded `maxDepth` and eight raw AX attributes), `gesture` (synthesized event record), `screenshot { quality, format }`, `type` (a synthesized text-input event with `shouldRedact`, because `XCUIElement typeText:` writes the text into xcodebuild's output and the `.xcresult` bundle, which the secrets rule forbids), `button` (home and volume via `XCUIDevice pressButton:`, volume unavailable on the Simulator; lock via `pressLockButton`, which WDA's `fb_lockScreen` uses on simulators, while on iOS 27 devices WDA sends an IOHID power press instead, so Phase 6 must do the same), `activate { bundle_id }`, `open_url`. Wire details in `docs/protocol.md` section 3.

The runner serves one request at a time. Every request runs on the main thread as a main-run-loop block (`CFRunLoopPerformBlock`), not a main-queue block, because event synthesis completes through main-queue callbacks that a running main-queue block would deadlock. XCTest reports UI failures as test issues; the runner's `recordIssue:` turns each into an error reply instead of ending the test. Snapshot and screenshot both run on the main thread, so the Runner and Simulator tiers poll for settle more slowly than the CoreDevice tier (section 12). Measured on the iPhone 17 simulator (iOS 26.5): snapshot 13 to 25 ms, screenshot about 75 ms JPEG and 63 ms PNG (`docs/spikes/ios.md`). Every synthesized event (tap, swipe, typing, key) costs about 230 ms more than its own duration: the testbed logs the tap 30 to 50 ms after the request, but XCTest's completion arrives at about 240 ms, for an empty spot as for a button; the session's `implicitEventConfirmationIntervalForCurrentContext` (5 s by default) made no difference at 0 or 0.05. The runner waits for the completion, so the Simulator tier misses the 80 ms tap budget (decision D15).

Simulator tier (task 5.5, `crates/ios/src/device.rs`). `tree` is the runner's snapshot with the foreground app as the application window and the keyboard, when shown, as an input-method window. Back: the button whose identifier is `BackButton` (UIKit and SwiftUI both name the navigation bar's back button so), else a drag from the left edge to 70% of the width over 300 ms, which also goes back in a web view with back-forward gestures; a hidden navigation bar disables the interactive pop, so the edge swipe then does nothing. `paste` and secret text type through the runner's redacted text input at 5000 characters per second: a synthesized Cmd+V pastes text the app copied itself but not text another process put on the pasteboard (runner or `simctl pbcopy`; no prompt appears, nothing is pasted), so the pasteboard is never used for typing and a secret never lands there. `keys` mode types at 60 per second. `clipboard` uses `simctl pbcopy`/`pbpaste` with a UTF-8 locale (`pbcopy` reads Mac Roman otherwise). `apps` is `simctl listapps` converted by `plutil`; `launch` is `simctl launch`; `open` is the runner's `open_url`, which unlike `simctl openurl` raises no confirmation for custom schemes. `press notifications` drags down from the top edge at 20% of the width. Volume and recents are `Unsupported`.

As built in task 5.6: the tree drops XCUI's scroll indicators ("Other" elements labeled "Vertical scroll bar, 2 pages" with a percentage value, matched on the English label the Simulator uses), which added two to four lines per list and, as the only labeled child of a multi-line field or a Files rename field, became that field's label. A toggle row (Settings, SwiftUI `Toggle`) is a labeled switch the width of the row around an unlabeled switch, the control itself; a tap at the row's center does not toggle, so the row takes the inner switch's bounds and the inner switch is dropped. For about 1.3 s after `simctl launch` returns, the runner's snapshot fails with `kAXErrorServerNotFound` or still reports the app that was in front (every launch of Settings), so `launch` returns once the snapshot reports the launched app, and a tree read retries `kAXErrorServerNotFound` for up to 5 s. `SimSource` reuses a simulator listing for 2 s: the daemon lists devices on every request and `simctl list` takes about 70 ms.

Host connects through idevice's device-port connect (as `services/wda.rs` does), so no host port forwarding. usbmuxd reaches a device's TCP ports from a non-loopback address, so on a device the runner listens on `0.0.0.0` (the Simulator build stays on `127.0.0.1`); anything on the device's networks can reach port 27753 while the runner runs (observed 2026-11-06: a loopback-only listener was not reachable, the all-interfaces one was). Launch: on devices, `xcodebuild test-without-building -destination id=<udid>` signed with `PHONE_USE_TEAM` (idevice `xctest` is the way for a machine without Xcode and is not built yet); on simulators, `xcodebuild test-without-building -xctestrun ... -destination id=<udid>` as a child of the daemon, with the port passed as `TEST_RUNNER_PHONE_USE_PORT` (xcodebuild only forwards `TEST_RUNNER_`-prefixed variables to the test process, without the prefix). Readiness: the daemon polls the port with `hello` every 100 ms up to 60 s; it does not rely on xcodebuild's log output.

Orientation (task 7.3). The simulator rotates only through the GUI or an XCUITest (`XCUIDevice.orientation`); the app's frame follows it. The runner's synthesized touches are expressed in the portrait frame whatever the orientation (observed on an iPad Pro 13-inch simulator in landscape: a tap at (519, 438) landed at (438, 513); the record's `interfaceOrientation` made no difference), so `Gesture.m` rotates each point back by `XCUIDevice.orientation`. The screen size in the header and in swipes comes from the application element's frame on every snapshot (`Device::current_screen`), not from the size the runner reported at connect.

Simulator lifecycle (task 5.4, `crates/ios/src/runner.rs`). The runner's sources are compiled into the `phone-use` binary (`include_str!`), so the daemon builds the runner wherever it runs: it writes them to `~/.phone-use/runner/<hash>/src`, runs XcodeGen and `xcodebuild build-for-testing` for `generic/platform=iOS Simulator` once per source hash, and deletes builds of other hashes (2.4 s on the Mac mini with Xcode's module caches warm; the build log is `build.log` next to it). Ports: the first port from 27753 to 27784 that nothing listens on and no other simulator of this daemon claimed. Reuse: before launching, the daemon says `hello` on each runner port; a runner whose `hello` reports this simulator's `udid` and this binary's source hash as `build` is reused. Process groups: `xcodebuild` runs in the daemon's process group (the CLI starts the daemon as a group leader), and killing `xcodebuild`, even with `SIGKILL`, ends the runner inside the simulator (observed on the mini). So dropping the device or killing the daemon's process group ends the runner; a daemon killed alone leaves `xcodebuild` and the runner running, and the next daemon reuses that runner. The runner's output goes to `~/.phone-use/runner/<udid>.log`; typed text never reaches it (section 7.3, `type`).

Private headers drift between Xcode releases. The runner is built and tested against the Xcode on the build machine (27.0 at planning time); a build failure after an Xcode update is expected maintenance, not a design flaw.

Signing: built with Xcode automatic signing for the user's team. A free Apple ID works and expires every 7 days; `phone-use devices` shows the expiry. Signing without a Mac is out of scope for v1 (see "Known gaps").

### 7.4 iOS states

| State | Behavior |
|---|---|
| Locked with passcode | `NeedsUser(UnlockDevice)` |
| Developer Mode off | `NeedsUser(EnableDeveloperMode)`; idevice `amfi` reveals the toggle in Settings |
| Not paired | `NeedsUser(TrustComputer)` |
| Runner expired | `NeedsUser(ResignRunner)` with the exact command |
| Tunnel dropped | Actor reconnects once, then `DeviceGone` |

## 8. Video (`phone-use-video`)

- Input: H.264 Annex-B access units from the Android agent, or RTP packets from iOS depacketized into access units (section 7.2).
- Decode: `openh264` for H.264 on every OS. The CoreDevice tier asks the device for H.264 only (spike I3), so no HEVC decoder is needed.
- Output, `FrameStore`: the latest decoded frame (`Arc`, planar YUV 4:2:0), a luma thumbnail with one sample per 4×4 points (the settle measure of section 4.6), a sequence number, and the time of the last frame, with a `watch` channel carrying `FrameRef { seq, changed_area_pt2, changed_box }`. Every frame is decoded (inter-frame coding forbids skipping).
- Screenshot = encode the latest frame (PNG or JPEG) on the host. A static screen can send no frames for seconds (observed in task 2.7 on the API 37 emulator through the tunnel: bursts at about 10 frames per second, then gaps of up to 5 s with nothing changing on screen), so a quiet stream is not a broken one and the last frame is a correct screenshot. The stream is broken only when its socket ends (`VideoStream::ended`, which `AndroidDevice::screenshot` reports as `DeviceGone`, and which makes the actor reconnect on the next call). The first frame after a connect is awaited for up to 2 s. Screenshots requested while an action is settling wait for settle, so encoder pipeline latency cannot return the pre-action frame.

## 9. Pixels and OCR

For canvases, games, WebViews without accessibility, and CoreDevice devices without the runner.

- `Source::Ocr` elements carry `role: Text`, a label, bounds, and confidence. They get refs like any element.
- Engine: Apple Vision on macOS (`objc2-vision`), `VNRecognizeTextRequest` revision 2 at the Accurate level (revisions 1 and 3 take 18 to 36 s and often fail on macOS 27.0.1; revision 2 takes about 45 ms on a 411x923 frame). The first call after a long idle can take about 20 s while the Neural Engine loads. Accuracy depends on resolution: at 1 pixel per point small gray text (13 pt button labels, tab bar captions) is misread, at twice that most is read, so OCR should be fed `--native` frames. `core::ocr::merge` drops a box whose text matches a tree element over it (IoU > 0.5) and appends the rest as `Source::Ocr` elements with `TAP`, printed with `(ocr)`. On Linux and Windows, `snapshot --pixels` returns `Unsupported` in v1; add `ocrs` when a user needs it there.
- `snapshot --pixels` forces OCR. When the tree is empty or a `WebView` has no children, the session merges OCR boxes into the snapshot automatically and marks them `(ocr)`. As built (task 7.2): the daemon screenshots and recognizes on the Mac after reading the tree; an OCR failure in the automatic case is ignored (the tree alone is shown), with `--pixels` it is the reply. A box that matches an OCR line of the last snapshot keeps that line's ref, and while the last snapshot used OCR, ref and text targets are resolved against a tree with OCR merged in, so `tap "Send"` taps an `(ocr)` line. Tested on the testbed tab `webopaque` (a WebView whose `getAccessibilityNodeProvider` returns null; `setImportantForAccessibility` and a view delegate do not hide Chromium's tree).

## 10. Agent surfaces

### 10.1 CLI verbs

```
phone-use devices                          list devices, tier, capabilities, leases
phone-use snapshot [--diff] [--interactive] [--within @eN] [--pixels] [--max-lines N]
phone-use screenshot [-o file] [--native] [--quality N]
phone-use find <text> [--role R]           refs matching text
phone-use tap <@ref|x,y|"text"> [--role R] [--long [ms]] [--count N]
phone-use type <text> [--into <target>] [--mode paste|set|keys] [--submit]
phone-use type --secret [--into <target>]  text from stdin or PHONE_USE_SECRET, never argv
phone-use clear <target>
phone-use swipe <up|down|left|right> [--from <target>] [--distance 0..1] [--fast]
phone-use scroll <up|down|left|right> [--in <target>] [--until "text"|@ref] [--max N]
phone-use gesture <json>                   multi-finger paths with timing
phone-use press <home|back|recents|notifications|lock|volume-up|volume-down|enter|hide-keyboard|...>
phone-use launch <app name|package|bundle id>
phone-use open <url>                       deep link or web URL in its handler app
phone-use apps
phone-use wait <"text"|@ref|gone:"text"|settled> [--timeout ms]
phone-use clipboard [get|set <text>]
phone-use release
phone-use daemon [--detach] | mcp | doctor | ios setup | version
```

Global flags: `--device` (default from `PHONE_USE_DEVICE`), `--session`, `--json` (machine output), `--no-settle`, `--timeout`.

Scroll mechanics: `scroll` drags 60% of the largest scrollable container (or `--in`) in 10 steps over 250 ms, then holds 150 ms with a stationary sample every 25 ms before the lift (Android 8 flings a list that lifts after a gap with no samples), so it never flings. `scroll --until "Label N"` flings when the visible labels share the prefix, the target is over three screens away and the device has a frame stream (`FRAMES`; a fling moves nothing on the polled iOS runner, observed in the agent-experience audit, so there it drags one screen at a time), and turns back on overshoot; other targets scroll one drag at a time. Without `--max` the limit is 30 drags, or enough one-screen drags to cross a numbered target's distance (at most 300). The search ends with `NotReached` (code -32016) when two drags in a row change nothing (`the list ended at "Developer"` after it had moved, `the list did not move` when it never did) or the limit is hit (`scrolled 30 times and "Row 640" is still not on screen`); `NotFound` is only for a target that cannot be resolved.

`clear <target>` taps the field to focus it, then sets its value to nothing with `ACTION_SET_TEXT` and reports `verified` when the field then shows no text (`via: set`). `clipboard set <text>` takes the text on the command line (it is not a secret; use `type --secret` for secrets), and `clipboard` prints the device clipboard; the daemon does not trace either.

`wait "text"`, `wait @ref` and `wait gone:"text"` read the tree once at the start and then once after each window change or frame with more than 128 pt² of change (100 ms after the burst's first signal), never on a timer; the reply's summary says how many tree reads it took. They resolve targets like `tap` does (substring match included), return a full snapshot, and fail `Timeout` naming the condition (default timeout 5 s). `snapshot --within @eN` prints only that element's subtree under the usual header, and a ref the screen no longer has is `StaleRef`.

Defaults: `scroll --max 30`; `swipe --distance 0.5`; `tap --count 1` (2 = double tap, taps 80 ms apart); `swipe --from` takes a target and starts the gesture at its center.

How the less obvious verbs work per platform:

| Verb | Android | iOS CoreDevice | iOS Runner / Simulator |
|---|---|---|---|
| `press notifications` | StatusBarManager `expandNotificationsPanel` (scrcpy wrapper) | swipe down from the top-left edge | same swipe via runner |
| `open <url>` | ActivityManager VIEW intent | runner `XCUIDevice.shared.system.open` if installed, else `Unsupported` | runner on device and Simulator (`simctl openurl` asks "Open in …?" for custom schemes) |
| `press back` | `KEYCODE_BACK` | nav-bar back button, else left-edge swipe; `via` says which | same; with the keyboard shown, `back` hides it first, as Android does |
| `press hide-keyboard` | no-op when the keyboard is hidden (`back` would leave the screen), else `KEYCODE_BACK` | n/a | a "Hide keyboard" key if the keyboard has one, else an upward drag from just above the keyboard (forms that dismiss on drag: Contacts, Settings; a drag down dismissed Contacts' New Contact sheet and lost the draft in the first live try); if the keyboard is still there the call fails `Unsupported` and says to `press enter` or tap Done (the testbed's Plain field keeps its keyboard on a drag) |

Text targets resolve by: exact label, then case-insensitive exact, then substring. More than one match fails with `Ambiguous` and lists refs. No match fails with `NotFound`, whose `similar` lists elements whose text contains the query or is within 0.3 of it by normalized optimal-string-alignment distance (insertions, deletions, substitutions and swaps of neighbors, case ignored, so `Rwo 5` suggests `Row 5`; plain Levenshtein would put that swap at 0.4). A list line says its size (`list 1000 rows`); a text row of a list is a `cell`, with its position when the app attached collection item info (RecyclerView does only for rows bound while an accessibility service was connected).

### 10.2 MCP

`phone-use mcp` with `rmcp`, stdio. Ten tools: `devices`, `snapshot`, `screenshot` (returns an image content block), `tap`, `type`, `swipe`, `scroll`, `press`, `launch` (takes `app` or `url`; a URL maps to the `open` verb), `wait`. Each tool's input schema mirrors the CLI verb; outputs are the same text the CLI prints. Few, well-described tools beat many. Every tool also takes an optional `device` (default `PHONE_USE_DEVICE`). `type` takes `secret: true` instead of the CLI's stdin route, since the text already crossed the MCP channel to reach the server; it is masked in the reply and traced by length only. A failed call returns `isError: true` with the message and `hint: ...` as one text block. `screenshot` returns an `image/png` block and a text block `WxH at scale S`. The server is one session `mcp-<pid>` on the daemon (started on demand like for the CLI) and releases it when stdio closes (or, if the client dies first, when the daemon sees the connection close). Calls run one at a time on that connection, in the order they arrived: the server runs on a single-thread runtime, because rmcp starts one task per request and on several threads two calls sent together took the lock in either order (5 runs of 4 swipes in one turn: 3 of 3 failed on the multi-thread runtime). The `instructions` and the descriptions of `tap`, `type`, `swipe`, `scroll`, `press` and `launch` say that calls share one screen and go one at a time (agents sent tap and type together in 7 of 9 audit runs). `find`, `clear`, `clipboard`, `gesture` and `apps` stay CLI-only until an agent run shows a need.

### 10.3 Skill

`SKILL.md` at the repo root, printed by `phone-use skill`: the loop (snapshot, act on refs, read the diff), when to use pixels, how to handle `NeedsUser`, and the rule that the agent never enters passwords or payment details without the user's explicit instruction.

## 11. Development environment

- MacBook (this machine): Rust builds with `blaze`. Has Xcode 27, Android build-tools 37 and platform `android-37.0` at `/opt/homebrew/share/android-commandlinetools`, `d8`, `adb`, JDK 17.
- Android emulator: Windows PC `win-pc`, AVD `Pixel_API_37`, SDK `D:\Android\Sdk`. Reach it from the Mac with `ssh -N -L 5038:127.0.0.1:5037 win-pc` and `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`. All agent streams flow through the adb server, so the daemon on the Mac drives the emulator on the PC.
- Launch the API 37 emulator with `emulator.exe -avd Pixel_API_37 -port 5554 -no-snapshot -no-audio -no-boot-anim -no-window -gpu swangle_indirect` (the AVD's `config.ini` also says `hw.gpu.mode=swangle_indirect`). Without `-gpu swangle_indirect` the emulator picks lavapipe (software Vulkan; the old `swiftshader_indirect` mode is rejected at launch and falls back to it), and with lavapipe the emulator crashes within 2 to 10 s of the first WebView, with no UiAutomation client involved; when its crash reporter also crashes, the process is left suspended and `adb shell` never answers. With swangle 11 trials had no hang or death, but two emulators launched with it still stopped later (`docs/bench/m2.md`), so it is not shown to be sufficient.
- An API 26 emulator for the minimum-version checks: create `Pixel_API_26` on win-pc.
- iOS simulators and any GUI: the Mac mini (cua Space `mini` for shell and files, `mini-desktop` MCP for its screen). Never drive GUIs on the MacBook's own screen. On 2026-11-05 the mini had Xcode 27.0, the iOS 26.5 simulator runtime only (no iOS 27), Python 3.14, XcodeGen 2.46 (installed with Homebrew), and no Rust toolchain. Sources reach it by `cua` upload into `~/phone-use-work/`.
- Physical phones: the MacBook currently sees a paired iPhone 14 Plus on iOS 26.5 and two other iOS devices. These belong to people. Ask the user before driving any physical device.

## 12. Performance budgets

Falsifiable targets, measured by `bench/` (Phase 2 and 6). A measured miss means profile first, then change the design only with evidence.

| Operation | Android (emulator or USB, daemon on the same machine) | iOS CoreDevice | iOS Runner / Simulator |
|---|---|---|---|
| tap, request to delivered | p50 ≤ 15 ms | p50 ≤ 30 ms | p50 ≤ 80 ms |
| snapshot (Settings main), cold tree, `--no-settle` | p50 ≤ 80 ms | n/a (runner) | p50 ≤ 300 ms |
| screenshot, settled screen | p50 ≤ 30 ms | p50 ≤ 40 ms | p50 ≤ 400 ms |
| type 100 chars incl. emoji and CJK | ≤ 150 ms | ≤ 200 ms | ≤ 400 ms |
| tap to settled diff (simple toggle) | ≤ 600 ms | ≤ 700 ms | ≤ 1200 ms |

The Android numbers are measured with the daemon on the machine that runs the emulator (win-pc), because a network hop through an SSH tunnel adds its own latency. Development over the tunnel is fine; budgets are not judged there.

Baseline: phone-harness 0.3.3 on the same emulator, same machine, same operations (Phase 2, task 2.10).

Measured in task 2.10 (`docs/bench/m1.md`, win-pc, API 37 emulator, p50 of two 50-run series): snapshot Settings main `--no-settle` 60 and 68 ms (met), screenshot 15 and 14 ms (met), tap to settled diff 544 and 420 ms (met), tap 30 and 37 ms (missed), type 100 chars 253 and 288 ms (missed). The tap budget includes about 7 ms of adb transport and, with `WAIT_FOR_FINISH` on the last event, about 14 ms of the app handling it on a software-rendered emulator; injecting `ASYNC` reached 15.1 ms and was not adopted because "delivered" would mean "queued". The type budget is spent on clipboard and accessibility binder calls and on the wait that confirms the paste before the clipboard is restored. Both are re-judged on a physical phone in Phase 3.

Measured in task 5.6 (`docs/bench/m4.md`, Mac mini M4, iPhone 17 simulator, iOS 26.5, p50 of two 50-run series, release build): snapshot Settings main `--no-settle` 28 ms (met), screenshot 83 and 84 ms (met), type 100 chars 355 and 356 ms (met), tap to settled diff 712 and 717 ms (met), tap 359 and 357 ms (missed: about 240 ms is XCTest's event completion, D15, and up to 80 ms is waiting for an in-flight settle poll).

## 13. Security and privacy

- No telemetry. Nothing leaves the machine except traffic to the device and, for iOS DDI mounting, Apple's TSS server.
- The daemon socket is user-only. No network listener.
- Never executes agent-supplied code.
- Secret text arrives on stdin or in `PHONE_USE_SECRET`, never on the command line (argv is visible in `ps` and shell history). It is never logged, traced, or echoed. Typing into a `SecureField` is treated as secret even without the flag. `SecureField` values never render.
- The Android agent's socket requires the per-launch token (section 6.2).
- Clipboard is restored after paste on Android. On the iOS Simulator, typing never touches the pasteboard (section 7.3). The CoreDevice tier pastes through it (section 7.2) and the pasteboard holds the text afterwards; replies say so.

## 14. Known gaps in v1 (stated, not hidden)

- iOS runner signing requires a Mac with Xcode.
- iOS 17 to 26 devices have no frame stream, so settle and screenshots are slower.
- DRM video renders black on both platforms.
- Biometric prompts (Face ID, fingerprint) need the user.
- No audio capture or camera injection.
- No file push or pull (for example a photo to upload). Add only when an agent task needs it.
- No GPS location simulation.
