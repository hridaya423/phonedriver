# Competitor gap analysis (2026-11-06)

Question: what do the leading phone-control harnesses give an agent that phone-use does not, or give it better? Scope: Android and iOS action layers, observation formats, and harness design. phone-use baseline is `README.md`, `docs/architecture.md` (sections 4, 10, 14), `docs/audits/ax-audit.md`, `docs/bench/m1.md`, and `docs/research/competitor-teardown.md` (phone-harness, already torn down in detail).

## How to read this

- Read on 2026-11-06 from each project's GitHub README, docs page, or (for mobile-mcp) `src/server.ts` and `src/format-elements.ts`. The page fetches went through a summarizing fetch tool, so quoted phrases are as that tool reported them. Re-open the link before quoting any of them outside this repo.
- "?" in a table means the page I read does not say. It does not mean "no".
- Nothing here was benchmarked. No device was touched and no model was called. Latency and token figures for competitors are therefore absent; section 4 proposes how to get them.
- Not read at source level: Open-AutoGLM's action executor, AskUI's agent, Appium's driver internals, the PhoneHarness paper beyond its arXiv HTML, Maestro's hierarchy schema. AskUI and the AndroidWorld top-score claims are vendor statements.
- Stars are as displayed on 2026-11-06.

## 1. Capability matrix

### 1a. Platforms, observation, waiting

| Tool | Platforms | Real devices | Observation | Element refs | Wait / settle | Install | License | Stars |
|---|---|---|---|---|---|---|---|---|
| **phone-use** | Android, iOS | Android (untested on hardware), iPhone/iPad (tested, 454-534 ms tap) | Text a11y snapshot, settled diffs, screenshot, OCR fallback | `@eN`, re-located at tap time | Event + frame settle on every action; `wait` verb | Build from source: Rust, Android SDK, d8, JDK 17 | AGPL-3.0 | unpublished |
| [mobile-mcp](https://github.com/mobile-next/mobile-mcp) | Android, iOS, cloud | Yes both | A11y list (one line per element, with `at=x,y size=WxH`), screenshot (JPEG q75, max 1024), `raw` hierarchy | `@eN`, "valid as long as the screen does not change" | None in `server.ts` (no sleeps after actions) | `npx @mobilenext/mobile-mcp@latest install`; needs Node 20, adb, Xcode CLT | Apache-2.0 | 10.4k |
| [mobilerun](https://github.com/droidrun/mobilerun) (was droidrun) + [Portal](https://github.com/droidrun/droidrun-portal) | Android, iOS, cloud | Yes both | A11y tree with overlay indices, screenshot, `--vision` modes | Overlay indices | ? | Python 3.11-3.13, adb, Portal APK with accessibility service enabled; iOS via `mobilerun-ios` or iOS Portal app | MIT | 10.0k (Portal 408) |
| [minitap mobile-use](https://github.com/minitap-ai/mobile-use) | Android, iOS Simulator | Android yes; iOS device WDA per paper, README says unsupported | Hierarchy + screenshot every cycle ("UI hierarchies as the primary source while invoking vision selectively") | ? | LangGraph "verified execution" (agent re-checks) | uv, adb, Docker for quick launch, LLM keys; iOS needs idb | Apache-2.0 | 3.4k |
| [Appium MCP](https://github.com/appium/appium-mcp) | Android (UiAutomator2), iOS (XCUITest) | Yes both | Page-source XML (filtered summary capped at 100 nodes), screenshot PNG, optional vision `ai-element:` | WebDriver element ids; locators by a11y id, id, predicate, XPath | None general; `scroll_to_element` stops when page source stops changing | `npx -y appium-mcp@latest`; Node 22, JDK, Android SDK/Xcode | Apache-2.0 | 519 |
| [Maestro MCP](https://docs.maestro.dev/getting-started/maestro-mcp) | Android emu, iOS Simulator, web | Android yes; "Physical iOS devices are not yet supported" | `inspect_screen` compact JSON hierarchy, `take_screenshot` | Selectors in YAML flows | "automatic waiting" inside flows | `curl ... | bash`, Java 17 | Apache-2.0 | 16.3k |
| [agent-device](https://github.com/callstackincubator/agent-device) | iOS, Android, Harmony, TV, web, macOS, Linux | Yes (iOS via XCTest) | `snapshot -i` a11y tree, `@e2 [button] "Add"`; screenshot, video, logs, network, perf | `@eN`, latest output only | `--settle` waits, returns `+ - =` diff | `npm i -g agent-device`, Node 22 | MIT | 5.3k |
| [mobilecli](https://github.com/mobile-next/mobilecli) | iOS, Android | Yes both (iOS needs signed on-device agent) | `snapshot`: indented text tree with `@e5` refs or JSON with rects | `@eN` | ? | `npm i -g mobilecli`; per-user daemon, exits after 30 min idle | FSL-1.1-Apache-2.0 | 315 |
| [Android-MCP](https://github.com/CursorTouch/Android-MCP) (clean-room "beta" page read) | Android | Yes | `uiautomator dump` element list with coordinates; optional screenshot with labels drawn on it | Coordinates, `ClickBySelector` | `Wait`, `WaitForElement` tools | Python 3.13, uv, adb only | MIT | 920 |
| [scrcpy-mcp](https://github.com/JuanCF/scrcpy-mcp) | Android | Yes | Screenshot image, `ui_dump` XML | Coordinates from `ui_find_element` | ? | Node 24, adb, optional scrcpy, ffmpeg | MIT | 131 |
| [ios-simulator-mcp](https://github.com/joshuayoes/ios-simulator-mcp) | iOS Simulator | No | `ui_describe_all` (format unstated), `ui_view` compressed screenshot | None; `ui_find_element` by label/id, coordinates | None | Node 20, Xcode, Facebook idb | MIT | 2.2k |
| [Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM) | Android (adb), HarmonyOS (hdc), iOS via WDA guide | Yes | Screenshot only ("截图 → 视觉模型理解界面 → 输出点击坐标 → ADB 执行操作") | None, model emits coordinates | `Wait` action | Python 3.10, adb, ADB Keyboard IME; model needs ~24 GB VRAM and ~20 GB download if self-hosted | Apache-2.0 | 26.5k |
| [AskUI](https://github.com/askui/python-sdk) | Android (adb), iOS Simulator only via idb ("experimental") | Android yes | Vision (not described on the page I read) | ? | ? | pip + separate AgentOS | commercial-leaning, not read | ? |
| [phone-harness](https://pypi.org/project/phone-harness/) 0.3.3 | iPhone via Mac mirroring, Android via adb, cloud | Yes | OCR text, `uiautomator dump` | None | Sleeps, MD5 `wait_stable` | `pip install` | MIT | n/a (see `competitor-teardown.md`) |
| PhoneHarness ([arXiv 2606.14832](https://arxiv.org/abs/2606.14832)) | Android (GUI proxy translates to adb) | ? | Not stated in the abstract or HTML I read; GUI proxy does "UI-tree retrieval" | ? | ? | Not released as far as I found | ? | n/a |

### 1b. Action set

Y = documented. blank = not documented on what I read. phone-use column is from `docs/architecture.md` 10.1 and `SKILL.md`; "CLI" means the verb exists on the command line but not in the 11 MCP tools.

| Action | phone-use | mobile-mcp | mobilerun | Appium MCP | Maestro MCP | agent-device | mobilecli | Android-MCP | scrcpy-mcp | ios-sim-mcp | AutoGLM |
|---|---|---|---|---|---|---|---|---|---|---|---|
| tap by ref / by text | Y both | ref or xy | node/text | locators | selectors via flows | ref | ref | xy, selector | xy | xy, find | xy |
| long press | Y (`--long`) | Y | ? | Y | flow | Y | Y | Y | Y | duration | Y |
| double tap | Y (`--count 2`) | Y | ? | Y | flow | ? | ? |  |  |  | Y |
| swipe | Y | Y | Y | Y | flow | Y | ? | Y | Y | Y | Y |
| scroll (to target) | Y, `--until` | via swipe | `scroll_until` | `scroll_to_element` | flow | Y | ? |  | scroll | via swipe | via swipe |
| drag and drop | via `gesture` CLI |  | ? | Y | flow | ? | ? | Y | Y |  |  |
| pinch / multi-touch | `gesture` CLI | ? | ? | Y (`pinch_zoom`, `perform_actions`) | ? | ? | ? |  |  |  |  |
| type text | Y (paste/set/keys, Unicode) | `type_keys` into focused field | `type`, base64 via Portal IME | `set_value` | flow | `fill` | Y | Y (ASCII; IME for rest) | `input_text` | Y (paste slow) | Y via ADB Keyboard |
| keys (home/back/enter) | Y | Y | Y | Y | flow | ? | Y | Y | Y | ? | Y |
| hide keyboard | Y (`press hide-keyboard`) |  |  | Y (`appium_mobile_keyboard`) | flow | ? |  |  |  |  |  |
| launch app | Y | Y | Y | Y | flow | Y | Y | Y (shell) | Y | Y | Y |
| terminate / install / uninstall / clear data |  | Y | `start_app`/`stop_app` | Y | flow | ? | Y |  | Y (no clear) | install, terminate |  |
| foreground app | in header | Y | ? | Y | ? | ? | ? |  | Y | ? | ? |
| deep link / URL | Y (`open`) | http(s); others gated by `MOBILEMCP_ALLOW_UNSAFE_URLS` | `open_deep_link` | `deep_link` | flow | ? | ? |  |  | Y | ? |
| arbitrary intent / extras | VIEW only |  |  | via mobile commands | ? | ? | ? |  |  |  |  |
| shell | none (by design D6) |  |  | `mobile:` shell where enabled | ? | ? | ? |  | Y `shell_exec` |  |  |
| clipboard | CLI only | Y | Y (needs Portal keyboard for get) | Y | ? | ? | Y |  | Y |  |  |
| file push / pull / list | none (known gap) |  | Y (`files/*`, Android 11+) |  |  | ? | Y |  | Y |  |  |
| notifications | `press notifications` (open shade) |  |  | open_notifications | ? | ? |  | Y (shade) | Y (shade, qs) |  |  |
| permission dialogs | seen in tree; no helper |  | `grant_permission` | `appium_mobile_permissions`, `appium_alert` | flow | alerts | ? |  |  |  |  |
| orientation | iOS runner only | get/set | ? | Y | ? | ? | ? |  | rotate |  |  |
| location override | none (known gap) | Y (`set_location`) | ? | Y (`appium_geolocation`) | ? | ? | Y |  |  |  |  |
| screen recording | none | Y | ? | Y | ? | Y | ? |  | Y |  Y |  |
| device logs / crashes | none | Y (`get_device_logs`, `list_crashes`) | ? | ? | ? | Y | Y | |  |  |  |
| batch many actions in one call | none | Y (`mobile_batch_commands`) | ? | `perform_actions` | `run` flows | ? | ? |  |  |  |  |
| webview inspect | none | ? | cloud `execute_script` | Y (`appium_context`) | web only | ? | Y |  |  |  |  |
| cloud devices | none (SSH remote) | Y | Y | ? | Y | Y (BrowserStack, AWS DF, ...) | Y |  |  |  |  |
| multi-device | Y, per-device actors + leases | list + `device` arg | ? | sessions | ? | Y, per-worktree claims | Y | `--device` |  | `udid` | one per run |
| secrets kept out of logs | Y (`--secret`, D9) | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? |

### 1c. Fixed context cost of the tool list

Every session pays this before the first call. Counts are tools, from the pages above.

| Server | Tools | Note |
|---|---|---|
| phone-use | 11 | `ax-audit.md`: about 1.8k tokens for the schemas |
| mobile-mcp | about 34 | count from the README list in section 1a of its page |
| scrcpy-mcp | 44 | stated |
| appium-mcp | about 25 | several are multiplexed (`appium_gesture` has 8 actions) |
| Maestro MCP | 9 | 3 are cloud-only |

## 2. Ranked gaps

Ranking weighs how often an agent would hit the gap in ordinary tasks, how directly it costs calls or success (`ax-audit.md`: "The number of calls is the cost"), and how cheaply phone-use can close it. Effort: S under a day, M a few days, L a week or more. Items 1 to 10 are what I would do first.

### 1. Install friction: nothing to `npx` (M, needs the user: publishing is a one-way door)

Evidence: phone-use README Install: "phone-use is not published. Build it from source with Rust 1.99 or later", plus Android SDK build-tools 37, `d8`, JDK 17 (or `PHONE_USE_AGENT_JAR`). mobile-mcp: "`npx @mobilenext/mobile-mcp@latest install` detects installed MCP clients and configures the ones you choose" (README). agent-device: `npm install -g agent-device@latest`. mobile-harness (droidrun): "Claude Code plugin: `/plugin marketplace add droidrun/mobile-harness`".
Why it matters: adoption and any head-to-head by a third party start here. The 153x speed claim is moot if the first hour is a toolchain install.
Sketch: release CI that builds the jar once and embeds it (`build.rs` already honors `PHONE_USE_AGENT_JAR`), static binaries per OS/arch, an `install` subcommand that writes the MCP entry for Claude Code, Cursor, Codex and Gemini CLI (mobile-mcp's detection logic is a model), a Claude Code plugin and a Skills-CLI package that ships `SKILL.md`. Needs the user's decision on publishing and signing (iOS runner needs a team; see Known gaps).

### 2. No batching: one action per call, and agents already parallelize badly (M)

Evidence: mobile-mcp `src/server.ts` `mobile_batch_commands`: steps run sequentially, `stopOnError` defaults true, `listElementsAtEnd` appends a snapshot. phone-use `ax-audit.md` finding 5: "7 of 9 agents issued parallel tool calls ... which a stateful device does not tolerate"; testbed-row "sent 8 scrolls in one turn". Each call re-reads 6k to 28k tokens of context.
Why it matters: the audit says calls are the cost, not snapshot tokens. A batch that returns only the final settled diff is the largest lever on tokens and wall time.
Sketch: MCP tool `steps` taking an ordered list of verb objects (same schemas as `tap`, `type`, `scroll`, `press`, `wait`), run serially inside one lease, stop on first error, return per-step one-liners plus the diff from the first to the last settled state. Keep refs valid across steps by letting later steps use text targets only (refs are per snapshot). Also makes finding 5 non-fatal: the server can serialize parallel calls per device instead of failing them.

### 3. MCP surface is narrower than the CLI: no clipboard, gesture, find, clear, release (S)

Evidence: README: "The server has eleven tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`." `SKILL.md`: "On the command line there are also `find`, `clear`, `clipboard`, `open`, `gesture` and `release`." Competitors expose clipboard (mobile-mcp `mobile_clipboard`, Appium `appium_mobile_clipboard`, scrcpy-mcp `clipboard_get/set`), pinch and drag (Appium `appium_gesture` `pinch_zoom`, `appium_drag_and_drop`; Android-MCP `Drag`). `ax-audit.md` finding 2: the agent "had no tool to release".
Sketch: add `clipboard`, `gesture` (the existing JSON), `clear` and `release` to `crates/cli/src/mcp.rs`. Resist adding one MCP tool per CLI verb: fold `clear` into `type mode=set text=""`, `find` into `snapshot --within`. Net count stays under 15 tools. Each tool schema costs tokens every session (section 1c), so the budget is real.

### 4. No app lifecycle beyond launch: install, terminate, clear data, uninstall (S for Android and Simulator, M for iOS devices)

Evidence: mobile-mcp: `mobile_install_app` (".apk, .ipa, .app, or .zip"), `mobile_terminate_app`, `mobile_uninstall_app`, `mobile_clear_app_data` (Android and Simulator). scrcpy-mcp: `app_stop`, `app_install`, `app_uninstall`. Appium: `appium_app_lifecycle` with `activate, terminate, install, uninstall, list, is_installed, query_state, background, clear, deep_link`. Open-AutoGLM and phone-use have `launch` only. `ax-audit.md`: "A `launch` reopens an app where it was left (Settings may open inside a sub-page)": agents need terminate or clear to get a known state.
Why it matters: reproducible starting state is the first thing every benchmark and every repeated agent task asks for. Installing the app under test is the first thing a developer's agent asks for.
Sketch: verbs `app stop <id>`, `app clear <id>`, `app install <path>`, `app uninstall <id>` on the `Device` port. Android: `am force-stop`, `pm clear`, `pm install` through the existing adb sync push (the adb client already does sync). Simulator: `simctl terminate/install/uninstall`. iOS device: idevice `app_service` terminate; install needs installation_proxy (M). Fold into the MCP as one `app` tool with an `action` enum rather than four tools.

### 5. Permission and system-alert handling is by-hand (S to M)

Evidence: Appium MCP has `appium_alert` and `appium_mobile_permissions`; mobilerun's mobile-harness lists `grant_permission`; agent-device lists "handling alerts". phone-use sees dialogs in the tree (UiAutomation `getWindows`, architecture 6.2) but gives the agent no distinct handle and no way to pre-grant.
Why it matters: a first launch of almost any app puts a runtime permission or notification sheet in the way. The agent spends 2 to 3 calls per sheet. Pre-granting avoids them.
Sketch: (a) `launch --grant <perm,...>` or `app grant`: Android `pm grant` (S); Simulator `simctl privacy` (S); iOS device: none, say `Unsupported`. (b) a header line `alert "Allow X to send notifications?" [Allow] [Don't Allow]` when a `Dialog` or `Alert` role is frontmost, so the agent sees it without searching; the role enum already has `Dialog` and `Alert`. (c) optional `launch --auto-allow` is a policy decision for the user, so default off.

### 6. No escape hatch to the OS: shell, generic intent, file push/pull (M, gated)

Evidence: PhoneHarness abstract: "reliable phone automation depends on action-surface routing and verifiable execution, not just visual GUI control"; "75.0% pass rate ... 12.9 points above the strongest non-PhoneHarness settings"; its design is "deterministic-first routing" across GUI, CLI (shell/Python/ADB) and host tools (arXiv 2606.14832). scrcpy-mcp: `shell_exec`, `file_push`, `file_pull`, `file_list`. mobilerun Portal: `files/*` (Android 11+), `sms/gateway`. Our `docs/architecture.md` 14: "No file push or pull ... Add only when an agent task needs it." and D6 rejects code execution from agent scripts.
Caveat: the PhoneHarness benchmark is theirs, tasks mix GUI with email/search/file tools, and I did not read how much of the gain comes from the CLI surface. Treat 12.9 points as upper-bound motivation, not an expected gain.
Sketch: keep D6 (no scripts) but add three typed operations that are not "run arbitrary code": `intent` (action, data URI, package, component, string extras; Android `am start`; replaces ad hoc `open`), `push`/`pull` (adb sync, already implemented; iOS Simulator via `simctl` container paths), and `shell` behind an explicit `--allow-shell` daemon flag that is off by default and recorded in the trace. Photo-upload tasks and "share this file" tasks need `push`. Ask the user before adding `shell`: it widens what a prompt-injected agent can do.

### 7. Screenshots carry no labels for vision-only or hybrid models (S)

Evidence: Android-MCP: "With `use_vision`, it also returns a screenshot with the labels drawn on it." Portal overlay: "The overlay highlights clickable, checkable, editable, scrollable, and focusable elements" with indices. AGI-0 reports 97.4% on AndroidWorld "operat[ing] entirely from screenshots" (vendor claim). Open-AutoGLM is screenshot plus coordinates. Minitap feeds both hierarchy and screenshot to its Cortex every cycle. phone-use `snapshot --pixels` is OCR boxes and `screenshot` is plain.
Why it matters: when the tree is empty (games, canvas, some WebViews) or when the model is vision-tuned, a screenshot with `@eN` boxes lets the model say "tap @e12" instead of guessing x,y. We already own the ref table and the pixels.
Sketch: `screenshot --marks` draws a rectangle and `@eN` per interactive element (and per OCR box when the tree is empty) with `image` or `tiny-skia`, same coordinates as the snapshot. Refs in the picture are the refs in the next snapshot. Measure whether it helps with the three model-free checks in section 4 plus one eval run the user approves.

### 8. No device logs or crash reports (M)

Evidence: mobile-mcp: `mobile_get_device_logs` ("logcat on Android, unified log on iOS"), `mobile_list_crashes`, `mobile_get_crash`. agent-device diagnostics: "screenshots, video, logs, traces, network data, performance samples, crash details". mobilecli: "device logs", "crash reports".
Why it matters: "the app did nothing" and "the app crashed" are indistinguishable from a settled diff alone. PhoneHarness's thesis is verifying the side effect; logs are one verification channel. Developer-facing agents need them.
Sketch: `logs [--since 30s] [--grep re] [--package id] [--max-lines N]`. Android: `logcat -d` over the existing adb shell transport, filter on the host (S). iOS Simulator: `simctl spawn log show`. iOS device: idevice syslog relay (M). Cap output hard (agents choke on 10k lines) and always say how many lines were dropped. `crashes`: Android `dumpsys dropbox --print data_app_crash` (S); iOS crash logs via crashreportcopymobile (M).

### 9. No screen recording or evidence bundle (M)

Evidence: mobile-mcp `mobile_start_screen_recording` / `mobile_stop_screen_recording`; Appium `appium_screen_recording`; scrcpy-mcp `screen_record_start/stop`; ios-simulator-mcp `record_video`; agent-device "video ... traces ... save evidence for review".
phone-use has the harder half already: a decoded H.264 frame stream on Android and iOS 27 CoreDevice, plus JSONL traces (decision D8-ish trace lines, `docs/decisions.md`). What is missing is a way for a person to see what the agent did.
Sketch: `record start|stop [-o file.mp4]` that muxes the stream's NAL units into MP4 without re-encoding (Android and CoreDevice streams are already H.264; polled iOS tiers fall back to screenshot frames at low fps or `Unsupported`). A `trace export` that joins trace lines to frame timestamps for a reviewable timeline. Redact `--secret` text frames: an on-screen secure field already renders as dots, but record `FLAG_SECURE` surfaces as black, which is the correct behavior to keep.

### 10. Device state control: location, orientation, folding (S on emulators and Simulator, hard on real Android)

Evidence: mobile-mcp `mobile_set_location`, `mobile_set_orientation`, `mobile_fold_device`; Appium `appium_geolocation`, `appium_orientation`; mobilecli "location override". `docs/architecture.md` 14: "No GPS location simulation." Orientation set exists only on the iOS runner and Simulator.
Sketch: Android emulator `adb emu geo fix` (S); Simulator `simctl location set` (S); real Android needs a mock-location provider app, so `Unsupported` with that reason; real iOS: idevice `simulatelocation` service needs the DDI, M. Orientation on Android: `settings put system accelerometer_rotation 0` then `user_rotation` (S). Skip folding unless a foldable emulator is in the test fleet.

### 11. No `assert` verb (S)

Evidence: Maestro `assertVisible` (README); agent-device lists "asserting state"; AGI-0: "After every action, it verifies whether the intended change occurred"; Minitap attributes a share of its AndroidWorld result to verified execution (the paper says +7 points in the abstract and +15 in section 5.2, so I would not quote a number). phone-use has `wait`, which can express most assertions but returns a snapshot on success and a `Timeout` on failure, with no exit code that scripts can tell from "device busy".
Sketch: `assert "text" | @ref [--state off|on|checked] [--count N]` returns `ok` or a distinct `AssertionFailed { expected, found }` and exit code 3 on the CLI. Cheap to build on the existing resolver and wait loop. Its value is mostly for CI and for agents that self-verify.

### 12. No replay or export of a successful run (M)

Evidence: agent-device: "saving runs as `.ad` scripts, with optional strict Maestro YAML export". Appium MCP: `appium_generate_tests`. Maestro MCP `run` executes YAML flows in one call.
Why it matters: an agent that found the path once should not pay model calls to repeat it. This also gives users regression tests from agent runs.
Sketch: `trace replay <file>` re-executes recorded verbs with text targets (never stale refs; the trace already stores target text for refs, verify), stops on the first diff mismatch, and `trace export --maestro` emits `tapOn`/`inputText`/`launchApp`. Secrets stay out: the trace stores `{"secret": true}` for them (D-trace decision), so replay prompts for them via `PHONE_USE_SECRET`.

### 13. WebView content is thin (M to L)

Evidence: `ax-audit.md` safari-read: the agent "gave the page text and said the tree labels a group 'Example Domain', no heading node". Appium: `appium_context` list/switch; mobilecli "webview inspection". Android WebView exposes a partial tree through a11y; Chrome and WebViews with `setWebContentsDebuggingEnabled` expose DevTools over `localabstract:webview_devtools_remote_<pid>`.
Sketch: for Android, when a `WebView` node is in the snapshot, optionally pull headings and links via DevTools `Accessibility.getFullAXTree` over the adb `localabstract` forward (we already speak adb transport). iOS Safari needs the Web Inspector service (idevice has `webinspector`, unverified API). Do only if web-heavy tasks show up in agent evals.

### 14. No cloud or farm backend (L, probably out of scope)

Evidence: mobile-mcp `mobile_allocate_remote_device`; mobilerun Cloud ("REST API, Python and TypeScript SDKs, or MCP"); Maestro `run_on_cloud`; agent-device providers "BrowserStack, AWS Device Farm, TestMu AI, and Limrun".
phone-use's SSH-forwarded daemon socket covers "my own device elsewhere". A farm adapter means a new `Device` impl per vendor, an account per vendor, and for hosted ones data leaving the machine. D9 (no telemetry) does not forbid it, but the project voice does. Recommend skipping unless a user asks; if asked, start with plain adb-over-network endpoints (Limrun, AWS DF expose them), which need no new code beyond `adb connect`.

### 15. No per-app knowledge (S per app)

Evidence: mobile-harness ships `apps/<platform>/<id>/CARD.md` per-app cards, `core/memory/GUIDE.md`, and `core/credentials`; PhoneHarness uses "progressive disclosure" skills with an index in the prompt. `ax-audit.md` shows agents wasting calls on "Search Settings (search returns nothing on the Simulator)" and "looked for Safari in the root list (it is under Apps)".
Sketch: `phone-use skill --app com.apple.Preferences` prints a 10-line card (where things live, known dead ends). Ship cards for Settings, Safari, Contacts, Chrome, Photos, Messages on both platforms. Cheap, and measurable with the agent eval at `bench/agent-eval/`. Lower rank because it is content, not mechanism, and cards go stale across OS versions.

### 16. Other platforms: HarmonyOS, TV, foldables, web (L each, skip)

Evidence: Open-AutoGLM supports HarmonyOS NEXT via hdc; agent-device lists Harmony, tvOS, Android TV, Fire TV, Vega OS, web, macOS, Linux; Maestro MCP covers Chromium. phone-use's brief is phones and tablets. Not a gap for the stated goal; listed so nobody rediscovers it as one.

## 3. Where phone-use is clearly ahead

Only claims I can support from our measurements (`docs/bench/m1.md`) or from what a competitor's own page says it does.

- **Latency, measured against the one competitor we benchmarked.** phone-use vs phone-harness 0.3.3 on an Android 17 emulator, median of 50: screenshot 15.2 ms vs 390.1 ms, Settings snapshot 67.7 ms vs 2,426 ms, 1,000-row list snapshot 15.5 ms vs 2,370 ms, tap 36.5 ms vs 110.7 ms (`m1.md`). No other competitor has a published latency figure that I found (mobilecli: "only the first command pays the discovery cost" is the only performance statement on its page; Android-MCP: "The page makes no performance or latency statements"). So "fastest" is unproven against anyone but phone-harness. Section 4 fixes that.
- **Act and observe in one call.** Every phone-use action returns the settled diff (README: tap returns `~ @e10 switch "Dark theme" off → on`). mobile-mcp's `server.ts` has no sleeps after actions and no diff, so an agent calls `list_elements` after each action; its batch tool needs `listElementsAtEnd` to do the same. agent-device has the closest equivalent (`--settle` returns a `+ - =` diff), so this is parity with one competitor and ahead of the rest. How agent-device decides "settled" I did not read.
- **Event-driven settle.** phone-use waits on accessibility events and frame change, with no timer polling (architecture 4.6). Appium's page documents no general wait model; mobile-mcp has none in `server.ts`; phone-harness uses sleeps.
- **Nothing installed on the phone, no accessibility service to enable.** Android agent is launched by `app_process` as the shell user (D2). Droidrun needs the Portal APK and an enabled accessibility service ("This installs the Mobilerun Portal app, enables its accessibility service"); Android text input in mobilerun and AutoGLM goes through an installed IME (Portal keyboard, ADB Keyboard); Appium installs UiAutomator2 server APKs (not read in detail). mobilecli also runs a shell-launched `DeviceServer` holding the one UiAutomation connection, the same design, so this is parity there.
- **Exact Unicode typing.** Android-MCP: `adb shell input text` "only covers printable ASCII and some accented Latin letters"; AutoGLM requires ADB Keyboard; ios-simulator-mcp paste mode is "slow (about 2 s, up to about 11 s)". phone-use typed mixed-script text (ASCII, accents, CJK, Arabic, emoji) in 288 ms for 100 characters (`m1.md`).
- **Secrets.** `--secret` text never reaches logs, traces or replies (D9, architecture 13). No competitor page I read mentions it. phone-harness ships user content to PostHog by default (`competitor-teardown.md`).
- **Refs that survive a shift.** Refs are re-located at tap time. mobile-mcp: refs are valid "as long as the screen does not change"; agent-device: "Refs are valid only in the latest output."
- **iOS real devices without WDA or signing on iOS 27.** CoreDevice HID plus the display stream (architecture 7.2) needs nothing installed. mobile-mcp, mobilerun-ios, mobilecli and agent-device all use an on-device XCTest or WDA agent for physical iPhones (signed with a team). Caveat: the iOS 17 to 26 tier needs our signed runner like they do, and iPhone taps cost 454 to 534 ms.
- **Concurrency safety.** Per-device actors and leases (architecture 5) against phone-harness's process-global `ANDROID_SERIAL` (`competitor-teardown.md` item 5). agent-device has a similar claim mechanism ("device claims prevent agents from taking over each other's simulators"); that is parity.
- **Compact snapshot lines.** `@e1 button "Search Settings"` against mobile-mcp's `@e1 Button text="OK" at=10,20 size=100x40` with every element emitted (`format-elements.ts`: "No elements are filtered out"). Measured size comparison is a section 4 item; the line shapes above already differ by about 2.5x in characters for a one-label button.

Not ahead, despite appearances: **doctor**. mobile-mcp has `install`, agent-device has `doctor`, phone-harness has `doctor`. Ours is good but it is table stakes.

## 4. What can be measured without paid model calls

All of this runs against a booted emulator or Simulator with the phone-use testbed apps, using scripted MCP/CLI calls and no LLM. The emulators are busy today, so none of it was run. Reuse `bench/adb-baseline/run_adb.py` (a baseline already exists there) and `crates/` `phone-use-bench`.

### 4.1 Tokens per observation (offline, deterministic)

For each tool run its observe call on the same four screens (Settings main, testbed List with 1,000 rows, Contacts New Contact with keyboard up, Safari example.com) and save the raw text:

| Tool | Call |
|---|---|
| phone-use | `snapshot`, `snapshot --interactive` |
| mobile-mcp | `mobile_list_elements_on_screen` format `text` and `json` |
| agent-device | `snapshot -i` |
| mobilecli | `snapshot` |
| Appium MCP | `appium_get_page_source` (and the filtered summary) |
| Maestro MCP | `inspect_screen` |
| Android-MCP | `Snapshot` without vision |
| scrcpy-mcp | `ui_dump` |
| ios-simulator-mcp | `ui_describe_all` |

Count with `tiktoken` (`o200k_base` and `cl100k_base`) and report chars, lines and tokens. Do not call Anthropic's `count_tokens`: it is an API call and the project rule is no paid calls without asking. State that the numbers are proxy tokens, comparable across tools but not Claude's own. Our own figures for reference (`ax-audit.md`, chars/4): 183 to 822 tokens per screen. Also count `tools/list` bytes and tokens per server (section 1c) since that is paid once per session.
Screenshot cost is not countable offline without a model; report pixel dimensions and bytes instead (mobile-mcp: JPEG q75 max 1024).

### 4.2 Loop latency (wall clock, 50 runs after 5 warm-ups, same as `m1.md`)

Drive each server over stdio JSON-RPC `tools/call` (the way an agent client does), timing from request written to reply read, with the server already running. Also report cold start (process spawn to first reply) separately, because per-call processes are how `phone-harness -c` behaves.

| Operation | Why |
|---|---|
| observe (tree), screenshot | Baselines |
| tap at a point | Raw input path |
| act then know the new state | phone-use: one `tap`. Others: `tap` + observe. This is the loop an agent runs |
| type 100 characters (ASCII, then mixed-script) | Input path and Unicode correctness |
| scroll to a row 640 down a 1,000-row list | The ax-audit worst case; tool call count as well as time |
| launch app cold | Includes any settle |
| 1,000-row snapshot | Large-tree behavior (phone-harness took 2.4 s) |

Run on one emulator, one tool at a time, in randomized order, with the machine otherwise idle. On Android, UiAutomation is single-owner: mobilecli and mobile-mcp's Android path and phone-use cannot hold it simultaneously, so reset the device between tools (`adb shell am force-stop`; kill the previous server). Record versions and commits.

### 4.3 Correctness checks proven by the testbed event log (not by the tool's reply)

The testbed apps log every touch and text event (AGENTS.md: "Device behavior is proven against the testbed apps' event logs"). For each tool run a fixed script and check the log:

- tap lands on the intended row after the list shifted by one row between observe and act (stale ref test)
- Unicode text arrives byte-for-byte
- act during an animation (spinner) does not hang or time out
- keyboard up: field below the keyboard reachable
- two clients at once on the same device: the second one fails cleanly or queues, and neither corrupts the other
- secret text absent from the tool's logs and its stdout

Score pass/fail per tool. This is the model-free proxy for "does the harness lose actions", and it tests exactly the failure classes in `ax-audit.md`.

### 4.4 Calls and bytes for a scripted scenario

Write the same 20-step scenario (open Settings, search, toggle dark theme, return home, open the testbed, fill the form, scroll to Row 640, open it) once per tool as the minimal sequence of that tool's calls, using each tool's best idiom (batch where it exists). Count calls and total reply bytes. That is the floor an agent could reach with a perfect model. phone-use should win on calls (diff in the reply) and lose to mobile-mcp when batching is used until gap 2 lands, which is the point of measuring it.

### 4.5 Install friction

On a clean macOS user account and a clean Linux container: time and steps from nothing to a first snapshot, and the size of what was downloaded. Expected: `npx` tools in under two minutes; phone-use in as long as an Android SDK download. This is gap 1 as a number.

### 4.7 Token benchmark with `claude -p` (user-approved subscription runs): harnesses and tasks

Same model (`claude-sonnet-5-5`), same tasks, one run per harness per task, tokens summed from the stream-json `result` usage (input + output + cache creation + cache read, the M3 definition in `ax-audit.md`). The existing runner is `bench/agent-eval/run.sh` with `tasks.yaml` checks that read device state through adb, so the pass/fail check is harness-independent. Reuse it; add one `mcp.json` or Bash-only variant per harness. Use `--strict-mcp-config --setting-sources "" --no-session-persistence` and `--allowedTools` limited to that harness (plus `Bash` for the two Bash harnesses), so no harness gets the others' tools and Claude Code's own prompt is the fixed overhead.

Harnesses (4). All install locally and run against an Android emulator over the same adb server (`ADB_SERVER_SOCKET` for win-pc, as in `mcp.json`):

| # | Harness | Why it is in | Install | How Claude Code drives it |
|---|---|---|---|---|
| A | **phone-use** | Subject | `cargo build --release` (or `PHONE_USE_AGENT_JAR`), existing `bench/agent-eval/mcp.json` | MCP, `"command": "<bin>", "args": ["mcp"]` |
| B | **mobile-mcp** | Most adopted MCP (10.4k stars), a11y list plus screenshot, Android emulator supported | `npm` needs Node 20 and adb. Pin the version: `npx -y @mobilenext/mobile-mcp@<x.y.z>` (look up `npm view @mobilenext/mobile-mcp version` and record it) | MCP: `{"mcpServers":{"mobile-mcp":{"command":"npx","args":["-y","@mobilenext/mobile-mcp@<x.y.z>"],"env":{"ANDROID_SERIAL":"emulator-5554","ADB_SERVER_SOCKET":"tcp:127.0.0.1:5038"}}}}` (command from its README: `claude mcp add mobile-mcp -- npx -y @mobilenext/mobile-mcp@latest`). It selects the device through the `device` argument of each tool; `ANDROID_SERIAL` is a guess from adb behavior, check `mobile_list_available_devices` once before the run |
| C | **phone-harness 0.3.3** | The incumbent we already benchmark for speed (`bench/baseline`), and the only one whose action layer is Python run through Bash | `uv venv --python 3.12 ph && uv pip install --python ph phone-harness==0.3.3`; env `DO_NOT_TRACK=1 PHONE_HARNESS_TELEMETRY=0 PHONE_HARNESS_NO_UPDATE_CHECK=1 PHONE_HARNESS_PLATFORM=android ANDROID_SERIAL=emulator-5554` (same flags as `m1.md`) | No MCP. Allow `Bash`. The prompt appendix is its documented usage: pipe a script to `phone-harness -c '<python using screen(), tap_text(), type_text(), open_app()>'` (helper names from its PyPI page). Run `phone-harness doctor` first. Telemetry must be off or the run uploads task text |
| D | **raw adb via Bash** | The floor: what an agent does with no harness | `adb` only (already installed) | No MCP. Allow `Bash(adb:*)`. Appendix: one line naming the serial and that `adb shell uiautomator dump /dev/stdout`, `input tap|swipe|text|keyevent`, `am start`, `screencap -p` exist. Nothing else |

Considered and left out: **agent-device** is the nearest design (refs, settle diffs, MCP) and the most informative fourth comparison, but I did not read its Android path or confirm an emulator run, so it is a reserve harness (`npm i -g agent-device`, `agent-device doctor`, MCP via `agent-device mcp` per its README) to add if A to D leave the question open. Appium MCP is heavy to install (JDK, drivers) and Maestro MCP has no input verbs beyond flows, so neither is a like-for-like tool surface. Android-MCP needs Python 3.13; scrcpy-mcp needs Node 24; AutoGLM needs a 9B vision model, not Claude.

Tasks (6). Chosen so that each one separates a specific mechanism, using the existing `tasks.yaml` checks where they exist:

| Task | Prompt | Check | What it separates |
|---|---|---|---|
| T1 `dark-theme` | "Turn on the dark theme on the phone." | `cmd uimode night` contains `yes` (existing) | Baseline navigation cost; settled diff vs observe-after-act |
| T2 `alarm-0730` | "Set an alarm for 7:30 in the morning." | `next_alarm_formatted` contains `7:30 AM` (existing) | Pickers and dialogs; first-run sheets |
| T3 `testbed-web-form` | "In the phone-use testbed app, fill in the web form with the name Ada Lovelace, tick the agreement box and submit it." | testbed log events (existing) | Typing into WebView, refs vs coordinates |
| T4 `testbed-row-640` | "In the phone-use testbed app, find the list, scroll to the row called Row 640 and open it." | testbed log (existing) | Long-list scrolling; the largest call-count spread (`ax-audit.md`: 47 calls on iOS) |
| T5 `unicode-note` | "In the testbed Text tab, type `Zoë 你好 مرحبا 🙂` into the field." (new) | testbed log text equals the string | Unicode typing: raw adb and phone-harness cannot, so they should fail or detour via IME |
| T6 `read-answer` | "What Android version and build number does this phone report in Settings > About phone?" (new) | exact string match on the final answer vs `getprop` | Read-only task; separates tokens spent observing from tokens spent acting |

6 tasks x 4 harnesses = 24 runs. Budget from the audit's own numbers: median run about 100k tokens, so roughly 2.4M tokens in total, which is the cost of the slowest runs (T4 on raw adb and phone-harness will dominate; cap with `--max-turns 40` and report the cap as a failure). To stay lean: run T1, T4 and T5 first (cheapest to separate the harnesses); add T2, T3, T6 only if the first three do not.

Controls to write down in `docs/bench/`: the fixed prompt overhead per harness (measure one `devices`-only call, as in `ax-audit.md`: about 13k for phone-use), emulator reset between runs (existing `reset`), one run per cell (single samples show what can go wrong, not rates, per the audit), and the tool versions. Report calls, wall seconds, tokens split into cache-read vs fresh, and pass/fail from the independent check, not the agent's claim. Do not run harness D with the phone-use skill text and do not give A the `SKILL.md` unless every other harness gets an equivalent usage note; the audit's runs used none.

### 4.6 Not measurable here

Task success, steps per task and cost per task need a model: AndroidWorld (task 8.1 was dropped by the user, see the last commit), the M3 agent eval, or PhoneHarness Bench. Published competitor numbers are vendor-run and not comparable: droidrun's 63.0% (June 2025, partly manually verified per its blog) and 100% (mobile-harness page, "gpt-6-astra"), Minitap's 100% and "31 seconds on average versus 68 seconds" (its paper), AGI's 97.4% screenshot-only, PhoneHarness 75.0% on its own benchmark with AutoGLM-Phone at 37.1%. The aggregators disagree with each other. AndroidWorld is near saturation at the top, so it would not separate harnesses anyway; a harness comparison with a fixed model and the tasks that stress the action layer (permissions, files, logs, long lists) would.

## 5. Closest competitors, for the record

- **agent-device** is the nearest in shape: refs, text snapshots, `--settle` diffs, MCP, real devices on iOS and Android, device claims. It is MIT, Node, 5.3k stars, and it already ships record, logs, replay and cloud-provider backends. Its observation and settle internals are unread; reading them is the most valuable follow-up.
- **mobilecli / mobile-mcp** (same company): shell-launched UiAutomation holder, daemon with 30-minute idle exit, JSON-RPC, refs. mobilecli is FSL-1.1-Apache-2.0; mobile-mcp is Apache-2.0 and by far the best-known (10.4k stars). They lead on breadth (cloud, logs, recording, install, batch) and distribution; we lead on settle, latency and Unicode.
- **mobilerun/droidrun** has the largest published benchmark claims and the most opinionated agent loop; its action layer is a Python library (`mobilerun_core`) that an agent scripts, which is the model D6 rejected.

## 6. Order of work if the user wants the harness to be the best

1. Gaps 3 and 4 and 11 (all S): MCP parity, app lifecycle, assert. Closes the "obviously missing" list in a few days.
2. Gap 2 (batch): the largest effect on calls per task by the audit's own arithmetic. Do after the 4.4 scenario shows it.
3. Gaps 5, 7, 10 (S to M): permissions, marked screenshots, location and orientation.
4. Section 4 benchmark, run on the emulators when free, so every later claim has numbers.
5. Gap 1 once the user decides on publishing; gaps 6, 8, 9, 12 after, in that order. Ask before adding `shell`.
