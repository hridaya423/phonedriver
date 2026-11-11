---
title: How to troubleshoot phonedriver
nav: Troubleshooting
meta: Start with phonedriver doctor, then the daemon log and traces. Exit codes and the common errors: another UiAutomation client, WebViews, device setup.
description: Start with phonedriver doctor, then read the daemon log and traces. Exit codes, what each means, and the errors seen most often with another UiAutomation client, WebViews and device setup.
---

## Start with doctor

```sh
phonedriver doctor
```

It checks the toolchain, devices and the socket, and says how to fix each problem. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`.

## Where the evidence is

- `$PHONEDRIVER_HOME/daemon.log` (default `~/.phonedriver/daemon.log`) holds the daemon's own messages and startup errors.
- `$PHONEDRIVER_HOME/traces/<id>.jsonl` has one line per request: method, parameters, the reply's summary, `ms` and settle time. Typed text is recorded only as a length. Start the daemon with `PHONEDRIVER_TRACE_FULL=1` to keep observation text, and `PHONEDRIVER_TRACE_SCREENS=1` to keep screenshots. A running daemon keeps its options, so run `phonedriver daemon stop` first.
- iOS: `$PHONEDRIVER_HOME/runner/<udid>.log` is the runner's output, and `runner/<hash>/build.log` is its first build.
- `phonedriver daemon stop` stops the daemon and the iOS runners it started. The next command starts a new one.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | The command failed |
| 2 | The command line is wrong |
| 3 | A person must act on the device: unlock, trust, allow |
| 4 | No matching device, or it was disconnected |
| 5 | A tap that spends money or cannot be undone needs the human's approval. Add `--autonomous` only when the user has granted full autonomy |

## Common problems

**No device.** Run `phonedriver doctor` and `adb devices`. With more than one device, pass `--device` or set `PHONEDRIVER_DEVICE` with an id from `phonedriver devices`.

**`another UiAutomation client`, or `uiautomator dump` is killed.** Android allows one UiAutomation connection at a time, and phonedriver's agent holds it. Another tool that holds it, such as mobile-mcp's on-device server, blocks the other. Stop the other tool, or stop phonedriver's daemon with `phonedriver daemon stop`. Plain `adb shell uiautomator dump` fails while phonedriver is attached for the same reason.

**The socket path is too long.** On macOS the socket path must stay under about 100 bytes. Shorten `PHONEDRIVER_HOME`.

**A forward to another machine is down.** The error names `PHONEDRIVER_SOCKET`. Restart the `ssh -L` forward and check both machines run the same version.

**`NOT verified` after `type`.** The reply says what the field shows and why: a length limit, filtered characters, or focus elsewhere. On a web page the site may swap its search field after the tap, and the text lands in the old one. Act on the hint in the reply.

**Empty or sparse screen text.** WebViews and custom views can have an empty accessibility tree. phonedriver falls back to OCR (`text "Web form" (ocr)`) and a screenshot read. A web checkbox in a WebView has no checked state in the tree, so its state comes from the screenshot.

**An iOS tap is slow.** That is XCTest, about 240 ms of confirmation, not a hang. See [the iOS Simulator page](/docs/ios-simulator/).

**`--display hidden` fails as unsupported.** The hidden screen needs Android 13 (API 33) or later, and `press lock` and `press notifications` are not available in that mode. Use the default screen, or leave `PHONEDRIVER_DISPLAY` unset. See [the Android phone page](/docs/android-phone/#a-hidden-screen-for-the-agent).

**A tap was held.** Exit code 5, or a reply that says it needs the human's approval. See [the approval guard](/docs/approval-guard/).

## Report a problem

Open an issue at [the repository]({{REPO}}/issues) with the output of `phonedriver doctor` and the relevant trace lines. Text passed with `--secret` is never in them.
