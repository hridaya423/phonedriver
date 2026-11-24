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

Progress: a client that sends `"progress": true` in `hello` may receive notifications (no `id`) while a request is running, `{"jsonrpc":"2.0","method":"progress","params":{"message":"starting the iOS runner on iPhone 17 (the first run builds it; logs are in /Users/you/.phonedriver/runner)"}}`, one for each setup step that runs longer than a second (building the iOS runner, pushing the Android agent). A client that does not ask never receives one. The CLI asks only when its stderr is a terminal, and prints each message on stderr; `phonedriver mcp` never asks.

### Methods

| Method | Params | Result |
|---|---|---|
| `hello` | `version, client, progress?` | `version, pid` |
| `mask` | `text` | `{text}`: the text with every secret the daemon remembers masked (D105), for text a client made from the screen, such as what a screenshot comparison read. Traced by its length |
| `shutdown` | none | `{pid}` (the daemon exits, ending the iOS runners it started) |
| `devices` | none | `[DeviceSummary]` |
| `hidden` | `ids?` | `{count, ids}`: the devices the device list hides (D101); `ids` adds ids of devices that are not ready, judged by id alone. For the CLI and `doctor`; not an MCP tool |
| `boot` | `name?` | without `name`: `{off: [DeviceSummary]}`, the devices that are off and can be started, the ones to prefer first. With `name` (a model, any case, or an id): `{id, model, os}` once it is listed, with `already: true` when a connected device has the name; `Refused` when nothing off has it (decisions D77, D79). For an Android virtual device the id in the list is `avd:<name>` and the id in the reply is the serial it booted as, `android:emulator-<port>` |
| `release` | `device?` | `{}` |
| `told` | `line` | `{same}` (records `line` as the device line the session was last shown; `same` is true when it already was. The CLI drops a repeated line from its text output, never from `--json`; no device, no lease) |
| `snapshot` | `device?, diff?, interactive?, within?, pixels?, max_lines?, no_settle?, elements?, capture?, timeout_ms?` | `Observation` |
| `screenshot` | `device?, inline?, marks?` | `{path, width, height, scale, data_b64?, marks?}` (`path` is on the daemon's machine; with `inline: true` the PNG bytes also come back base64-encoded in `data_b64`, for a client that does not share the daemon's file system) |
| `find` | `device?, text, role?` | `[ElementLine]` (rendered lines such as `@e12 cell "Row 1"`; the elements whose label or hint contains `text`, in any case; `role` is a snake_case role name) |
| `act` | `device?, action: Action, approval?: "once"\|"session"\|"autonomous", ask?: ("send"\|"delete"\|"install")[], settle?: bool, timeout_ms?, expect_change_ms?, secret_field_only?: bool, on?: {app, over}` | `ActionResult`; with `secret_field_only` a secret `type` goes only into a password or one-time code field and is refused before typing otherwise (a replay sets it, D105); with `on` (a replayed step's saved screen) the action is refused before any touch on another app or when a dialog is in front and was not, or the other way round (D110) |
| `wait` | `device?, condition: WaitCondition, timeout_ms?, expect_change_ms?` | `ActionResult`; a wait for an element returns when it shows (or is gone), with `settled: false` and `settle_ms: 0`, since nothing was settled; its summary says how many tree reads it took. A text is found in a label, a hint, or, when neither has it, what a field holds (never a password field's) |
| `check` | `device?, target: Target` (a text or a ref, not a point), `equals?: string`, `as?: string` | `ActionResult` whose summary is the reading: `"Total" = "€24.99"`, `"Dark theme" = "off"`, or `"Order placed" is shown` when the element is there and nothing reads as its value; `as` adds ` · <as>` so one reply can carry several named readings. It touches nothing (decision D113). With `equals`, a value that differs (after folding case, width and runs of spaces) is `not_as_expected` (-32027). A password field is `refused`. Saved by `journal` and replayed like a `wait`. The trace keeps `checked` or the error's name, never the value, unless traces are full |
| `batch` | `device?, steps: Step[], approval?: "autonomous", ask?: ("send"\|"delete"\|"install")[], timeout_ms?, expect_change_ms?` | `BatchResult` |
| `apps` | `device?` | `[App]` (launchable apps: `{id, name, system}`) |
| `app` | `device?, action: "install"|"stop"|"clear"|"uninstall"|"grant"|"revoke"|"notify", app?, path?, permissions?: string[], payload?: string` | `{ok, summary}` (`path` is for install, on the daemon's machine; `app` is a name or id; `permissions` is for grant and revoke, with the names `launch`'s `grant` takes; `payload` is for notify: the push notification as JSON text with an `aps` object, delivered to the app on an iOS Simulator and `unsupported` everywhere else (decision D100), `refused` when it is not valid; all but install and stop are refused with `unsupported` on a physical device for an app phonedriver did not install) |
| `shell` | `device?, command, settle? (default true), timeout_ms?, expect_change_ms?, time_limit_s? (default 30, at most 600), grep?, head? (default 40, 0 = all), force?, secret?` | `{exit, output, omitted, timed_out, ms, action?}`; `action` is an `act` result (`observation` is the diff) and is absent when the command only reads or `settle` is false; `unsupported` for a physical iPhone or iPad, and for a destructive command on a physical device without `force` (decision D21) |
| `logs` | `device?, app?` (name or id; default: the app in front), `grep?`, `head?` (the last lines to keep; default 40, 0 = all), `since_s?` (default 60) | `{app, output, omitted, crash}`: the app's log lines of the last `since_s` seconds, oldest first; `omitted` counts earlier lines left out; `crash` is its last crash in that time (the exception and the frame that threw it) or null. `unsupported` on a device whose app logs cannot be read (everything but Android; decision D58) |
| `notifications` | `device?, app?, limit?` (default 20) | `{lines: string[], more}`: the notifications the device is showing, newest first, one line each as `<app> · "<title>" "<text>" · <age>` with ` · ongoing` for one the user cannot dismiss and ` (ai-instruction)` when a text reads as one; `more` is how many older ones `limit` left out. `app` is a name, an id, or a package with nothing to launch. Under an app scope only the scope's apps are listed. Android reads `dumpsys notification --noredact`; other devices answer `unsupported` (decision D117) |
| `network` | `device?` | `{networks: [{id, transports, default, interface, dns, owner, uids, underlying}], private_dns, always_on_vpn, lockdown, missing}`: the networks `dumpsys connectivity` lists as connected, which is the system default, each VPN's owning package and the user id ranges it carries, DNS for each, and the always-on VPN, its lockdown and private DNS mode from the secure and global settings. `missing` names every field the platform did not give; the others are absent or null, never guessed. Read-only. `unsupported` (-32011) off Android (decision D99) |
| `page` | `device?, grep?` (words, `a|b`), `all?: boolean` | `{title, url, text, lines, omitted_lines, omitted_chars}`: the web page the app in front shows, read through its DevTools socket (decision D71). `text` is the page's `innerText` with blank lines dropped, cut at a line end near 8,000 characters unless `all`; with `grep`, the lines holding one of the words and the two lines after each. `refused` (-32025) when the app in front has no debuggable page; `unsupported` on iOS |
| `record` | `device?, action: "start"\|"stop", path?` (absolute, on the daemon's machine, for a start; default: a new `.mp4` in the device's folder under `tmp`) | start: `{path}`. stop: `{path, ms, frames, width, height, bytes, steps, steps_path?, early?}`: the finished MP4 (H.264 as the device encoded it), its length, and how many actions ran during it; `steps_path` is `<path>.steps.jsonl`, one `{"t_ms", "summary", "at"?}` line per successful `act` with the time on the video's clock, the reply's summary line (never typed text) and where a tap landed, present when `steps` is above 0; `early` says why the recording ended before the stop (the screen changed size or turned, the stream ended). `refused` for a second start, a stop with none running, and a stop after the connection to the device was replaced (the message names the file, which is whole). On the iOS Simulator the file is the Simulator's own recording (decision D66): `early` is set when its recorder had stopped by itself, and a turned screen does not end it. `unsupported` on an iPhone. Decision D59. |
| `build` | `device?, app` | `{build}`: the installed app's version and build number, `"4.37.1 (437001)"`, or `null` where the device cannot tell (a physical iPhone). `replay` compares it with the `build` of the file's launch steps (decision D109). |
| `journal` | `device?, keep?, drop?` | `{steps: Step[], over}`: what the session has done since the last `journal` call, in order, and the session's list starts again empty (decision D60). A step is `{method, params, summary?, changed?, by_position?, expect_exit?, build?, on?}` for a call that succeeded (`act`, `wait`, `shell`, `app`, and a `setting` that wrote or opened), with `params` as that method takes them, or `{manual}` for one that cannot be kept (a shell command given with `secret`). A typed secret is an `act` step whose `type` action has `secret: true` and no `text`; `save` adds `secret_env`, the name of the environment variable a replay reads the text from (decision D98). In an action's or a wait's target, a ref is replaced by `{id, text?, role?}` when the element has an id of its own that no other element on the screen the agent had seen shares (decision D67), by `{text, role}` of its label or hint when those single it out, and by its centre as a point otherwise (`by_position: true`, also set for a target that was a point and for `gesture`). `build` is on a `launch` step, the launched app's build when the steps were read (D109). `on` is `{app, over}` on an action step: the app in front and whether a dialog, sheet or alert was over it on the screen the agent last saw; an `act` call given `on` is refused before any touch when the screen is another app or `over` differs (D110). `changed` is the action's `changed`; `expect_exit` is a `shell` command's exit status when it was not 0 (a step without it expects 0, and `null` accepts any). A shell step does not keep `force`. `over` is true when more than 500 steps were done; only the first 500 are kept. With `keep: true` the steps are read and stay in the list; with `drop: n` the first `n` of them are removed (`{}`), so a client that stores the steps in a file empties the list only after the write worked. The client writes the steps to a file (`save`) and runs them again (`replay`): one call per step, stopping at the first error, at a step saved with `changed: true` that now answers `changed: false`, and at a `shell` step that timed out or exited with another status than its `expect_exit`. |
| `setting` | `device?`, and one of `find: string` (words; `a|b` runs a search per part), `key: string` (with `value?: string` to write), `values: {key: value}` (several writes, in key order), `open: string` (a screen key); `force?`, `settle?`, `timeout_ms?`, `expect_change_ms?` | `find`: `{lines: string[], more}` (each line is `key  [accepted values]  purpose`, or `screen <key>  description`; at most 12 settings and 3 screens, or 4 and 1 per search when there are several). `key`: `{key, value, hint?}`. `key` with `value`: `{key, value, action}`, where `value` is what the device reports after the write and `action` is an `act` result. `values`: `{values: {key: value}, action}`; every key is checked before anything is written, and a write the device declines stops the rest and names what was already set. `open`: an `act` result. `unsupported` on a device without settings functions (decision D57); `refused` when the key is unknown, read-only or the device declines the write; `unsupported` on a physical device for a setting the device marks as needing its user's confirmation, without `force` |
| `clipboard` | `device?, set?: string` | `{text?: string}` (`{}` after a set; `text` is null when the clipboard is empty) |
| `health` | `device?` | `{id, model, os, frames_ms}` (what `doctor` needs: connects, reads the tree once, then waits up to 1 s for the next frame; `frames_ms` is null when none came or the device has no frame stream) |

`hello` takes `client` (`cli`, `bench` or `mcp`); the daemon ends the leases of every session an `mcp` connection used when it closes. `devices` entries carry `lease_idle_s` beside `leased_by`, and `lease_live: true` while that lease will not lapse for another session (an MCP session's, or a command line's within 30 s of its last call); `DeviceBusy` carries `idle_s` and `expires_s`.

`device` is omitted when exactly one device is connected or a default is set. Every device method also takes `session` (string, default `"default"`); refs, diff baselines and leases belong to the session, not to the socket connection (architecture section 5).

`open` and `launch` are `act` kinds, not methods. A key a method does not take, at the top of its params or inside an action, is `invalid_params` (-32602) naming it; nothing runs. `share?: boolean` is accepted by every device method (architecture 5, leases). So is `display?: "main" | "hidden"` (default `main`; decision D32): a call in the other mode than the live connection reconnects the device in the requested one. A `device` that ends in `+<name>` (1 to 8 lowercase letters or digits; `+a` alone when one device is connected) is a lane of that device (decision D73): it is always `hidden`, has its own lease and refs, and `devices` lists it as an entry of its own while it is open. `devices` entries and the `health` reply carry `hidden_display` (the virtual display's id) while the device works on one. Typed text and secrets: a `type` action's text is masked (`•••`) in `observation.text` when `secret` is true, and the trace records only its length.

```ts
type Target = {ref: string} | {point: {x: number, y: number}} | {id: string, text?: string, role?: Role} | {text: string, role?: Role}
// {id}: the one element with that resource name (Android) or accessibility identifier (iOS). When no element
// or more than one has it, `text` and `role` find the element as a text target does. Saved steps use it.
type Action =
  | {kind: "tap", target: Target, count?: number, long?: number}   // long: hold ms, at most 10000; count taps are 80 ms apart, at most 10
  | {kind: "type", text: string, into?: Target, mode?: "paste"|"set"|"keys", secret?: boolean, submit?: boolean, append?: boolean}
    // the CLI reads secret text from stdin or PHONEDRIVER_SECRET and sends it here over the user-only socket;
    // from then on the daemon replaces that text with ••• in every reply, error and trace line (decision D70)
  | {kind: "clear", target: Target}
  | {kind: "swipe", direction: Dir, from?: Target, distance?: number, fast?: boolean}
  | {kind: "drag", from: Target, to: Target}                       // press and hold, move, hold, lift
  | {kind: "pinch", direction: "in"|"out", at?: Target, distance?: number}  // two fingers; `out` zooms in
  | {kind: "scroll", direction: Dir, in?: Target, until?: Target, max?: number, collect?: number}   // collect: scroll on and return the list's rows, up to that many, in `rows` (D116); not with `until`
  | {kind: "gesture", fingers: Array<Array<{x: number, y: number, t_ms: number}>>}
  | {kind: "press", key: Key}
  | {kind: "open", url: string, app?: string}         // a link or deep link, in `app` when given
  | {kind: "launch", app: string, grant?: string[], restart?: boolean}  // restart: stop the app first and open it on its first screen (Android: its task is cleared too, D112); grant: runtime permissions to grant first (Android `pm grant`:
    // `notifications`, `camera`, `location` or a full name; Simulator `simctl privacy` services). On a physical device only
    // for an app phonedriver installed (`Unsupported`, -32011, otherwise); a physical iPhone and `notifications` on the
    // Simulator are `Unsupported`.
type WaitCondition = {appears: Target} | {gone: Target} | {settled: true}
  | {vpn: "up"|"down"}                                   // Android: a VPN network is connected, or none is (decision D99)
  | {log: {pattern: string, app?: string}}               // a log line written after the wait began; `pattern` is `A|B` words, any case
  | {notification: {pattern: string, app?: string}}      // Android: a notification posted since the session's last action (at most 120 s back; since the wait began when it has done none) whose title or text holds one of the words; an empty pattern with `app` is any from that app. The reply's summary is `a notification from <app> is up` and `rows` holds its line; a `timeout` says how many are up (D117)
// appears and gone reply with a full Observation and `summary: "\"Row 640\" appeared after 3 tree reads"`; a text
// target is a substring match, so `gone:Loading` is not gone while "Hide loading" is on screen. They fail `timeout`
// (-32013) naming the condition. `vpn` and `log` reply with the summary and the header of the screen, no screen text;
// `summary` is `a VPN is up (<package>)`, `no VPN is up` or `log line: <the line>` (masked like any reply, D70), and a
// `timeout` names the last state seen (`last seen: cellular, wifi (default)`) or how many log lines arrived. `unsupported`
// (-32011) where the device has no network state (everything but Android) or no log to follow (a physical iPhone or iPad).

type Step = Action | {kind: "wait", condition: WaitCondition, timeout_ms?: number} | {kind: "check", target: Target, equals?: string, as?: string}   // a check step reads as the `check` method does and stops the batch when `equals` does not hold (D113); a wait's own timeout stands in for the batch's (D111); the CLI refuses `secret` text in a batch, the MCP server passes it (D85); the daemon masks it either way (D70)
type BatchResult = {
  ok: boolean,
  steps: Array<{summary: string, rows?: string[], verified?: boolean, unverified?: {shows, hint}}>,   // the steps that ran and succeeded; `rows` as in ActionResult
  failed?: {step: number, error: {code, message, data}}, // 1-based; `error` is as in section "Errors"
  settled: boolean, settle_ms: number, changed: boolean,
  observation: Observation   // diff from what the session last observed before step 1 to after the last step that ran
}
// The steps run in order in one lease, each acted on and settled like `act`. The first error ends the batch with
// `ok: false` (a result, not a JSON-RPC error, so the steps that ran and the screen are not lost). An empty list or a
// step the daemon cannot parse is -32602 and nothing runs.

type Observation = {
  header: Header,                 // core::render::Header: device {id, model, os, screen, orientation}, app {id, name},
                                  // keyboard (rect or null), settle {settled, ms}, alerts [{package, title}]
  text: string,                   // rendered snapshot (with header) or diff lines, exactly what the CLI prints
  kind: "full" | "diff",
  changed: boolean, settled: boolean, settle_ms: number,
  still_changing?: Rect,          // the union of recent changed boxes when settled is false
  empty?: true,                   // nothing to act on: a hollow tree and no text read from the pixels (decision D62)
  loading: boolean,
  elements?: Element[],           // only with `elements: true` (CLI --json)
  capture?: string                // the device's own tree JSON with a `density` key, only with `capture: true`
}
type ActionResult = {
  ok: true,
  summary: string,                // "tapped @e6 \"Display & touch\""; a tap at a point: "tapped at 206,488"
  at?: {x, y},                    // where a tap landed, in points
  via?: string,                   // "nav-button", "edge-swipe", "paste", ...
  verified?: boolean,             // type: the field holds the input (replace: exactly; append or no target: ends with it)
  unverified?: {shows: string, hint: string},  // with verified: false: what the field shows and a hint for that cause
  settled: boolean, settle_ms: number,
  changed: boolean,               // pixels or windows changed, or the diff is not empty
  unchanged_repeats?: number,     // from 3: this same action has settled with no change that many times in a row
  crashed?: {app, what},          // the app that was in front crashed during the action; `what` is the exception and the frame that threw it
  woke?: true,                    // the screen was off and the action woke it first
  unlocked?: true,                // a swipe-only lock screen was swiped away first
  rows?: string[],                // a scroll with `collect`: the list's rows in its order, each a snapshot line without its ref, then the lines of what it holds; a wait for a notification: its line. Never in the trace or the saved steps
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
| -32006 | stale_ref (`data.screen`, when the session had seen a screen: what changed since the one it was last shown, or the whole screen when there is nothing to compare with, and `""` when nothing changed; the session's last screen is then this one, so its refs can be used at once. Decision D114) |
| -32007 | not_found (`data.screen` as for `stale_ref`, on a failed `act` or `check`) |
| -32008 | ambiguous |
| -32009 | off_screen (`data.behind_keyboard` is true when the keyboard covers the element; `data.covered_by` names a floating toolbar that covers it) |
| -32010 | version_mismatch |
| -32039 | scope_mismatch (`hello` asks for an app scope the daemon was not started with; run `phonedriver daemon stop`; decision D102) |
| -32011 | unsupported |
| -32012 | not_settled |
| -32013 | timeout |
| -32014 | automation_conflict |
| -32021 | device_driven (another phonedriver daemon owns the device's agent; `data.id`, `data.pid`, `data.host`; decision D34) |
| -32022 | disabled (a tap by ref or text names an element the app reports as disabled; nothing is sent; `data.ref`; a tap at `x,y` is never refused) |
| -32015 | transport |
| -32017 | no_app (`data.similar` lists installed apps that share a word with the name; when none does and at most 40 apps are installed, `data.installed` lists every installed app by name) |
| -32018 | unknown_key (`press` got a key it does not know; `data.valid` lists the keys) |
| -32020 | secret_field (a `type` with plain text into a password field, or a text field whose label, hint or id names a one-time code, without `secret`; `data.ref`, `data.kind` `password` or `code`; decision D31) |
| -32024 | point_off_screen (an `x,y` target outside the screen; `data.x`, `data.y`, and the screen's `data.w` and `data.h` in points; nothing is sent to the device) |
| -32025 | refused (the device answered and did not do what was asked: a setting it would not write, an unknown or read-only setting key, a `pm grant` that changed nothing; the message is the reason; nothing changed) |
| -32027 | not_as_expected (a `check` with `equals` read another value; `data: {what, shows?, expected}`; nothing was touched, and in a batch no later step ran) |
| -32026 | crashed (the daemon failed partway through the call, a bug: the call may have acted, and the device connects again on the next call; `daemon.log` has the failure) |
| -32023 | not_focused (a `type` without `into` when the screen shows no focused text field and no keyboard; nothing is typed; `data.fields` lists up to five text fields on screen) |
| -32019 | needs_approval (a tap or `type --submit` that spends money, deletes an account, or is in a category named in `ask`, without an `approval`; `data.label`, `data.app`, `data.category` (`payment`, `account`, `send`, `delete`, `install`), `data.details`; decisions D27 and D31) |
| -32016 | not_reached (`data.end` is `ended`, `stuck` or `limit`; `data.scrolls`, `data.last`) |

## 2. Daemon ↔ Android agent

Two adb `localabstract:` streams to the agent's socket. The host writes one byte first on each stream to name it: `C` for control, `V` for video. The agent rejects a third stream.

### Control stream

Frames: `u32 big-endian length` + UTF-8 JSON. Requests carry `id`; replies echo it; events have no `id`.

```json
→ {"id":1,"op":"hello","version":"0.1.0","token":"<64 hex chars written to the agent's stdin at launch>"}
← {"id":1,"ok":true,"api":37,"model":"Pixel 9","density":2.625,"display":{"w":1080,"h":2400,"rotation":0}}
(a `hello` that carries "display":"hidden", and for a lane "lane":"<name>", opens a virtual display for this connection and adds "hidden_display":<id> to the reply; `display` is then that display's size. The video connection's first frame carries the same two fields. Refusals: `hidden display: <why>`, `lane in use: <name>`)
```

`hello` with the right token must be the first frame on the control stream. On the video stream, the frame after the `V` byte is `u32 length + {"token": "..."}`. A wrong or missing token closes the stream with no reply.

| op | Request fields (device px) | Reply |
|---|---|---|
| `hello` | `version` | device facts |
| `tree` | none | `{windows:[{id, type, package, layer, bounds}], nodes:[RawNode], bar?}`; every node is listed with its `v`/`V` flags, and full-width status and navigation bar windows (at most a tenth of the screen high, on the top or bottom edge) are left out; `bar` is the bottom edge in pixels of the status bar left out, when there was one |
| `gesture` | `pointers:[{id, path:[{x, y, t_ms}]}]` | `{}` after the final UP is delivered |
| `key` | `keycode, meta?, action?: "down"|"up"|"press"` | `{}` |
| `text` | `mode` (`paste`, `set`, `keys`), `text`, `sensitive?` (marks the clip `IS_SENSITIVE`, also the paste that `set` falls back to) | `{}`; `set` acts on the focused input field; `keys` fails with `unmappable character U+XXXX at N` before typing anything |
| `shell` | `command`, `timeout_ms?` (default 30000) | `{exit, stdout, stderr, timed_out}`; runs `sh -c` as the shell user on a thread of its own, each stream capped at 1 MiB, killed at the timeout with exit 124 |
| `clipboard_get` / `clipboard_set` | `text` (set), `sensitive?` | `{text}` (get; null when empty) |
| `apps` | `all?` | `{apps:[{package, label, system}]}` |
| `launch` | `package`, `fresh?` | `{}`; `fresh` starts the app's launcher activity with its task cleared (`FLAG_ACTIVITY_CLEAR_TASK`), which `launch` with `restart` sends (decision D112) |
| `open_url` | `url` | `{package}` that handled it |
| `notifications` | none | `{}` (expands the shade) |
| `state` | none | `{screen_on, locked, secure_keyguard, ime_visible, foreground}` |
| `power` | none | `{screen_on, locked, secure_keyguard, focused}` (`focused`: an accessibility window has input focus); cheap, read before every action |
| `sync_frame` | none | `{}`; next video frame is a keyframe |
| `video_barrier` | `barrier` (u64) | `wait_ms`; the agent writes a barrier packet with that number into the video stream (below) once its encoder has given out what the screen showed when the request came. `wait_ms` is how much longer than usual that may take: 0 while the encoder keeps up. The number is not sent as `id`, which is the request's own (decision D74) |
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
| `rd` | `getExtras()` key `AccessibilityNodeInfo.roleDescription` | Compose role description (`Switch`, `Checkbox`, `Radio button`, `Tab`, `Button`, `Image`, `Dropdown list`); omitted when absent |
| `n` | hidden `getSourceNodeId` via reflection | source id for ref continuity (architecture 4.3); omitted when unavailable |

`Element.value` comes from `sd`, else `range`, else editable `text`. `scrollable↓` and friends come from the directional scroll actions; when only `scroll_fwd`/`scroll_back` exist, the axis comes from the collection shape (`cols > rows` means horizontal).

Events:
```json
{"ev":"window_changed","w":12,"pkg":"com.android.settings","t":123456789}
{"ev":"display_changed","w":1080,"h":2400,"rotation":1,"density":2.625}
{"ev":"state","locked":true}
```

### Video stream

After the `V` byte, the agent sends one codec header:
`u32 codec fourcc ("h264") | u32 width | u32 height`.

Then frames:
`u64 pts_us | u8 flags (bit0 config, bit1 keyframe) | u32 length | bytes (Annex-B)`.

On display change the agent sends a new codec header preceded by a frame with `pts_us = u64::MAX` and length 0.

A barrier packet answers `video_barrier`: `pts_us = 0`, `flags` bit2 set, length 8, and the request's `barrier` number as a big-endian `u64`. The host reads packets in order, so when it has decoded the barrier every frame the agent wrote before it has been shown. The agent writes it after the encoder output on hand when its last frame came out within 100 ms of its presentation time. When that frame was later than that, the encoder is behind, and the barrier is held until a frame presented after the request has been written, or until the encoder has given nothing for 100 ms plus twice that lateness.

## 3. Daemon ↔ PhonedriverRunner

One TCP stream to the runner's port: 27753 on a physical device (reached through the idevice provider), or on a simulator the port the daemon assigned through the `PHONEDRIVER_PORT` environment variable (the first free port from 27753 up to 27784, one per running simulator; simulators share the host's loopback). Same `u32` big-endian length + JSON framing as the Android control stream. On a simulator the runner listens on `127.0.0.1` only; on a device on all interfaces. Off a Mac on Wi-Fi the daemon opens the port inside the remote pairing's tunnel (decision D106).

The runner is started with a token, 64 hex characters in its environment (`PHONEDRIVER_TOKEN`). The first frame on a connection must be:

```
→ {"id":1,"op":"hello","version":"0.2.0","nonce":"<32 hex chars, fresh>","proof":"<hex HMAC-SHA256 under the token of host:<nonce>>"}
← {"id":1,"ok":true,...,"proof":"<hex HMAC-SHA256 under the token of runner:<nonce>>"}
```

The runner compares the proof in constant time and refuses a nonce it has accepted before. Any other first frame, a wrong proof, or a first frame over 4 KB gets `{"ok":false,"error":{"code":"unauthorized",...}}` and the connection is closed. The daemon uses the runner only when the reply's `proof` checks. The token never crosses the connection. One connection at a time: a new connection that proves the token closes the previous one; one that does not leaves it alone. Requests run one at a time.

Every request carries `id` and `op`. A reply echoes `id` and adds `ok: true` plus the op's fields, or `ok: false` and `error: {code, message}` with `code` one of `bad_request` (malformed frame, unknown op, missing or ill-typed field), `unsupported` (the op cannot work here), `failed` (XCTest refused or failed; `message` is XCTest's first line).

| op | Request | Reply |
|---|---|---|
| `hello` | `version`; on the first frame also `nonce` and `proof` | `{version, ios, model, screen:{w,h,scale}, udid?, build?}` and `proof` on the first frame (points; `version` is `0.2.0`; `model` is the machine id, for example `iPhone18,3`; on a simulator `udid` is the simulator's, from `SIMULATOR_UDID`; `build` echoes the `PHONEDRIVER_BUILD` the daemon launched it with, the hash of the runner sources) |
| `snapshot` | `max_depth` (default 30) | `{app:{bundle_id, pid}, nodes:[RawNode], alerts?, bar?}` with bounds in points; `bar` is the bottom edge of SpringBoard's status bar, asked for once for each width of the app |
| `gesture` | `pointers:[{path:[{x,y,t_ms}]}]` (points, 1 to 5 pointers, `t_ms` non-decreasing) | `{}` after the last lift is delivered |
| `banner` | `x`, `y` (points) | `{covered}`: whether a notification banner covers the point, the test the runner's own gestures wait on; the Simulator tier asks it before a touch through simio that starts in the top 200 points (decision D93) |
| `screenshot` | `quality (0..100, default 80), format: "jpeg"|"png"|"luma"` (default `jpeg`), `scale?` (output pixels per point, above 0 and at most 4; 0 or absent means native pixels; required for `luma`) | `{data_b64, w, h}` (`w`, `h` in pixels); `luma` is one byte of gray per pixel, rows top to bottom, no header |
| `type` | `text`, `speed?` (characters per second, default 60) | `{}` |
| `key` | `key` (an `XCUIKeyboardKey` string or a character), `modifiers?` (`"command"`, `"shift"`, `"option"`, `"control"`) | `{}` after the key is delivered |
| `button` | `name: "home"|"volume_up"|"volume_down"|"lock"|"landscape"|"portrait"` | `{}`; volume is `unsupported` on the Simulator; `landscape` and `portrait` set `XCUIDevice.orientation` (decision D78) |
| `activate` | `bundle_id` | `{}` |
| `open_url` | `url` | `{}` (`XCUIDevice.shared.system.openURL`; unlike `simctl openurl`, raises no "Open in …?" confirmation) |

`snapshot` covers the first of `XCAXClient_iOS activeApplications` (the foreground app). Its attribute list is fixed (label, value, identifier, frame, element type, traits, placeholder value, native focus); a request-chosen list was planned and dropped because nothing needs it.

`type` sends key events to whatever has keyboard focus, through a synthesized text-input event with redaction, so the text never reaches XCTest's activity log, xcodebuild's output or the `.xcresult` bundle. With no focused field the keys are dropped and the reply is still `{}`; the daemon checks the field afterwards (`verified` in `ActionResult`). At `speed` 5000 the runner entered 102 mixed characters (accents, CJK, Arabic, emoji with skin tone) exactly in 267 ms on the iOS 26.5 Simulator; the Simulator tier uses that speed for `paste` mode (architecture 7.3). A newline goes through `type` (`"\n"`): `key` with `"\r"` or `"\n"` did not insert one in a multi-line field.

On the Simulator the daemon also runs simio (decision D93), a helper that speaks one JSON object per line over its stdin and stdout and answers `{"ok":true}` once it holds the device, or `{"ok":false,"error":...}`. Started as `simio <udid> <developer dir> touch`, it takes `{"op":"touch","path":[{"x","y","t_ms"}]}` (one finger, portrait points) and answers when the lift is delivered. Started with `frames <pixels per point>`, it prints `{"w","h","y":<base64 luma>}` each time the screen buffer is written, at most every 16 ms, and `{"locked":bool}` first and on each change, and exits when its parent or the Simulator stops.

iOS `RawNode` uses the same keys as Android with `cls` set to the `XCUIElementType` name and `n` and `w` omitted. `text` is `label`, `desc` is unused, `sd` is `value`, `hint` is `placeholderValue`, `id` is `identifier`, `b` is `[l,t,r,b]` in points rounded to 1/100. Flags: `v` and `V` when the frame intersects the application's frame (`core::prune` drops nodes without `V`); `e` enabled; `s` selected; `F` focused; `c` for switch, toggle and checkbox, plus `C` when the value is `1`; `E` for text field, secure text field, search field and text view; `P` for secure text field; `S` for scroll view, table, collection view and web view. `a` is `["tap"]` for button, cell, link, switch, toggle, checkbox, radio button, tab, menu item, key, icon and the text-entry types; no other actions are emitted.

## 4. Fixture format

Real trees captured for golden tests are stored as the device wire JSON (`tree` or `snapshot` reply) under `crates/core/tests/fixtures/<platform>/<name>.json`, with a sibling `.png` screenshot for humans. Capture with `phonedriver snapshot --save-fixture <name>` (Phase 2 task 2.7; the files go to `crates/core/tests/fixtures/<platform>/` or `--fixture-dir`). The JSON is the device's own `tree` reply without `id` and `ok`, plus a top-level `density` key; core tests read the density from the file. Older `real_*` fixtures have no `density` key (2.625).
