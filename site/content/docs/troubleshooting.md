---
title: How to troubleshoot phonedriver
nav: Troubleshooting
meta: Start with phonedriver doctor, check daemon logs and traces, decode exit codes 0–5, and fix the most common Android and iOS snags.
description: Start with phonedriver doctor, inspect daemon logs and traces, decode exit codes, and fix common snags like UiAutomation conflicts and empty WebViews.
seo: phonedriver troubleshooting: doctor, exit codes, common errors
updated: 2026-11-21
---

## Start with doctor

```sh
phonedriver doctor
```

`doctor` checks `adb`, devices, Xcode, simulators, and socket permissions. Every line prints `ok`, `warn`, or `fail`—and every non-`ok` line prints the exact fix right under it. Physical phones and tablets, and devices another session is using, are listed but not probed: `phonedriver doctor --device <id>` probes one, and `--all-devices` probes every device nobody else holds.

## Where the logs live

- **`$PHONEDRIVER_HOME/daemon.log`** (default `~/.phonedriver/daemon.log`) — daemon startup messages and errors.
- **`$PHONEDRIVER_HOME/traces/<id>.jsonl`** — one JSON line per call with method, parameters, the reply's summary, timings, and settle duration. Typed text logs length only, and so does the text of a text target, never secrets. The reply's summary and an error's message are recorded as returned, so they can name elements from the screen (`tapped @e6 "Busy"`, `no element matches "Pay €49.99"`): read trace lines before you share them. Restart the daemon (`phonedriver daemon stop`) with `PHONEDRIVER_TRACE_FULL=1` to keep observation text and text targets, or `PHONEDRIVER_TRACE_SCREENS=1` to keep screenshots.
- **iOS logs:** `$PHONEDRIVER_HOME/runner/<udid>.log` (runtime) and `runner/<hash>/build.log` (first Xcode compile).
- **Clean restart:** `phonedriver daemon stop` shuts down the daemon and any iOS runners it owns; your next command starts fresh.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Done |
| `1` | Command failed |
| `2` | Bad CLI arguments or invalid env config |
| `3` | Human action needed on the device (unlock, trust, allow USB debugging) |
| `4` | No matching device, or it was disconnected |
| `5` | Action held by the [approval guard](/docs/approval-guard/) (it spends money or can't be undone); pass `--autonomous` only if the user granted full autonomy |

## Common snags

**No device found.** Run `phonedriver doctor` and `adb devices`. With multiple devices attached, pass `--device <id>` or export `PHONEDRIVER_DEVICE` from `phonedriver devices`.

**`another UiAutomation client` or `uiautomator dump` gets killed.** Android allows one `UiAutomation` client at a time. Another driver (like mobile-mcp's on-device server) blocks phonedriver, and phonedriver blocks plain `uiautomator dump` while attached. Stop the competing tool, or run `phonedriver daemon stop` when you want `uiautomator dump` back.

**Socket path too long.** macOS caps Unix socket paths around 100 bytes. Point `PHONEDRIVER_HOME` at a shorter directory (e.g. `/tmp/pd`).

**SSH forward dropped.** Errors naming `PHONEDRIVER_SOCKET` mean the tunnel closed—restart your `ssh -L` forward and confirm both hosts run the same phonedriver version.

**`NOT verified` after `type`.** The reply tells you what ended up in the field—often a character cap, input filter, or a webpage that swapped out its search input mid-tap. Follow the hint in the reply.

**Sparse or empty screen text.** Custom canvases and some WebViews expose empty accessibility trees. phonedriver falls back to OCR (`text "Web form" (ocr)`) and pixel inspection for web checkboxes automatically.

**`text recognition on Linux uses Tesseract, which is not installed`.** Reading text from a screenshot (a screen with no element list, `snapshot --pixels`, `compare`) uses Tesseract on Linux, which the system does not ship: `apt install tesseract-ocr`. macOS and Windows use the text recognition the system ships.

**Slow iOS taps.** Physical iPhones sit around 450–530 ms per tap because of XCTest's event confirmation (not a hang). On the iOS Simulator, 0.1.5 uses native touch input for supported single-finger gestures ([iOS Simulator guide](/docs/ios-simulator/)).

**`--display hidden` unsupported.** Virtual displays require Android 13 (API 33)+ and don't support `press lock` or `press notifications` ([Android phone guide](/docs/android-phone/#a-hidden-screen-for-the-agent)).

**Tap held for approval.** Exit code 5 or a `needs_approval` message—see [the approval guard](/docs/approval-guard/).

## Still stuck?

[Open an issue]({{REPO}}/issues) with your `phonedriver doctor` output and the relevant trace lines (`--secret` text is never recorded in them).
