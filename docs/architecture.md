# phonedriver architecture

phonedriver lets an AI agent see and operate a real or virtual phone or tablet: Android and iOS, USB or network, device or emulator. This document is the design. `docs/decisions.md` records why each choice beat its alternatives. `docs/protocol.md` defines every wire format. Terms are defined in `docs/GLOSSARY.md`.

## 1. What the agent experiences

The design starts here. Everything below exists to make this loop fast, exact, and hard to misuse.

```console
$ phonedriver snapshot
device android:emulator-5554 · Pixel 9 · Android 17 · 412x915pt · portrait
app com.android.settings (Settings) · keyboard hidden · settled in 212ms
@e1 searchbox "Search settings"
@e2 list scrollable↓
  @e3 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  @e4 button "Connected devices" "Bluetooth, pairing"
  @e5 button "Apps" "Recent apps, default apps"
  @e6 button "Display & touch" "Dark theme, font size, brightness"

$ phonedriver tap @e6
ok tapped @e6 "Display & touch" at 206,488 · settled in 340ms · new screen
app com.android.settings (Settings) · keyboard hidden
@e9 heading "Display & touch"
@e10 switch "Dark theme" off
@e11 button "Brightness level" "64%"

$ phonedriver tap @e10
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
 agent ──CLI──┐                                ┌── Android device ── phonedriver-agent (app_process, shell uid)
 agent ──MCP──┼─ JSON-RPC over local socket ─ phonedriver daemon ──┼── iOS 27+ device ── CoreDevice services (no app)
 SDK  ────────┘                                │                 └── iOS ≤26 device / Simulator ── PhonedriverRunner (XCTest)
                                               └── per device: DeviceActor { connection, FrameStore, Session state }
```

One Rust binary, `phonedriver`, plays three roles selected by subcommand:

- `phonedriver <verb>`: the CLI. A thin client. It connects to the daemon, sends one request, prints the reply.
- `phonedriver daemon`: the long-lived process that owns every device connection. Started on demand by the CLI.
- `phonedriver mcp`: an MCP server on stdio. Also a thin client of the daemon.

The daemon exists because the expensive things are persistent: the on-device agent, the iOS tunnel, the video decoder, the tree cache, ref identity. A per-call process (the competitor's model) pays all of that every time.

## 3. Repository layout

```
phonedriver/
  Cargo.toml                    workspace
  crates/
    core/      phonedriver-core   pure domain: elements, refs, snapshot render, diff, settle, geometry, errors
    video/     phonedriver-video  H.264/HEVC decode to FrameStore (RGBA + luma thumbnail)
    android/   phonedriver-android adb server client, agent deploy, agent protocol client, AndroidDevice
    ios/       phonedriver-ios    idevice transport, CoreDevice tier, runner client, simulator, IosDevice
    cli/       phonedriver        the binary: cli, daemon, mcp, rpc
  android-agent/                Java sources for the on-device agent, built to phonedriver-agent.jar
  ios-runner/                   XcodeGen project for PhonedriverRunner (XCTest UI test bundle)
  testbeds/android/             Gradle app with ground-truth UI and event logging
  testbeds/ios/                 XcodeGen SwiftUI app with ground-truth UI and event logging
  bench/                        workspace member `phonedriver-bench` (publish = false): latency, scenarios, baselines
  NOTICE                        attribution for code copied from scrcpy and WebDriverAgent
  docs/
```

Why these crate boundaries (each must pay rent):
- `core` is pure and has no IO. It holds the logic that decides correctness (ref matching, diff, settle, rendering) so it can be property-tested without devices.
- `video` has two consumers (Android and iOS streams).
- `android` and `ios` are the two implementations of the `Device` port. Their dependency trees are disjoint (`idevice` is large).
- `cli` is the only binary. Daemon, CLI and MCP share request types, so they live together.

No other shipped crates (`bench` is a dev-only binary). A new crate needs a second consumer or a disjoint dependency tree. OCR engines live in `cli` because they are IO; the pure merge logic lives in `core`.

## 4. Domain model (`phonedriver-core`)

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
4. Elements that left the screen (a popup or dialog that hid them, a menu that closed back to the same dialog) stay claimable: the last 400 are kept, and a node whose `(role, id, label, ancestor)` key, or without the ancestor, equals one of them and sits within 8 points of where it was takes its ref again. Found by dogfooding the Clock alarm dialog: every ref renumbered after the AM/PM popup closed and after the time dialog closed.
5. Otherwise a new ref.

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
2b. **One control, one line.** (An unlabeled tappable `Cell`/`Other` wrapper that holds exactly one enabled labeled `Button` and no text of its own is dissolved the same way: iOS Settings rows are an unlabeled cell around one button.) An iOS row is a `cell` with a button of the same label inside it, and both take the tap (observed in the agent-experience audit: `tap "Display & Text Size"` was ambiguous between `@e451 cell` and `@e452 button`, and every row cost two or three lines). A labeled, tappable `Cell` or unclassified (`Other`) node that has a tappable `Button` descendant with the identical label is dropped and the button stays (innermost wins, and a text target resolves to it); any other control (a switch, a button) keeps its own line and absorbs the same-label button inside it. Two separate buttons that share a label are left alone. A tappable `text` child whose text the labeled parent already contains is absorbed too (the Clock alarm row `button "8:30 AM Alarm"` over a tappable `text "8:30 AM"`: the tap lands on the row, and the child is no separate target). A checkable control whose label is only `on` or `off` loses the label (`switch "OFF" off` is `switch off`; the state word carries it). Found by dogfooding the Clock alarm list (golden `captured_clock_alarms`). Labels and hints space with plain spaces: Android's Clock writes `8:30\u200aAM` (hair space) inside a row labeled `8:30 AM`, iOS writes times with U+202F, and an agent searches with a space. Field values keep their own characters, because `type` is verified against them. Wrappers that hold one element and say nothing go too: an unlabeled, inert, valueless group with one child, and an unlabeled web view whose only child is a web view or that sits inside one. Safari wraps every page in a group and three web views; the one that scrolls keeps its line (`webview "8 pages" scrollable↓`). On the Ada Lovelace article this cut the `open` reply from 2,059 to 1,754 bytes (39 to 35 lines, 402 to 228 spaces of indent). Web forms (`prune::web_forms`, after the folds): Safari's radio button is a group whose value is `0` or `1` beside a text with its label, so it becomes `radio … on|off`. A text in a web page that repeats the label or hint of a sibling that is not text, link or heading (the page's `<label>`, Chrome's `"Customer name: "` beside its field), or its parent's label (a fieldset legend), is not printed. Spaces and a trailing colon are ignored in the comparison. Safari's `… scroll bar, N pages` elements are not printed either. On httpbin's pizza form this took the Safari reply from 39 to 24 lines and Chrome's from 21 to 16, and `tap Medium` stopped failing as ambiguous between the radio and its label.
3. Collapse containers that have no label, no actions, and one child.

Inside a web view, a run of sibling text and links prints as one line with each link inline: `@e14 text "the [free @e15] encyclopedia that [anyone can edit @e17]"`. An element that is not pressable and whose children spell its label takes them into its own line: a heading, and on Hacker News the row `cell "104 points by [user1 @e12] [1 hour ago @e13] [hide @e14] [70 comments @e15]"`, which printed its label and then the same words again as a child line. The words are compared without separator punctuation (the label puts `|` between links whose separator texts were pruned). Such a parent takes a run of links alone (`cell "[24 minutes ago @e24] [hide @e25]"`), but only when the joined line fits `MAX_TEXT`, because a longer label is cut short and the links would be lost. Chrome names a link with no text after its address; a tappable web label without spaces that has a `?x=y` query keeps only the part before `?` (`vote?id=50427362&how=up&goto=news` prints as `button "vote"`). The first Hacker News screen went from 95 lines and 5643 bytes to 72 lines and 4054 bytes. Elements and refs are unchanged (D35).
4. Keep a node if it has actions, has text, is a `TextField`/`SecureField`, or is a scrollable container.

Reading order (task 2.7): siblings print top to bottom by vertical center in 8-point bands, then left to right, because the agent lists a window's views in z order (an app bar comes after the list it overlaps); windows keep the agent's order, topmost first (a dialog before the app under it), except the keyboard window, which prints last.

Text-target resolution follows the same rule: `tap "Network & internet"` matches the row whose merged label is that text, so the tap goes to the actionable ancestor, never to an inert text node. If text only matches a non-actionable node, the tap goes to its nearest actionable ancestor; if there is none, to the text's own center.

The keyboard window is one `keyboard` node without its keys (task 3.2): an agent types with `type`, and about sixty key nodes made the first typed character a "new screen" in the diff (more than 60% of refs changed). The same holds on iOS since the agent-experience audit (it measured 26 to 40 `button "q"` lines, about 190 tokens, per keyboard appearance): `ios::map::tidy` keeps only the `Keyboard` node of the window that holds the keyboard, as a child of the application, and drops that window's keys and its Emoji and Dictate buttons. A key is therefore not a tap target by name; `press enter`, `type --submit` and `press hide-keyboard` cover what keys were used for.

Covered and scrollable (agent-experience audit): a tap on an element whose center lies inside a floating `Toolbar` group that is neither its ancestor nor its child fails `OffScreen` with `covered_by: "Toolbar"` and a scroll hint (down for a bar on the lower half of the screen), instead of tapping the toolbar (the iOS 26 search bar covers the last Settings rows and `tap Developer` reported success twice); `scroll --until` keeps scrolling while its target is under such a bar. An XCUI scroll view has no scroll actions, but its indicator (`Vertical scroll bar, 2 pages`, value `48%`) says where it is: `ios::map::tidy` turns it into `scroll_up`/`scroll_down` (left and right for a horizontal one) on its owner, only when it has more than one page, and into the value `2 pages`, so an iOS list prints `@e25 list "2 pages" scrollable↓`. iOS does not report row counts; the page count is what it knows.

Compose and WebView (task 3.10): both build their accessibility tree on the first query, about a second after the view appears, and show as a childless node until then, so the agent's first read of each `WebView` or `AndroidComposeView` waits up to 2.5 s for children. A Compose toggle (`Switch`, `Checkbox`, `RadioButton`) is an `android.view.View` that is checkable and takes the tap, with its role in the `AccessibilityNodeInfo.roleDescription` extra of a child node (`rd` on the wire, protocol section 2); the parser gives the checkable parent that role. A Compose `testTag` is the node's `id` when the app sets `testTagsAsResourceId`. `isHeading` exists from API 28, so a Compose title is a `heading` only from there. A text target that matches an inert element and exactly one element with actions (a web label `Name` next to its input `Name`) resolves to the one with actions.

Noise rules (agent-experience audit, finding 9): the second quoted string is left out when it equals the label (iOS repeated a web text and a field's placeholder as both label and value); a disabled button named `chevron` (every iOS row has one) and an exact duplicate of a sibling (same parent, role, label and bounds: the keyboard's two Dictate buttons) are not kept; `focused` is not diffed for elements that are not text entry (typing in a web form printed eight `~ group "..." focused → (none)` lines).

Text limits: truncate text above 200 chars as `…(+N chars)`; `snapshot --within @eN` prints that element's own text whole, and a screen that cut a text ends with `hint: \`phonedriver snapshot --within @eN\` prints @eN whole` for the first cut element (MCP: `snapshot` with `within`). A value of 100 or more characters that only starts the label is a repeat too (iOS cuts values at 512 characters). A value that repeats the label except for spacing (WebKit gives a static text the label as its value, with the line breaks and a leading space the label drops) is not printed. `SecureField` values render as `•••` and are never logged.

Line grammar: `@eN role "label" ["value"] [state…]`, indented two spaces per kept depth. The second quoted string is the value, or the hint when there is no value (a row's merged subtitle, a field's placeholder). A checkable element's value is omitted when its `on`/`off` word already says it. Role words: `button link text heading textbox searchbox password switch checkbox radio slider image list cell tab menu menuitem dialog keyboard webview alert progress group` (`group` is `Other`). An unlabeled `Other` with tap or long-press renders as `button`. A web checkbox's node is neither checkable nor checked, so the daemon reads its state from the screenshot and renders `on`/`off` (decision D26). States render as words: `off`, `on`, `disabled`, `focused`, `selected`, `scrollable↓`, `scrollable↑↓`, `under-keyboard`.

Header lines: device, app, keyboard state, orientation, settle time, and `loading` when a visible `ProgressIndicator` exists or a `WebView` has no children yet (a childless `WebView` is kept as a line for that reason).

System alerts (decision D23, `core::alert`): a window the system puts over the screen is named in a header line after the app line, `alert com.google.android.permissioncontroller: "Allow Camera to take pictures and record video?"`, and its elements print with refs like any others, so `tap "While using the app"` works whatever app is behind. Android: a window of `com.android.permissioncontroller`, `com.google.android.permissioncontroller`, `com.android.packageinstaller`, `com.google.android.packageinstaller` or `android` (ANR and crash dialogs, the chooser) that covers less than 90% of the screen, or any window of those packages or of `com.android.systemui` that carries the framework dialog ids (`android:id/alertTitle`, `parentPanel`, `aerr_close`, `aerr_wait`) and some text. The title is the node with id `permission_message`, `alertTitle`, `aerr_title`, `message` or `title`, else the first heading, else the first inert text. The `app` header names the first application window that is not an alert (observed: a camera permission dialog was the only window and the header had named `com.google.android.permissioncontroller` as the app). iOS: the runner also asks SpringBoard for its alerts (section 7.3) and `ios::map` turns each into a window of kind `alert` in front of the app. Not covered: toasts (they are not accessibility windows, so the agent never sees them), a notification banner, and a dialog an app draws inside its own window (that stays a `dialog` role line).

Text that reads like an instruction to an AI (decision D24, `core::inject`): the element line ends with ` (ai-instruction)` when its label, shown value or hint matches the phrase rules; the text itself is printed unchanged.

Approval guard (decision D27, `daemon::guard`): a tap, or a `type --submit` into a field whose form holds one, on an element whose label says it spends money or cannot be undone (`Place order · $34.10`, `Buy now`, `Pay $12.00`, `Confirm payment`, `Delete account`) fails with `needs_approval` unless the request carries an `approval` (below). The error carries `label`, `app`, `category` (`payment` and `account` are always asked; `send`, `delete` and `install` only when the request names them in `ask`, which the CLI and MCP server fill from `PHONEDRIVER_ASK`, decision D31) and `details`, the visible lines that say what is committed. The daemon holds the categories allowed for the session, `guard_allowed`, set by `approval: "session"` and gone with the daemon.

Filters: `--interactive` (only nodes with actions), `--within @eN`, `--max-lines N` (summarizes the rest as `… N more`).

Golden tests (`insta`) render fixture trees captured from real devices in Phases 2 and 5.

### 4.5 Diff

`core::diff(prev, next) -> Vec<Change>` where `Change` is `Added(ref)`, `Removed(ref)`, `Changed { ref, field, from, to }`. Rendered as `+`, `-`, `~` lines; consecutive removals collapse to one line. When more than 60% of refs changed ((added + removed) / refs in either observation), the reply is a full snapshot instead of a diff, because the agent is on a new screen. Line shapes: `- @e1 @e2` (removed refs only), `+ <element line>`, `~ @eN role "label" <from> → <to>` per changed field (label, value, hint, state words). A secure field's value change renders `~ @eN secure field changed` and the `Change` value carries no text. When a new screen replaced the one before (a dialog, a menu, another page), the session keeps that earlier screen while the agent is still shown the new one. If the next screen matches the earlier one, the reply is `the screen before the last one is back, unchanged` (or `…, except:` and the changes): a dialog closed, or the agent went back to a page it had just seen. The earlier wording, `back to the screen from before`, read as a back navigation when a tap went forward to a page the agent had just left (Contacts: list, contact, back, the same contact again).

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
    NoDevice { requested: Option<String>, connected: Vec<DeviceSummary> }, // `requested` is a --device id that matches nothing
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
    UnknownKey { key: String, valid: Vec<String> },           // `press` with a key no device has
    NoApp { query: String, similar: Vec<String> },            // launch by a name no installed app has; similar = `Name (id)`
    NotReached { query: String, scrolls: u32, end: ScrollEnd, last: Option<String> }, // a scroll search stopped: Ended, Stuck or Limit
    AutomationConflict { holder: String },     // another UiAutomation client owns the bridge
    Transport { detail: String },              // the only catch-all, for IO failures we have not classified
}
```

`UserAction` is closed: `UnlockDevice`, `AcceptUsbDebugging`, `EnableUsbDebugging`, `AllowXiaomiInjection`, `TrustComputer`, `EnableDeveloperMode`, `MountDeveloperImage`, `ResignRunner`, `SetSigningTeam`, `ApproveSystemPrompt { text }` (a biometric or OS prompt only a person may answer). Each has one message that tells the user exactly what to do on which device.

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

- **Socket.** `$XDG_RUNTIME_DIR/phonedriver/daemon.sock`, else `~/.phonedriver/run/daemon.sock` (Unix), or the named pipe `\\.\pipe\phonedriver-<user>` (Windows), via the `interprocess` crate. A Unix path longer than 100 bytes (`sun_path` is 104 on macOS, 108 on Linux; a long `PHONEDRIVER_HOME` hit it: the CLI waited 3 s and printed `local socket name length exceeds capacity of sun_path`) becomes `$TMPDIR/phonedriver-<hash of user and path>/daemon.sock` (`/tmp` when `$TMPDIR` is long too), in a 0700 directory like any other, so the client and the daemon compute the same short name. Directory mode 0700; on Windows the pipe's security descriptor grants only the current user. No TCP listener. `PHONEDRIVER_SOCKET` overrides the path. Remote use goes through `ssh -L` socket forwarding (zero network code, README). The `screenshot` method always saves the PNG on the daemon's machine and replies with its path; a client that asks with `inline: true` also gets the bytes in `data_b64`. The MCP tool always asks inline, so it returns the image over a forwarded socket (observed failing before: the MCP server read the daemon's path on its own disk). The CLI's `screenshot -o file` asks inline and writes the file locally; without `-o` it prints the path on the daemon's machine.
- **Start.** The CLI connects; on `ENOENT`/`ECONNREFUSED` it spawns `phonedriver daemon --detach`, then waits for the daemon's readiness line on an inherited pipe (not a sleep), up to 3 s.
- **Version.** First message is `hello { version }`. On mismatch the CLI sends `shutdown` and starts its own version.
- **Progress.** A device connect that runs past one second says what it is doing (`daemon/progress.rs`): `starting the iOS runner on iPhone 17 (the first run builds it; logs are in <home>/runner)`, `starting the Android agent on emulator-5554 (the first run on a device pushes it)`. The daemon writes the line to its own stderr (`daemon.log`) and sends it as a `progress` notification to connections whose `hello` said `progress: true` and that are waiting on a request. The CLI says so only on a terminal, so stdout and piped stderr stay as they were, and `phonedriver mcp` never does.
- **Stop.** `phonedriver daemon stop` connects without starting one, sends `shutdown` (no `hello`, so it works across a version mismatch), waits for the socket to go and for the pid to exit, and prints `stopped the daemon (pid N)`, or `no daemon is running` (exit 0). Exiting drops every device, and dropping an iOS runner kills the `xcodebuild` the daemon started (`kill_on_drop`), which ends the runner; a runner the daemon only reused keeps running.
- **Idle exit.** No clients and no leases for 15 minutes.
- **DeviceActor.** One tokio task per device owns the connection, FrameStore, and the sessions on that device. Requests arrive on an mpsc channel. No locks around device state.
- **Sessions outlive connections.** Every CLI call is a new process and a new socket connection, so session state cannot belong to the connection. Each request names a session: `--session <name>`, else `PHONEDRIVER_SESSION`, else `default`. The session holds the ref table, the last rendered observation (the diff baseline), and the settle detector. An MCP server process uses one session named `mcp-<pid>` over one connection that it keeps for its whole life, and says `client: "mcp"` in `hello`. The daemon releases every session an `mcp` connection used on every device when that connection closes, so a client that exits or is killed before it can send `release` (observed with `claude -p`: the next agent was locked out for ten minutes) frees the device at once. CLI sessions are not released on close, since every CLI call is its own connection.
- **Leases belong to sessions.** A session acquires a device on its first device call. Another session gets `DeviceBusy` unless it passes `--share`. A lease ends on `release`, when an `mcp` connection closes, or after 10 minutes without calls. `DeviceBusy` and `devices` say how long the holder has been idle (`idle 3m, lease expires in 7m`); an agent cannot take a lease over, it waits or asks the user. When a CLI session holds it, the hint names the command the user runs to free it (`phonedriver release`, or with `PHONEDRIVER_SESSION=<name>`); a lease held by an `mcp-` session belongs to another agent, so the hint offers no command. Two agents on one machine should use different `--session` names; the default session is for one agent at a time.
- **Discovery.** Android: adb `host:track-devices-l` push stream. iOS: usbmuxd listen. Simulators: `xcrun simctl list devices -j booted` on demand.
- **Device ids.** `android:<serial>`, `ios:<udid>`, `sim:<udid>`, plus aliases in `~/.phonedriver/config.toml`. With exactly one device connected, it is the default. With several, the default is the one that can be driven when only one can: a physical iPhone or iPad on the Runner tier counts as blocked while `PHONEDRIVER_TEAM` is unset (D18). Otherwise `AmbiguousDevice` lists them. A `--device` id that matches nothing fails `NoDevice` naming the id and the connected ids.
- **Artifacts.** Screenshots go to `~/.phonedriver/tmp/<device>/<ulid>.png`. Unique names. Cleared on daemon start when older than 24 h.
- **Traces.** Per session, `~/.phonedriver/traces/<ulid>.jsonl` (directory mode 0700, files 0600): one line per request with `method`, `params`, the reply's summary line, `ms` and the settle time. Typed text is replaced by its length, `{"secret_len": n}` with `--secret` and `{"text_len": n}` otherwise (D16). Text targets are replaced by their length, and observation text (which can hold messages or balances) is added, only when the daemon runs with `--trace-full` or `PHONEDRIVER_TRACE_FULL=1`. Each `screenshot` is copied next to the trace as `<ulid>-<n>.png` only with `--trace-screens` or `PHONEDRIVER_TRACE_SCREENS=1`. A running daemon keeps the options it started with, so a harness that wants them starts its own daemon under its own `PHONEDRIVER_HOME`. Traces older than 7 days are deleted on daemon start. Local only; nothing is uploaded.

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

`android-agent/` builds `phonedriver-agent.jar` (dex inside a jar) with `javac` against `android.jar` and `d8`, the way `scrcpy/server/build_without_gradle.sh` does. Minimum API 26 (decision D8).

Launch: push with mode 0444 to `/data/local/tmp/phonedriver-agent-<sha8>.jar`, where `<sha8>` is the first 8 hex digits of the jar's SHA-256 computed on the host. The jar is already deployed when that path exists (`ls`); no hashing on the device. The push goes to `<path>.part` and is `mv`ed into place, so an interrupted push cannot leave a truncated jar at the final path. Then, over a `shell,v2,raw:` stream:
`CLASSPATH=<jar> app_process / dev.phonedriver.agent.Main <version> <socket-name>` and write a 32-byte random token to its stdin.

- The host holds that shell stream open for the agent's whole life and drains its stdout and stderr (logs go to the daemon's log). adbd kills the child when the stream closes, which is what guarantees a crashed daemon never leaves an agent running.
- The agent listens on abstract socket `phonedriver-<version>-<rand>`. Abstract sockets have no permissions: any app on the phone can connect. So the first request on each stream must carry the token, and the agent drops the connection otherwise. Without this, any installed app would get shell-uid input injection and a full screen reader.
- The host opens two streams: `control` and `video`. The agent also exits when both streams close or when nothing connects within 10 s.
- One daemon owns a device's agent (decision D34). The start shell reads `/data/local/tmp/phonedriver-owner` (`pid=<pid> host=<host>`, written by the daemon that started the agent) and, when an agent is running under another name, prints `owned <that line>` and exits 98 before it kills anything; the host reports it as `device_driven` (protocol -32021) after asking again for 2 s, because adbd ends an agent about a second after its owner's shell stream closes. An agent with no owner file (from a phonedriver that wrote none) and an owner file with no agent running (stale) are replaced as before, and a daemon replaces its own agent (a reconnect) without the check.
- Before launch, the host kills this daemon's own previous agent, and agents with no owner file, with `pkill -f 'dev.phonedriver.agent.[M]ain'` (the jar path is in the environment, not the command line, so matching on the jar name would never match). The bracket matters: `shell,v2,raw:` runs the command as `sh -c "<command>"`, whose own command line contains the pattern, so the unbracketed form kills the shell that runs it and its `echo $?` never prints (spike A5, API 26 and 37).
- Measured (spike A5): closing the shell stream ends the agent in about 1.0 s (1054 and 1137 ms with our polling), not instantly; a jar pushed with mode 0444 loads on API 26 and 37 (0666 also loads for the shell user on the API 37 emulator, so 0444 is hygiene, not a requirement); a connection with a missing or wrong token is closed with no reply; the agent exits by itself 10 s after start when nothing connects.

Components (one Java file each where possible):

- **Main.** Version check, Looper thread, socket accept, shutdown.
- **Control.** Length-prefixed JSON requests and replies plus pushed events (`docs/protocol.md`). Uses `org.json`, which ships with Android; no dependencies.
- **Video.** Mirror the display into a MediaCodec H.264 encoder input Surface, configured as scrcpy's `video/SurfaceEncoder.java:318-332` does: `KEY_REPEAT_PREVIOUS_FRAME_AFTER = 100 ms` (so the first frame of a static screen is delivered and quality recovers after motion; repeated frames have zero change and do not disturb settle), `KEY_PRIORITY = 0`, `KEY_LATENCY = 1`, I-frame interval 10 s, 30 fps cap, 8 Mbps. Copy scrcpy's size alignment and `MAX_SIZE_FALLBACK` / downsize-on-error logic (`SurfaceEncoder.java:36,79-95,201`): hardware encoders reject some sizes, and 4K tablets exceed most encoders. Spike A3: both emulators' software encoder (`c2.android.avc.encoder`) emits Constrained Baseline by default (profile_idc 66, constraint flags 0xC0; level 5.0 at 1080x2424 on API 37, 4.1 at 1080x1920 on API 26) and `openh264` decoded every frame with no errors, so `KEY_PROFILE` stays unset. Hardware encoders on physical phones are not measured yet (Phase 3, if a phone is provided). A static screen yields about 3 frames per second (the 100 ms repeat is honored at roughly 330 ms by this encoder), so a stall is `no frame for 1 s`, not for 200 ms. A sync-frame request is honored only when a new frame is rendered (10 of 10 while the screen changed, 2 to 3 of 10 on a static screen), so a sharp image of a settled screen comes from the repeat frames, not from `sync_frame`. Restart on display size or rotation change (`display/DisplayMonitor.java`; the mechanism differs below API 34; the spike polls `DisplayInfo` every 100 ms and restarts the encoder, which gave a new codec header and the first frame at the new size 0.5 to 0.9 s after rotation, `wm size` and `wm size reset` on both APIs; screen off keeps frames flowing and the first new frame came 0.5 s after wake). A sync frame can still be requested (`PARAMETER_KEY_REQUEST_SYNC_FRAME`) to resynchronize a decoder, not to sharpen an image.
- **Input.** `InputManager.injectInputEvent` through reflection, `INJECT_INPUT_EVENT_MODE_WAIT_FOR_FINISH` for the last event of a gesture so the reply means "delivered". Multi-pointer MotionEvents (`PointersState` pattern from scrcpy). Gestures arrive as one message with full timed paths and are played on the device with `SystemClock.uptimeMillis` scheduling, so network jitter cannot distort a fling.
- **Accessibility.** UiAutomation connected from the shell process (`UiAutomationShellWrapper` pattern) with `UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` (API 24). Without it, connecting silently turns off the user's own TalkBack, Switch Access, and password-manager autofill services while we run. Service flags: `FLAG_RETRIEVE_INTERACTIVE_WINDOWS | FLAG_INCLUDE_NOT_IMPORTANT_VIEWS | FLAG_REPORT_VIEW_IDS`. Construction (spike A1): `new UiAutomation(Looper, IUiAutomationConnection)` is public on API 26 and 37 and works on both (API 37 also has `(Context, conn)` and a private `(int displayId, Looper, conn)`); then the hidden `connect(int flags)` by reflection. Connect takes 12 to 14 ms on API 26 and 24 to 43 ms on API 37, and the first `getWindows()` can be empty for about 20 ms. Tree: call `UiAutomation.clearCache()` first (API 33+; on older levels `AccessibilityInteractionClient.getInstance().clearCache()` by reflection), because the node cache is invalidated by content-change events and we deliberately subscribe only to window events, so without it a tree read after an action returns the old text (A2 saw a touch counter stuck at its old value). Then every window from `getWindows()`, walked with `getChild(i, FLAG_PREFETCH_DESCENDANTS_DEPTH_FIRST)` on API 33+ (plain `getChild(i)` below), serialized as a flat array with parent indexes. `FLAG_PREFETCH_DESCENDANTS_HYBRID` on the window root was not better than the default: roughly one binder round trip per 2 to 4 nodes whatever the mode (a prefetch call is capped at 50 nodes), and depth-first on every `getChild` cut the round trips by about 45% on the Settings main screen (99 to 52) and gave the best time (A1 table). Task 2.10 added `FLAG_PREFETCH_UNINTERRUPTIBLE` to those flags: without it the next `getChild` cancels the prefetch in flight, and on win-pc Settings main (155 nodes) spent 78 to 103 ms in `getChild` against 41 to 46 ms with it (`getChild(i)` with no flag: 118 to 137 ms). The agent leaves out full-width system windows of at most a tenth of the screen height on the top or bottom edge (the status and navigation bars): Settings main went from 192 to 155 nodes and the tree from p50 104 ms to 84 ms over the tunnel (task 2.2, API 37). A read that comes back hollow (no window other than the edge bars, or at most one node per window) is repeated every 20 ms for up to 800 ms: right after a navigation `getWindows()` lists the status bar only, or the closing activity's window with no children (API 37, Back from a second activity, task 3.1). On API 26 a connection made while a screen animates forever sees no window until the first window change (neither waiting, nor `setServiceInfo` again, nor a tap or key changes that), so once per connection the agent opens and closes the notification shade, which is a window change and repairs it; 16 of 16 connections on the Busy tab were repaired, 4 of 16 had been broken. The keyguard state comes from `IWindowManager` directly, because `KeyguardManager` cannot be created from `app_process` on API 37 (`ApplicationSharedMemory not initialized`). Events: window-state and windows-changed only, pushed as `window_changed { window_id, package, t }`. Spike A1 saw one navigation produce 14 to 21 `TYPE_WINDOWS_CHANGED` and 5 `TYPE_WINDOW_STATE_CHANGED` events over about 2 s, so a 16 ms coalescing window does not turn them into one event; the settle detector treats every one as activity and `quiet` ends the storm. A pure scroll produced none of them (it produced only `TYPE_VIEW_SCROLLED` and `TYPE_WINDOW_CONTENT_CHANGED`, which we do not subscribe to), which is why scrolling is seen only through frames. Never calls `waitForIdle`. A second UiAutomation client is detected by the exception text `UiAutomationService ... already registered!` (an `IllegalStateException` from `registerUiTestAutomationService`), which is what `AutomationConflict` matches; `uiautomator dump` hit by the same conflict dies with exit 137 and prints nothing on stdout (the text is in logcat only).
- **Text.** Three modes:
  - `Paste` (default; on a hidden display (D32), a native field that offers `set_text` gets `Set` instead, because the paste's clipboard toast shows on the phone's own screen): keep the current `ClipData` object in agent memory (any type: text, URI, image), set ours, inject `KEYCODE_PASTE` with WAIT_FOR_FINISH, then restore the saved `ClipData` only after the field's text is seen to change in the tree or 500 ms pass, whichever comes first. WebView and some Compose fields read the clipboard asynchronously, so restoring right after injection can paste the old clip (spike A4, Chrome on API 37: restoring at 0 ms pasted the old clip into the page's input; restoring at 50, 150 and 500 ms pasted the new text). The agent thread that touches `ClipboardManager` needs a Looper on API 26 (`Looper.prepare()`), or the call throws "Can't create handler inside thread that has not called Looper.prepare()". On API 33+ SystemUI draws a clipboard preview over the bottom left of the screen for every new clip, and that window takes the next tap there: on the API 37 emulator its touchable region covered 525 to 599 pt, and a tap on an httpbin checkbox at y 577 right after typing into the form did nothing, with `adb shell input tap` as with our injection (run a5c). `IS_SENSITIVE` only masks the text in it. SystemUI skips the preview when a clip set by the shell carries `com.android.systemui.SUPPRESS_CLIPBOARD_OVERLAY`, so every clip the agent sets (ours, the restored one, and `clipboard set`) carries that extra. Android 13+ still shows a "<app> pasted from your clipboard" toast when the focused app reads the clip; it takes no touches. Full Unicode. Respects the app's own input handling.
  - `SetValue`: `ACTION_SET_TEXT` on the target node. Replaces the content exactly; some apps ignore it (no IME events).
  - `Keystrokes`: `KeyCharacterMap.load(VIRTUAL_KEYBOARD).getEvents(chars)` injected one by one. That call returns `null` for characters it cannot map (spike A4: emoji and also `é`; plain ASCII works); the agent checks every character first and fails the whole request with the first unmappable character, before typing anything. It takes 1.6 to 2.4 s for 100 characters (about 16 ms each with WAIT_FOR_FINISH), so it is a fallback, not a speed path.

  After any mode, the session reads the node's text back and reports `verified: true|false`. A password field exposes masked text, so for `SecureField` targets `verified` compares lengths only. Plain text is refused (`secret_field`, -32020, decision D31) when the field is a password field or a text field whose label, hint or resource id names a one-time code (`daemon::secret_field`; the element model has no input type or autofill hint, so the words are all there is); `secret: true` is the only way in, and clearing a field is not refused. `type --into` REPLACES the field's text (fill semantics, decision D22): when the field holds text, the daemon taps it, empties it (`SetValue("")` where the device can set values; otherwise backspaces, tapping the field's right edge first so the caret is at the end), then enters the text; `--append` keeps the text. Without `--into` the text goes after the caret as before. When neither the last observation nor a fresh read shows a focused text field, and no keyboard is shown, `type` without `--into` fails with `not_focused` (-32023) and types nothing; the hint names up to five text fields on screen. Found in agent run a3: a tap on Wikipedia's search icon in Chrome while the page loaded opened nothing, and the text was pasted into nothing and reported as typed. A keyboard that opens shrinks a form's list, and the fields below it leave the tree while the agent still holds their refs (Contacts, run a4: three `type` calls planned from one screen, and the third failed with `stale_ref`). The session remembers the text fields that a later screen of the same app dropped (32 at most). `type --into` a stale ref it remembers, or a text that matches no element and is in the label of exactly one remembered field, drags the field's list toward where the field was, at most 4 times, until exactly one field with the same role and label shows, then types there. The summary starts `scrolled down 1 time to "Phone (Mobile)", then typed …`, and the reply diffs against the screen the agent was shown, so the agent sees what scrolled in. `tap` keeps failing on a stale ref, because a button's label ("Delete") does not say which one it was. A `false` carries `unverified: {shows, hint}`: what the field shows (`@e5 shows "10"`, never the text of a secure field) and a hint chosen by cause (length limit when the field kept a prefix, filtered characters when it kept a subsequence, nothing taken, focus on another field, field gone, field exposes no text). Found by dogfooding the Clock alarm: `type "07"` appended to a full two-digit hour field, `type ""` reported verified without emptying it, and the hint said `--mode keys` whatever the cause. The iOS backspace path is not proven on a Simulator yet. `type` (without `--submit`) settles with a 150 ms first-change wait and 150 ms quiet instead of 500 and 250: the paste already waited for the field to change, so no effect is still to come (a no-change settle after a type cost about 500 ms of the 700 to 850 ms an agent saw per step). `--submit` sends Enter (Android `KEYCODE_ENTER`, iOS Return) after the text. Web pages (decision D26): a field inside a `WebView` is followed after the tap, because a page can replace it with another focused field a few hundred ms to seconds later (the text goes into the field that ends up focused, and `verified` is about that one); the Android agent's focused input node in a page is the `WebView` itself, which refuses `ACTION_SET_TEXT`, so a replace there is Ctrl+A then delete or paste. Safari on iOS zooms in on a field whose font is under 16 px when it takes focus, and WebKit then reports an element that runs past the left edge with `x = 0` and its whole width, so taps on the zoomed page miss. When the followed field reads at least 1.3 times wider than before the tap, `type` pinches the page back out after the text and before `--submit`, and its summary says `then zoomed the page back out`. Not on Android: Chrome zooms in on a small field too, but it reports where elements are, and it zooms back in on the field while the keyboard is up, so the pinch did not hold and cost about half a second per field (httpbin form, run a5: `settled in` 2.7 to 3.1 s with the pinch, 2.4 s without). Where the zoom remains (the agent zoomed in itself), `tap` aims a web element that starts at the left edge no more than 8 pt in from it. WebKit calls every field on the page focused while none is being typed into, so when more than one web field claims focus, `Rendered::build` clears the state from all of them; the snapshot shows no field focused, and the followed field stays the one the tap aimed at. A `--submit` whose field no longer shows the typed text, after the screen changed beyond the field and the keyboard, is neither `verified` nor `NOT verified` (Chrome's address bar shows the completed URL).
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
| IME shown | Header `keyboard shown`; elements under it get `under-keyboard`. The platform calls a view the keyboard covers invisible to the user, so `prune` keeps an on-screen node that lies inside the keyboard window's bounds (flag `v` without `V`). `tap` on an element marked `under-keyboard` hides the keyboard, finds the element again and taps it (summary `hid the keyboard, then tapped …`; agents answered the old refusal with a hide-keyboard call and the same tap). If the element is still under a keyboard that stays, the tap fails `OffScreen` with `behind_keyboard: true` and sends no tap. A diff that shows the keyboard close says `keyboard hidden` once instead of listing each element it had covered |

Display 0 is the default. On Android 13 and later `--display hidden` works on a virtual display instead (decision D32, `docs/plans/2026-11-08-10-hidden-display.md`): the agent is started with a third argument `hidden`, creates the encoder and the display before it prints `ready` (`Hidden.java`, the host waits up to 20 s instead of 3), and reports `hidden_display` in `hello`. Windows, focus, injected events, launches, foreground and video are all scoped to that id; power state reads as on and unlocked.

## 7. iOS

Three tiers, chosen automatically per device and reported in `phonedriver devices`:

| Tier | When | Screen | Input | Tree | Apps | Install on phone |
|---|---|---|---|---|---|---|
| **CoreDevice** | iOS 27+ physical device, Developer Mode on | RTP H.264 stream (displayservice) | HID: multitouch, buttons, keyboard | Runner if installed; else OCR | app_service | Nothing (runner optional) |
| **Runner** | iOS 17 to 26 physical device | Runner screenshots, DVT screenshot | Runner (`XCSynthesizedEventRecord`) | Runner | app_service or runner | PhonedriverRunner (signed) |
| **Simulator** | Booted simulator on a Mac | Runner screenshots | Runner | Runner | `simctl` | PhonedriverRunner (unsigned) |

Honest consequence: without the runner, a CoreDevice-tier phone has no accessibility tree, so snapshots are OCR text only, with no roles, no switches' states, and no icon-only buttons. The full product on iOS needs the runner, which needs code signing (a free Apple ID re-signs every 7 days). Spike I5 (Phase 6) tests whether the Accessibility Inspector audit service (go-ios `ios/accessibility/`) can give a signing-free tree fast enough; if it can, it replaces OCR as the CoreDevice tree source.

Not available on iOS in v1: `press recents` (no API; returns `Unsupported`).

### 7.1 Transport

`idevice`: usbmuxd discovery and lockdown pairing (the user taps Trust once), then the CoreDevice tunnel over the userspace TCP stack, then RSD service lookup. Works on macOS, Linux, Windows. DDI: on macOS, use Xcode's personalized DDI files; elsewhere, `phonedriver ios setup` downloads them only after the user confirms the source. Mounting needs Apple's TSS server.

### 7.2 CoreDevice tier

*Update 2026-11-06:* the receiver defers the quarter turn to screenshots (`Frame::turned_cw`), derives the settle thumbnail from the unturned luma, and `receive::Pacing` shows the newest picture every 100 ms during a backlog and skips publishing above 400 waiting datagrams. A release build followed 95 alternating taps over 135 s with no missed change (docs/spikes/ios.md I6 follow-up). Open: no accessibility tree and no orientation handling, so Runner stays the default.

- Start the display media stream first. HID input is silently discarded without it (idevice `hid.rs:1-11`). The stream stays up for the life of the lease.
- Codec: the offer lists only the H.264 bank (RTP payload type 123). Spike I3 (iPad, iOS 27.0) showed the device accepts it and sends H.264 (High, level 5.2, 2736 x 2064). With idevice's default two-bank offer the device sends HEVC on payload type 100, so idevice's labels are right and go-ios `ios/display/offer.go:35-38` is wrong. The video crate depacketizes RFC 6184 (single NAL, STAP-A, FU-A) and the device's parameter packet (a 4-byte header, then an `avc1` sample entry whose `avcC` holds SPS and PPS).
- Touch: points → normalized 0..65535 per axis. Multitouch up to 5 contacts.
- Buttons: Home, Lock, Volume via `send_button` (button usage codes confirmed in spike I1). `back` taps the navigation bar's back button when the tree shows one (`via: "nav-button"`), else performs a left-edge swipe (`via: "edge-swipe"`).
- Text: pasteboard `set_text`, then HID keyboard Cmd+V. Observed on the iPad (iOS 27.0, spike I1): Cmd+V raises the system prompt "Testbed would like to paste from dtpasteboardd" every time, and the text arrives (accents, emoji, CJK intact) only after someone taps "Allow Paste"; so Paste mode reports `ApproveSystemPrompt` and Keystrokes (US layout, no prompt) is the prompt-free path. Creating the HID keyboard makes iOS treat a hardware keyboard as attached: the software keyboard does not show (the runner's snapshot says "keyboard hidden" with a field focused). A modifier sent in the same report as its key is ignored (paste arrived as "V", Shift+A as "a"), so keys go down one report at a time, modifiers first, and up in reverse, 25 ms apart (10 ms dropped the character after a Shift).
- Frames: the stream is the panel's landscape frame with a portrait UI rotated a quarter turn counterclockwise, so the decoder output is rotated clockwise (2736 x 2064 becomes 2064 x 2736, 1032 x 1368 pt at 2x; the real screen is 1032 x 1376 pt, so a tap at a point of the frame lands 0.6% further down on the glass: the frame's normalized coordinates are the screen's). Orientation other than portrait is unobserved. The stream was not stale, it was late (2026-11-06, root cause): the receiver published every decoded picture (copy, rotation, thumbnail) and in an unoptimized build that cost about 190 ms against a decode of 11 ms, while the device sends 20 pictures a second, so the picture shown lagged by 2 s after 10 s and 34 s after 40 s, and a screenshot showed a screen from long ago (status bar included; the earlier "status bar updates" was a transitional picture). `phonedriver_video::receive::decode_stream` now decodes every access unit (each is a reference for the next, and each is acknowledged) and publishes only when no datagram is waiting. Feedback was ruled out: acknowledging with the RTP timestamp, with the RTP extension's frame counter, with no acknowledgments, with no receiver-control reports, and without audio all behaved alike (docs/spikes/ios.md, I6). Still open: when the host is starved of CPU (load average above 20 while other builds ran) the decode thread cannot keep 20 pictures a second and the backlog returns; the Runner tier stays the default until a release build is measured on an idle host.
- Keyframe on demand with RTCP (idevice `display_stream/rtcp.rs`) when the session settles, for a sharp screenshot.

### 7.3 PhonedriverRunner

An XCTest UI test bundle (`ios-runner/`), Objective-C for direct use of private headers copied from WebDriverAgent. One test method that never returns: it opens a TCP listener on device port 27753 and serves our framing.

Requests: `snapshot { max_depth }` of the foreground application (first of `XCAXClient_iOS activeApplications`, `requestSnapshotForElement` with bounded `maxDepth` and eight raw AX attributes), `gesture` (synthesized event record), `screenshot { quality, format }`, `type` (a synthesized text-input event with `shouldRedact`, because `XCUIElement typeText:` writes the text into xcodebuild's output and the `.xcresult` bundle, which the secrets rule forbids), `button` (home and volume via `XCUIDevice pressButton:`, volume unavailable on the Simulator; lock via `pressLockButton`, which WDA's `fb_lockScreen` uses on simulators, while on iOS 27 devices WDA sends an IOHID power press instead, so Phase 6 must do the same), `activate { bundle_id }`, `open_url`. Wire details in `docs/protocol.md` section 3.

The runner serves one request at a time. Every request runs on the main thread as a main-run-loop block (`CFRunLoopPerformBlock`), not a main-queue block, because event synthesis completes through main-queue callbacks that a running main-queue block would deadlock. XCTest reports UI failures as test issues; the runner's `recordIssue:` turns each into an error reply instead of ending the test. Snapshot and screenshot both run on the main thread, so the Runner and Simulator tiers poll for settle more slowly than the CoreDevice tier (section 12). Measured on the iPhone 17 simulator (iOS 26.5): snapshot 13 to 25 ms, screenshot about 75 ms JPEG and 63 ms PNG (`docs/spikes/ios.md`). Every synthesized event (tap, swipe, typing, key) costs about 230 ms more than its own duration: the testbed logs the tap 30 to 50 ms after the request, but XCTest's completion arrives at about 240 ms, for an empty spot as for a button; the session's `implicitEventConfirmationIntervalForCurrentContext` (5 s by default) made no difference at 0 or 0.05. The runner waits for the completion, so the Simulator tier misses the 80 ms tap budget (decision D15).

Simulator tier (task 5.5, `crates/ios/src/device.rs`). `tree` is the runner's snapshot with the foreground app as the application window and the keyboard, when shown, as an input-method window. Back: the button whose identifier is `BackButton` (UIKit and SwiftUI both name the navigation bar's back button so), else a drag from the left edge to 70% of the width over 300 ms, which also goes back in a web view with back-forward gestures; a hidden navigation bar disables the interactive pop, so the edge swipe then does nothing. `hide-keyboard` taps a "Hide keyboard" key when there is one, else a `Done` button that ends at most 48 pt above the keyboard (Safari's form bar: Previous, Next, Done, 32 pt above the keys; a drag left Safari's keyboard up, run i8f), else drags the content above the keyboard up. `paste` and secret text type through the runner's redacted text input at 5000 characters per second: a synthesized Cmd+V pastes text the app copied itself but not text another process put on the pasteboard (runner or `simctl pbcopy`; no prompt appears, nothing is pasted), so the pasteboard is never used for typing and a secret never lands there. `keys` mode types at 60 per second. `clipboard` uses `simctl pbcopy`/`pbpaste` with a UTF-8 locale (`pbcopy` reads Mac Roman otherwise). `apps` is `simctl listapps` converted by `plutil`; `launch` is `simctl launch`; `open` is the runner's `open_url`, which unlike `simctl openurl` raises no confirmation for custom schemes. `press notifications` drags down from the top edge at 20% of the width. Volume and recents are `Unsupported`.

As built in task 5.6: the tree drops XCUI's scroll indicators ("Other" elements labeled "Vertical scroll bar, 2 pages" with a percentage value, matched on the English label the Simulator uses), which added two to four lines per list and, as the only labeled child of a multi-line field or a Files rename field, became that field's label. A toggle row (Settings, SwiftUI `Toggle`) is a labeled switch the width of the row around an unlabeled switch, the control itself; a tap at the row's center does not toggle, so the row takes the inner switch's bounds and the inner switch is dropped. For about 1.3 s after `simctl launch` returns, the runner's snapshot fails with `kAXErrorServerNotFound` or still reports the app that was in front (every launch of Settings), so `launch` returns once the snapshot reports the launched app, and a tree read retries `kAXErrorServerNotFound` for up to 5 s. `SimSource` reuses a simulator listing for 2 s: the daemon lists devices on every request and `simctl list` takes about 70 ms.

Host connects through idevice's device-port connect (as `services/wda.rs` does), so no host port forwarding. usbmuxd reaches a device's TCP ports from a non-loopback address, so on a device the runner listens on `0.0.0.0` (the Simulator build stays on `127.0.0.1`); anything on the device's networks can reach port 27753 while the runner runs (observed 2026-11-06: a loopback-only listener was not reachable, the all-interfaces one was). Launch: on devices, `xcodebuild test-without-building -destination id=<udid>` signed with `PHONEDRIVER_TEAM` (idevice `xctest` is the way for a machine without Xcode and is not built yet); on simulators, `xcodebuild test-without-building -xctestrun ... -destination id=<udid>` as a child of the daemon, with the port passed as `TEST_RUNNER_PHONEDRIVER_PORT` (xcodebuild only forwards `TEST_RUNNER_`-prefixed variables to the test process, without the prefix). Readiness: the daemon polls the port with `hello` every 100 ms up to 60 s; it does not rely on xcodebuild's log output.

Gestures to XCTest (2026-11-09). `map::gesture_request` sends a path in fewer points and wiggles a still finger. XCTest moves the finger in a straight line between the points it gets, and each point costs time: a 400 ms scroll of 17 points took 1,240 ms to play, and the same scroll as 5 points took 870 to 950 ms over the same distance. So an interior point on a straight, constant-speed segment is dropped, and still samples are kept at least 50 ms apart. XCTest also sends no touch-moved event for a point that does not move, so UIKit kept the last real velocity and a scroll that held still before lifting still flung Safari 143 to 194 pt further, with a 2.6 s settle. Each kept still sample moves half a point up or down in turn, and the drift is 0. A Safari `scroll down` went from 4.5 s to 2.0 to 2.2 s.

Orientation (task 7.3). The simulator rotates only through the GUI or an XCUITest (`XCUIDevice.orientation`); the app's frame follows it. The runner's synthesized touches are expressed in the portrait frame whatever the orientation (observed on an iPad Pro 13-inch simulator in landscape: a tap at (519, 438) landed at (438, 513); the record's `interfaceOrientation` made no difference), so `Gesture.m` rotates each point back by `XCUIDevice.orientation`. The screen size in the header and in swipes comes from the application element's frame on every snapshot (`Device::current_screen`), not from the size the runner reported at connect.

Simulator lifecycle (task 5.4, `crates/ios/src/runner.rs`). The runner's sources are compiled into the `phonedriver` binary (`include_str!`), so the daemon builds the runner wherever it runs: it writes them to `~/.phonedriver/runner/<hash>/src`, runs XcodeGen and `xcodebuild build-for-testing` for `generic/platform=iOS Simulator` once per source hash, and deletes builds of other hashes (2.4 s on the Mac mini with Xcode's module caches warm; the build log is `build.log` next to it). Ports: the first port from 27753 to 27784 that nothing listens on and no other simulator of this daemon claimed. Reuse: before launching, the daemon says `hello` on each runner port; a runner whose `hello` reports this simulator's `udid` and this binary's source hash as `build` is reused. Process groups: `xcodebuild` runs in the daemon's process group (the CLI starts the daemon as a group leader), and killing `xcodebuild`, even with `SIGKILL`, ends the runner inside the simulator (observed on the mini). So dropping the device or killing the daemon's process group ends the runner; a daemon killed alone leaves `xcodebuild` and the runner running, and the next daemon reuses that runner. The runner's output goes to `~/.phonedriver/runner/<udid>.log`; typed text never reaches it (section 7.3, `type`).

Private headers drift between Xcode releases. The runner is built and tested against the Xcode on the build machine (27.0 at planning time); a build failure after an Xcode update is expected maintenance, not a design flaw.

Signing: built with Xcode automatic signing for the user's team. A free Apple ID works and expires every 7 days; `phonedriver devices` shows the expiry. Signing without a Mac is out of scope for v1 (see "Known gaps").

### 7.4 iOS states

| State | Behavior |
|---|---|
| Locked with passcode | `NeedsUser(UnlockDevice)` |
| Developer Mode off | `NeedsUser(EnableDeveloperMode)`; idevice `amfi` reveals the toggle in Settings |
| Not paired | `NeedsUser(TrustComputer)` |
| Runner expired | `NeedsUser(ResignRunner)` with the exact command |
| Tunnel dropped | Actor reconnects once, then `DeviceGone` |

## 8. Video (`phonedriver-video`)

- Input: H.264 Annex-B access units from the Android agent, or RTP packets from iOS depacketized into access units (section 7.2).
- Decode: `openh264` for H.264 on every OS. The CoreDevice tier asks the device for H.264 only (spike I3), so no HEVC decoder is needed.
- Output, `FrameStore`: the latest decoded frame (`Arc`, planar YUV 4:2:0), a luma thumbnail with one sample per 4×4 points (the settle measure of section 4.6), a sequence number, and the time of the last frame, with a `watch` channel carrying `FrameRef { seq, changed_area_pt2, changed_box }`. Every frame is decoded (inter-frame coding forbids skipping).
- Screenshot = encode the latest frame (PNG or JPEG) on the host. A static screen can send no frames for seconds (observed in task 2.7 on the API 37 emulator through the tunnel: bursts at about 10 frames per second, then gaps of up to 5 s with nothing changing on screen), so a quiet stream is not a broken one and the last frame is a correct screenshot. The stream is broken only when its socket ends (`VideoStream::ended`, which `AndroidDevice::screenshot` reports as `DeviceGone`, and which makes the actor reconnect on the next call). Before a screenshot serves that frame it sends the agent `video_barrier` and waits up to 1.5 s for the barrier packet to come back through the video stream (protocol section 2): the barrier proves the stream still delivers and has been decoded up to now. Observed on API 26 through an adb tunnel: after the agent was killed, which is what a second daemon does, neither socket reported it for 5 to 21 s, so `ended` stayed false, a screenshot answered with the last frame and tree reads hung. A barrier that does not come back is `DeviceGone`, and the daemon reconnects once and takes the screenshot from the new agent. `screenshot --marks` reads the tree first and the frame after it, so the picture is never older than the boxes drawn on it. The first frame after a connect is awaited for up to 2 s. Screenshots requested while an action is settling wait for settle, so encoder pipeline latency cannot return the pre-action frame.

## 9. Pixels and OCR

For canvases, games, WebViews without accessibility, and CoreDevice devices without the runner.

- `Source::Ocr` elements carry `role: Text`, a label, bounds, and confidence. They get refs like any element.
- Engine: Apple Vision on macOS (`objc2-vision`), `VNRecognizeTextRequest` revision 2 at the Accurate level (revisions 1 and 3 take 18 to 36 s and often fail on macOS 27.0.1; revision 2 takes about 45 ms on a 411x923 frame). The first call after a long idle can take about 20 s while the Neural Engine loads. Accuracy depends on resolution: at 1 pixel per point small gray text (13 pt button labels, tab bar captions) is misread, at twice that most is read, so OCR should be fed `--native` frames. `core::ocr::merge` drops a box whose text matches a tree element over it (IoU > 0.5) and appends the rest as `Source::Ocr` elements with `TAP`, printed with `(ocr)`. On Linux and Windows, `snapshot --pixels` returns `Unsupported` in v1; add `ocrs` when a user needs it there.
- `snapshot --pixels` forces OCR. When the tree is empty or a `WebView` has no children, the session merges OCR boxes into the snapshot automatically and marks them `(ocr)`. As built (task 7.2): the daemon screenshots and recognizes on the Mac after reading the tree; an OCR failure in the automatic case is ignored (the tree alone is shown), with `--pixels` it is the reply. A box that matches an OCR line of the last snapshot keeps that line's ref, and while the last snapshot used OCR, ref and text targets are resolved against a tree with OCR merged in, so `tap "Send"` taps an `(ocr)` line. Tested on the testbed tab `webopaque` (a WebView whose `getAccessibilityNodeProvider` returns null; `setImportantForAccessibility` and a view delegate do not hide Chromium's tree).

## 10. Agent surfaces

### 10.1 CLI verbs

```
phonedriver devices                          list devices, tier, capabilities, leases
phonedriver snapshot [--diff] [--interactive] [--within @eN] [--pixels] [--max-lines N]
phonedriver screenshot [-o file] [--marks]
phonedriver find <text> [--role R]           refs matching text
phonedriver tap <@ref|x,y|"text"> [--role R] [--long [ms]] [--count N]
phonedriver type <text> [--into <target>] [--mode paste|set|keys] [--submit]
phonedriver type --secret [--into <target>]  text from stdin or PHONEDRIVER_SECRET, never argv
phonedriver clear <target>
phonedriver swipe <up|down|left|right> [--from <target>] [--distance 0..1] [--fast]
phonedriver drag <from> <to>                 press, hold 400 ms, move over 400 ms, hold 100 ms, lift
phonedriver pinch <in|out> [--at <target>] [--distance 0..1]
phonedriver scroll <up|down|left|right> [--within <target>] [--until "text"|@ref] [--max N]
phonedriver gesture <json>                   multi-finger paths with timing
phonedriver press <home|back|recents|notifications|lock|volume-up|volume-down|enter|hide-keyboard|...>
phonedriver launch <app name|package|bundle id> [--restart]
phonedriver open <url>                       deep link or web URL in its handler app
phonedriver apps [install <path> | stop|clear|uninstall <app>]   list launchable apps, or manage one
phonedriver wait <"text"|@ref|gone:"text"|settled> [--timeout ms]
phonedriver batch "<step>" "<step>" ...      steps in order, one diff at the end, stops at the first failure
phonedriver clipboard [get|set <text>]
phonedriver shell <command> [--grep words] [--head n] [--force] [--time-limit s] [--secret]
phonedriver release
phonedriver daemon [--detach] | mcp | doctor | ios setup | version
```

Global flags: `--device` (default from `PHONEDRIVER_DEVICE`), `--session`, `--json` (machine output), `--no-settle`, `--timeout`.

Scroll mechanics: `scroll` drags 60% of the largest scrollable container (or `--within`; `--in` is a hidden alias) in 10 steps over 250 ms, then holds 150 ms with a stationary sample every 25 ms before the lift (Android 8 flings a list that lifts after a gap with no samples), so it never flings. `scroll --until "Label N"` flings when the visible labels share the prefix, the target is over three screens away and the device has a frame stream (`FRAMES`; a fling moves nothing on the polled iOS runner, observed in the agent-experience audit, so there it drags one screen at a time), and turns back on overshoot; other targets scroll one drag at a time. Without `--max` the limit is 30 drags, or enough one-screen drags to cross a numbered target's distance (at most 300). The search ends with `NotReached` (code -32016) when two drags in a row change nothing (`the list ended at "Developer"` after it had moved, `the list did not move` when it never did) or the limit is hit (`scrolled 30 times and "Row 640" is still not on screen`); `NotFound` is only for a target that cannot be resolved.

`shell <command>` (decision D21, `docs/bench/history/shell.md`): Android runs `sh -c` in the agent over the control stream (agent op `shell`, own thread pool); the Simulator runs `simctl spawn <udid> /bin/sh -c`, or `simctl <sub> <udid> ...` for a command starting with `simctl`; a physical iPhone is `Unsupported`. The daemon refuses a short list of destructive commands on a physical device unless `force`, settles after commands that are not read-only and returns the screen diff, and trims output (exit first, `grep`, `head`, ANSI and CR removed). The trace keeps the command (length only with `--secret`) and the output length.

`clear <target>` taps the field to focus it, then sets its value to nothing with `ACTION_SET_TEXT` and reports `verified` when the field then shows no text (`via: set`). `clipboard set <text>` takes the text on the command line (it is not a secret; use `type --secret` for secrets), and `clipboard` prints the device clipboard; the daemon does not trace either.

`wait "text"`, `wait @ref` and `wait gone:"text"` read the tree once at the start and then once after each window change or frame with more than 128 pt² of change (100 ms after the burst's first signal), never on a timer; the reply's summary says how many tree reads it took. They resolve targets like `tap` does (whole-word match included, D33), return a full snapshot, and fail `Timeout` naming the condition (default timeout 5 s). A text target in `scroll --until`, `wait` and `find` may name alternatives as `A|B`: the first of them on screen ends the search (agents wrote `"Airplane|Wi-Fi"` and got `not found` while Wi-Fi was a row away), and an alphabetical list is not flung for it. `tap` keeps one text. `snapshot --within @eN` prints only that element's subtree under the usual header, with that element's own text uncut, and a ref the screen no longer has is `StaleRef`.

`screenshot --marks` (MCP `screenshot` with `marks`) draws a 2 px box and the `eN` label on every element that takes a tap, a long press or text, using the same fresh tree read a tap would use, so the label in the picture is the ref of the next `tap @eN`. Screenshots are one pixel per point, so element bounds are pixel coordinates. The reply says how many elements were boxed. The drawing is in `phonedriver-video::marks` (a 3x5 bitmap font, no font dependency).

`apps stop|clear|uninstall <app>` and `apps install <path>` (daemon method `app`, `Device::app_op` and `Device::install`) manage an app. Android: `am force-stop`, `pm clear`, `pm uninstall`, and install pushes the `.apk` over adb sync to `/data/local/tmp` and runs `pm install -r`; the new package id is the one in `pm list packages` that was not there before (a reinstall of an existing app has no new id, and the reply says it replaced one). iOS Simulator: `simctl terminate`, `simctl install` of an `.app` folder (the id is read from its `Info.plist`), `simctl uninstall`, and clear data empties the app's data container. The path is on the daemon's machine. iOS devices answer `Unsupported` (installation_proxy install and removal are not implemented). Safety on a physical device (any `ios:` device, and any `android:` serial that is not `emulator-*`): `clear` and `uninstall` refuse an app phonedriver did not install; installs are recorded per device in `installed.json` under the daemon's home and the record ends with the uninstall. `stop` is allowed everywhere. The MCP `apps` tool takes `action` and `app` for these.

`batch` runs a short sequence in one call: `phonedriver batch "tap @e3" "type hello --submit" "press enter"`. Each step is the command line of tap, type (not `--secret`), clear, swipe, scroll, gesture, press, launch, open or wait, parsed by the same parser. The daemon (`batch` method) runs the steps one after another in one lease, each acted on and settled like a single call (so a later step resolves its target on the screen the earlier one produced), and answers once: the numbered one-line summaries of the steps that ran, and the diff from what the session last observed before step 1 to the screen after the last step. The first step that fails ends the batch: the reply names the step (`step 2 of 3 failed: ...`), keeps the error's code and hint, lists the steps that did run, and shows the diff so far, so the agent knows the screen it now has. Steps after a failure never run. A malformed step is refused before any step runs. Typed text in a batch is traced by length like any `type` (D16); secret text is refused, because it must come from stdin or the `secret` flag of a single call. The MCP `batch` tool takes `steps`, objects of `tool` plus that tool's arguments (`{"tool":"tap","target":"@e3"}`), converted to the same wire steps.

Defaults: `scroll --max 30`; `swipe --distance 0.5`; `tap --count 1` (2 = double tap, taps 80 ms apart); `swipe --from` takes a target and starts the gesture at its center.

How the less obvious verbs work per platform:

| Verb | Android | iOS CoreDevice | iOS Runner / Simulator |
|---|---|---|---|
| `press notifications` | StatusBarManager `expandNotificationsPanel` (scrcpy wrapper) | swipe down from the top-left edge | same swipe via runner |
| `open <url>` | ActivityManager VIEW intent carrying `Browser.EXTRA_APPLICATION_ID` `dev.phonedriver`, so a browser reuses one tab for every `open` instead of adding a tab each time (Chrome still opens a new tab when the reused tab holds text typed into a form, so typed data is not lost). The agent replies with the package that handles the link. When that app is already in front, `open` does not wait for it to leave: the wait for another app ran out its full 2.5 s on every `open` in the browser that was already in front (2.9 to 3.1 s to settle, then 1.1 to 1.8 s). The iOS runner cannot say which app opened a link; on the Simulator an http or https link always opens in Safari (no other browser can be installed there from the App Store), so the Simulator tier names Safari, and a physical iPhone, whose default browser may differ, names none (with Safari in front, an `open` took 3.7 to 5.0 s, then 2.3 to 3.0 s) | runner `XCUIDevice.shared.system.open` if installed, else `Unsupported` | runner on device and Simulator (`simctl openurl` asks "Open in …?" for custom schemes) |
| `press back` | `KEYCODE_BACK` | nav-bar back button, else left-edge swipe; `via` says which | same; with the keyboard shown, `back` hides it first, as Android does |
| `press hide-keyboard` | no-op when the keyboard is hidden (`back` would leave the screen), else `KEYCODE_BACK` | n/a | a "Hide keyboard" key if the keyboard has one, else an upward drag from just above the keyboard (forms that dismiss on drag: Contacts, Settings; a drag down dismissed Contacts' New Contact sheet and lost the draft in the first live try); if the keyboard is still there the call fails `Unsupported` and says to `press enter` or tap Done (the testbed's Plain field keeps its keyboard on a drag) |

Text targets resolve by: exact label, then the same label in any case and spacing, then a label that holds the text as whole words and has at most four other words (D33). More than one match fails with `Ambiguous` and lists refs (labels, for `scroll --until`, since refs do not survive a scroll). `find` is the plain substring search. An alternative that no label or hint holds and that is a role word (`radio|checkbox`) lists the elements of that role: in run a5b an agent checked a form that way and got `no match`, then scrolled to read it. No match fails with `NotFound`, whose `similar` lists elements whose text contains the query or is within 0.3 of it by normalized optimal-string-alignment distance (insertions, deletions, substitutions and swaps of neighbors, case ignored, so `Rwo 5` suggests `Row 5`; plain Levenshtein would put that swap at 0.4). A list line says its size (`list 1000 rows`); a text row of a list is a `cell`, with its position when the app attached collection item info (RecyclerView does only for rows bound while an accessibility service was connected).

### 10.2 MCP

`phonedriver mcp` with `rmcp`, stdio. Fourteen tools: `devices`, `apps`, `snapshot`, `screenshot` (returns an image content block), `tap`, `type`, `swipe`, `scroll`, `press`, `launch` (takes `app` or `url`; a URL maps to the `open` verb), `wait`, `batch` (a list of steps; see 10.1), `clipboard` (`set` text, or read), `shell` (a command in the device's shell; decision D21). Tool argument structs reject unknown fields, so a misspelled argument is an error and not silently dropped. `tap` also takes `ref` as another name for `target`, because agents that had not loaded its schema yet sent `{"ref": "@e27"}` in 2 of 50 audit runs and each paid for an error and a schema fetch; the schema names only `target`. Each tool's input schema mirrors the CLI verb; outputs are the same text the CLI prints. Few, well-described tools beat many. Every tool also takes an optional `device` (default `PHONEDRIVER_DEVICE`). `type` takes `secret: true` instead of the CLI's stdin route, since the text already crossed the MCP channel to reach the server; it is masked in the reply and traced by length only. A failed call returns `isError: true` with the message and `hint: ...` as one text block. `screenshot` returns an `image/png` block and a text block `WxH at scale S`. The server is one session `mcp-<pid>` on the daemon (started on demand like for the CLI) and releases it when stdio closes (or, if the client dies first, when the daemon sees the connection close). Calls run one at a time on that connection, in the order they arrived: the server runs on a single-thread runtime, because rmcp starts one task per request and on several threads two calls sent together took the lock in either order (5 runs of 4 swipes in one turn: 3 of 3 failed on the multi-thread runtime). The `instructions` and the descriptions of `tap`, `type`, `swipe`, `scroll`, `press` and `launch` say that calls share one screen and go one at a time (agents sent tap and type together in 7 of 9 audit runs). `apps` was added after an audit run could not find the testbed app (`launch "phonedriver testbed"` failed and the agent had no way to list names). Hints and `hint:` lines are rewritten into tool vocabulary before they reach the agent (`phonedriver snapshot` becomes `snapshot`, `--mode keys` becomes `mode` `keys`). The CLI verbs without a tool of their own are folded into one: `swipe` takes `to` (a drag from `from` to `to`, the `drag` verb) or `pinch` (`in` or `out`, centered on `from` or the screen, the `pinch` verb) in place of `direction`; `type` with empty text and `into` is `clear`; `snapshot` with `find` is `find`; `devices` with `release` is `release`. Only the raw `gesture` JSON, `open` (it is `launch` with `url`) and `snapshot --save-fixture` stay CLI-only. `drag` holds before it moves so lists and drag handles pick the item up; `pinch` puts two fingers on a horizontal line through its center and needs `MULTI_TOUCH`; both are `act` actions, so they settle and return a diff like a tap.

Agent-facing text has one home each (writing-for-agents pass, 2026-11-06): per-tool details live in the tool description and its argument docs, cross-tool strategy (shell for settings first, then the snapshot loop, batch, one call at a time) in the server `instructions` and `SKILL.md`. `list_tools` is the router's list with the argument schemas slimmed (`$schema`, `format`, `minimum: 0` and `null` in optional types removed). `tools/list` plus instructions went from 11,457 to 10,041 characters (about 2,938 to 2,575 tokens at 3.9 characters a token); `SKILL.md` from 6,312 to 4,597 characters. Each action tool keeps "One call at a time." because agents sent calls together before it was there. Token pass (`docs/audits/token-audit.md`): the prefix is re-read on every model turn and it is 4,204 measured tokens at 2.6 characters a token (not 3.9: JSON schemas tokenize densely), so `lean.rs` also hides from `tools/list` the `device` argument of every tool and the arguments no agent used in the audited runs (`snapshot` `diff`, `interactive`, `within`, `max_lines`, `pixels`; `tap` `role`, `count`, `long`; `type` `mode`, `append`; `swipe` `distance`, `fast`; `scroll` `in`, `max`; `shell` `grep`, `head`, `settle`, `force`, `secret`; `batch` `timeout_ms`). The server still accepts them and the hints that need one name it. Descriptions and argument docs were cut to what a model cannot guess. `tools/list` is 5,107 characters (was 10,987 before the dogfood pass).

The approval guard in the MCP server (decision D27): `tap` and `type` call `guarded()`. It runs the action; on `needs_approval` it asks the human through the client's elicitation (one form, choices `Allow` and `Allow for this session`, Decline and Cancel are no) and runs the action again with `approval: "once"` or `"session"` in the same tool call. On protocol 2026-07-28 the server may not send a request during a call, so `call_tool` (overridden for `tap` and `type`) answers with an `input_required` result holding the form and the client repeats the call with `inputResponses.approve`; older protocols use `elicitation/create` on the open connection. A client that does not declare elicitation gets a normal text reply that nothing was tapped and how to continue (`autonomous: true`). `autonomous` and `PHONEDRIVER_GUARD=off` send `approval: "autonomous"`. Not in `tools/list` (cost 0).

### 10.3 Skill

`SKILL.md` at the repo root, printed by `phonedriver skill`: the loop (snapshot, act on refs, read the diff), when to use pixels, how to handle `NeedsUser`, and the rule that the agent never enters passwords or payment details without the user's explicit instruction.

## 11. Development environment

- MacBook (this machine): Rust builds with `blaze`. Has Xcode 27, Android build-tools 37 and platform `android-37.0` at `/opt/homebrew/share/android-commandlinetools`, `d8`, `adb`, JDK 17.
- Android emulator: Windows PC `win-pc`, AVD `Pixel_API_37`, SDK `D:\Android\Sdk`. Reach it from the Mac with `ssh -N -L 5038:127.0.0.1:5037 win-pc` and `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`. All agent streams flow through the adb server, so the daemon on the Mac drives the emulator on the PC.
- Launch the API 37 emulator with `emulator.exe -avd Pixel_API_37 -port 5554 -no-snapshot -no-audio -no-boot-anim -no-window -gpu swangle_indirect` (the AVD's `config.ini` also says `hw.gpu.mode=swangle_indirect`). Without `-gpu swangle_indirect` the emulator picks lavapipe (software Vulkan; the old `swiftshader_indirect` mode is rejected at launch and falls back to it), and with lavapipe the emulator crashes within 2 to 10 s of the first WebView, with no UiAutomation client involved; when its crash reporter also crashes, the process is left suspended and `adb shell` never answers. With swangle 11 trials had no hang or death, but two emulators launched with it still stopped later (`docs/bench/history/m2.md`), so it is not shown to be sufficient.
- An API 26 emulator for the minimum-version checks: create `Pixel_API_26` on win-pc.
- iOS simulators and any GUI: the Mac mini (cua Space `mini` for shell and files, `mini-desktop` MCP for its screen). Never drive GUIs on the MacBook's own screen. On 2026-11-05 the mini had Xcode 27.0, the iOS 26.5 simulator runtime only (no iOS 27), Python 3.14, XcodeGen 2.46 (installed with Homebrew), and no Rust toolchain. Sources reach it by `cua` upload into `~/phonedriver-work/`.
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

Measured in task 2.10 (`docs/bench/history/m1.md`, win-pc, API 37 emulator, p50 of two 50-run series): snapshot Settings main `--no-settle` 60 and 68 ms (met), screenshot 15 and 14 ms (met), tap to settled diff 544 and 420 ms (met), tap 30 and 37 ms (missed), type 100 chars 253 and 288 ms (missed). The tap budget includes about 7 ms of adb transport and, with `WAIT_FOR_FINISH` on the last event, about 14 ms of the app handling it on a software-rendered emulator; injecting `ASYNC` reached 15.1 ms and was not adopted because "delivered" would mean "queued". The type budget is spent on clipboard and accessibility binder calls and on the wait that confirms the paste before the clipboard is restored. Both are re-judged on a physical phone in Phase 3.

Measured in task 5.6 (`docs/bench/history/m4.md`, Mac mini M4, iPhone 17 simulator, iOS 26.5, p50 of two 50-run series, release build): snapshot Settings main `--no-settle` 28 ms (met), screenshot 83 and 84 ms (met), type 100 chars 355 and 356 ms (met), tap to settled diff 712 and 717 ms (met), tap 359 and 357 ms (missed: about 240 ms is XCTest's event completion, D15, and up to 80 ms is waiting for an in-flight settle poll).

## 13. Security and privacy

- No telemetry. Nothing leaves the machine except traffic to the device and, for iOS DDI mounting, Apple's TSS server. `crates/cli/tests/network.rs` checks it under a macOS sandbox (decision D25).
- The daemon socket is user-only. No network listener.
- Never executes agent-supplied code.
- Secret text arrives on stdin or in `PHONEDRIVER_SECRET`, never on the command line (argv is visible in `ps` and shell history). It is never logged, traced, or echoed. Typing into a `SecureField` is treated as secret even without the flag. `SecureField` values never render.
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
