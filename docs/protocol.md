# Wire protocols

Three wires. Every message shape below is the contract the code is tested against. Field names are snake_case. Coordinates on the daemon wire are logical points (decision D5); device wires use device units, stated per field.

## 1. Client ↔ daemon: JSON-RPC 2.0, newline-delimited

Transport: local socket (architecture section 5). One JSON object per line, UTF-8.

### Handshake

```json
→ {"jsonrpc":"2.0","id":1,"method":"hello","params":{"version":"0.1.0","client":"cli"}}
← {"jsonrpc":"2.0","id":1,"result":{"version":"0.1.0","pid":4242}}
```

Version mismatch: the daemon replies with error code `-32010 version_mismatch` and `data.daemon_version`. The CLI then calls `shutdown` and starts its own daemon.

### Methods

| Method | Params | Result |
|---|---|---|
| `hello` | `version, client` | `version, pid` |
| `shutdown` | none | `{}` |
| `devices` | none | `[DeviceSummary]` |
| `release` | `device?` | `{}` |
| `snapshot` | `device?, diff?, interactive?, within?, pixels?, max_lines?, no_settle?, elements?, capture?, timeout_ms?` | `Observation` |
| `screenshot` | `device?, native?, quality?, format?` | `{path, width, height, scale}` |
| `find` | `device?, text, role?` | `[ElementLine]` |
| `act` | `device?, action: Action, settle?: bool, timeout_ms?` | `ActionResult` |
| `wait` | `device?, condition: WaitCondition, timeout_ms?` | `ActionResult` |
| `apps` | `device?, all?` | `[App]` |
| `clipboard` | `device?, set?: string` | `{text?: string}` |

`device` is omitted when exactly one device is connected or a default is set. Every device method also takes `session` (string, default `"default"`); refs, diff baselines and leases belong to the session, not to the socket connection (architecture section 5).

Additional methods: `open { device?, url }` → `ActionResult`. `share?: boolean` is accepted by every device method (architecture 5, leases). Typed text and secrets: a `type` action's text is masked (`•••`) in `observation.text` when `secret` is true, and the trace records only its length.

```ts
type Target = {ref: string} | {point: {x: number, y: number}} | {text: string, role?: Role}
type Action =
  | {kind: "tap", target: Target, count?: number}
  | {kind: "long_press", target: Target, ms?: number}
  | {kind: "type", text: string, into?: Target, mode?: "paste"|"set"|"keys", secret?: boolean, submit?: boolean}
    // the CLI reads secret text from stdin or PHONE_USE_SECRET and sends it here over the user-only socket
  | {kind: "clear", target: Target}
  | {kind: "swipe", direction: Dir, from?: Target, distance?: number, fast?: boolean}
  | {kind: "scroll", direction: Dir, in?: Target, until?: Target, max?: number}
  | {kind: "gesture", fingers: Array<Array<{x: number, y: number, t_ms: number}>>}
  | {kind: "press", key: Key}
  | {kind: "launch", app: string}
type WaitCondition = {appears: Target} | {gone: Target} | {settled: true}   // Phase 2 implements settled only

type Observation = {
  header: Header,                 // core::render::Header: device {id, model, os, screen, orientation}, app {id, name},
                                  // keyboard (rect or null), settle {settled, ms}
  text: string,                   // rendered snapshot (with header) or diff lines, exactly what the CLI prints
  kind: "full" | "diff",
  changed: boolean, settled: boolean, settle_ms: number,
  still_changing?: Rect,          // the union of recent changed boxes when settled is false
  loading: boolean,
  elements?: Element[],           // only with `elements: true` (CLI --json)
  capture?: string                // the device's own tree JSON with a `density` key, only with `capture: true`
}
type ActionResult = {
  ok: true,
  summary: string,                // "tapped @e6 \"Display & touch\" at 206,488"
  via?: string,                   // "nav-button", "edge-swipe", "paste", ...
  verified?: boolean,             // type: field text now contains the input
  settled: boolean, settle_ms: number,
  changed: boolean,               // pixels or windows changed, or the diff is not empty
  observation: Observation       // kind "diff" with empty text when nothing changed
}
```

### Errors

JSON-RPC `error` with `code` from the table, `message`, and `data: {code_name, hint, ...variant fields}` (architecture 4.7).

| Code | Name |
|---|---|
| -32001 | no_device |
| -32002 | ambiguous_device |
| -32003 | device_gone |
| -32004 | device_busy |
| -32005 | needs_user |
| -32006 | stale_ref |
| -32007 | not_found |
| -32008 | ambiguous |
| -32009 | off_screen |
| -32010 | version_mismatch |
| -32011 | unsupported |
| -32012 | not_settled |
| -32013 | timeout |
| -32014 | automation_conflict |
| -32015 | transport |

## 2. Daemon ↔ Android agent

Two adb `localabstract:` streams to the agent's socket. The host writes one byte first on each stream to name it: `C` for control, `V` for video. The agent rejects a third stream.

### Control stream

Frames: `u32 big-endian length` + UTF-8 JSON. Requests carry `id`; replies echo it; events have no `id`.

```json
→ {"id":1,"op":"hello","version":"0.1.0","token":"<64 hex chars written to the agent's stdin at launch>"}
← {"id":1,"ok":true,"api":37,"model":"Pixel 9","density":2.625,"display":{"w":1080,"h":2400,"rotation":0}}
```

`hello` with the right token must be the first frame on the control stream. On the video stream, the frame after the `V` byte is `u32 length + {"token": "..."}`. A wrong or missing token closes the stream with no reply.

| op | Request fields (device px) | Reply |
|---|---|---|
| `hello` | `version` | device facts |
| `tree` | none | `{windows:[{id, type, package, layer, bounds}], nodes:[RawNode]}`; every node is listed with its `v`/`V` flags, and full-width status and navigation bar windows (at most a tenth of the screen high, on the top or bottom edge) are left out |
| `gesture` | `pointers:[{id, path:[{x, y, t_ms}]}]` | `{}` after the final UP is delivered |
| `key` | `keycode, meta?, action?: "down"|"up"|"press"` | `{}` |
| `text` | `mode` (`paste`, `set`, `keys`), `text`, `sensitive?` (marks the clip `IS_SENSITIVE`) | `{}`; `set` acts on the focused input field; `keys` fails with `unmappable character U+XXXX at N` before typing anything |
| `clipboard_get` / `clipboard_set` | `text` (set), `sensitive?` | `{text}` (get; null when empty) |
| `apps` | `all?` | `{apps:[{package, label, system}]}` |
| `launch` | `package` | `{}` |
| `open_url` | `url` | `{package}` that handled it |
| `notifications` | none | `{}` (expands the shade) |
| `state` | none | `{screen_on, locked, secure_keyguard, ime_visible, foreground}` |
| `sync_frame` | none | `{}`; next video frame is a keyframe |
| `wake` | none | `{}` |

`RawNode` (flat; tree order is preorder). Empty and false fields are omitted to keep the payload small.
```json
{"p":-1,"w":12,"id":"com.android.settings:id/switch_widget","cls":"android.widget.Switch",
 "text":"","desc":"Dark theme","hint":"","sd":"On","b":[900,812,1040,960],
 "f":"vVcCeH", "a":["tap","long_tap","scroll_down"],
 "range":{"min":0,"max":100,"cur":64,"type":"int"},
 "coll":{"rows":1000,"cols":1},"item":{"row":17,"col":0},
 "n":"1234"}
```

| Key | Source (Android API) | Meaning |
|---|---|---|
| `p` | | parent index, -1 for a window root |
| `w` | `getWindowId` | window id |
| `id` | `getViewIdResourceName` | resource id |
| `cls` | `getClassName` | class name |
| `text` | `getText` | text; masked bullets for password fields |
| `desc` | `getContentDescription` | content description |
| `hint` | `getHintText` (API 26) | placeholder |
| `sd` | `getStateDescription` (API 30) | state text, for example "On", "64 percent" |
| `b` | `getBoundsInScreen` | `[l,t,r,b]` in px |
| `f` | flags: `v` visible, `V` visible-to-user, `c` checkable, `C` checked, `e` enabled, `E` editable, `F` focused, `s` selected, `P` password, `S` scrollable, `H` heading (`isHeading`, API 28), `X` expanded, `x` collapsed (from `ACTION_EXPAND`/`ACTION_COLLAPSE` availability), `h` showing hint text (`isShowingHintText`) | |
| `a` | `getActionList` | `tap`, `long_tap`, `set_text`, `scroll_fwd`, `scroll_back`, `scroll_up`, `scroll_down`, `scroll_left`, `scroll_right` (the directional ones from API 23 `AccessibilityAction.ACTION_SCROLL_*`) |
| `range` | `getRangeInfo` | sliders and progress |
| `coll`, `item` | `getCollectionInfo`, `getCollectionItemInfo` | list size and row position ("Row 17 of 1000" without scrolling) |
| `n` | hidden `getSourceNodeId` via reflection | source id for ref continuity (architecture 4.3); omitted when unavailable |

`Element.value` comes from `sd`, else `range`, else editable `text`. `scrollable↓` and friends come from the directional scroll actions; when only `scroll_fwd`/`scroll_back` exist, the axis comes from the collection shape (`cols > rows` means horizontal).

Events:
```json
{"ev":"window_changed","w":12,"pkg":"com.android.settings","t":123456789}
{"ev":"display_changed","w":1080,"h":2400,"rotation":1}
{"ev":"state","locked":true}
```

### Video stream

After the `V` byte, the agent sends one codec header:
`u32 codec fourcc ("h264") | u32 width | u32 height`.

Then frames:
`u64 pts_us | u8 flags (bit0 config, bit1 keyframe) | u32 length | bytes (Annex-B)`.

On display change the agent sends a new codec header preceded by a frame with `pts_us = u64::MAX` and length 0.

## 3. Daemon ↔ PhoneUseRunner

One TCP stream to the runner's port: 27753 on a physical device (reached through the idevice provider), or on a simulator the port the daemon assigned through the `PHONE_USE_PORT` environment variable (27753, 27754, … per running simulator; simulators share the host's loopback). Same `u32` length + JSON framing as the Android control stream.

| op | Request | Reply |
|---|---|---|
| `hello` | `version` | `{ios, model, screen:{w,h,scale}}` (points) |
| `snapshot` | `max_depth, attributes?` | `{app:{bundle_id, pid}, nodes:[RawNode]}` with bounds in points |
| `gesture` | `pointers:[{path:[{x,y,t_ms}]}]` (points) | `{}` |
| `screenshot` | `quality (0..100), format: "jpeg"|"png"` | `{data_b64, w, h}` |
| `type` | `text` | `{}` |
| `button` | `name: "home"|"volume_up"|"volume_down"|"lock"` | `{}` |
| `activate` | `bundle_id` | `{}` |
| `open_url` | `url` | `{}` (`XCUIDevice.shared.system.open`) |

iOS `RawNode` uses the same keys as Android with `cls` set to the `XCUIElementType` name and `n` omitted. `text` is `label`, `desc` is unused, `sd` is `value`, `hint` is `placeholderValue`, `id` is `identifier`; flags come from `enabled`, `selected`, `hasFocus`, and the element type (switch, secure text field), plus `v` and `V` when the element's frame intersects the screen. `core::prune` drops nodes without `V`, so the runner must set it.

## 4. Fixture format

Real trees captured for golden tests are stored as the device wire JSON (`tree` or `snapshot` reply) under `crates/core/tests/fixtures/<platform>/<name>.json`, with a sibling `.png` screenshot for humans. Capture with `phone-use snapshot --save-fixture <name>` (Phase 2 task 2.7; the files go to `crates/core/tests/fixtures/<platform>/` or `--fixture-dir`). The JSON is the device's own `tree` reply without `id` and `ok`, plus a top-level `density` key; core tests read the density from the file. Older `real_*` fixtures have no `density` key (2.625).
