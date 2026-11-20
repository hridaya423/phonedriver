---
title: "How phonedriver works: reading a phone's screen without uiautomator dump"
short: How phonedriver reads a phone without uiautomator dump
meta: How phonedriver lets an AI agent operate Android and iOS phones: stable refs, a resident helper, video settle detection, and an iOS runner.
description: Why giving an AI a phone is mostly fighting recycled list IDs, cold-start taxes, and screens that refuse to sit still.
date: 2026-11-14
updated: 2026-11-20
label: ENGINEERING
short_card: How phonedriver works, without uiautomator dump
card_sub: Recycled list rows, a process that stays open, two settle thresholds, and the iOS runner.
seo: How phonedriver works: reading a phone without uiautomator dump
---

Scroll a long list on Android and something sneaky happens behind the glass. The view holding "Row 13" doesn't scroll off the top and vanish. Android peels off its label and hands its exact same node ID to "Row 29" down at the bottom. An AI agent tracking that ID will happily tap sixteen rows off, and Android will return a polite `ok`.

Most phone-automation headaches look like that. Reading an accessibility tree fast is the easy half. The hard half is keeping element references honest across scrolls, figuring out when a screen has actually finished moving, and sending back only what changed so the model isn't paying to re-read the whole phone on every turn.

This is how phonedriver handles Android and iOS, what prior work it borrows from (scrcpy, WebDriverAgent, Playwright MCP, agent-browser), and the corners that are still sharp.

## What the model sees

Three moving parts: the **agent** is the AI calling phonedriver, the **daemon** sits on your computer, and the **helper** lives on the phone.

Instead of handing the model 25 KB of nested XML, phonedriver turns the screen into a short numbered list. Here is Settings on an Android emulator:

```screen
$ phonedriver snapshot
app com.android.settings (Settings)
@e1 button "Search Settings"
@e2 group scrollable↓
  @e3 button "Google" "Services & preferences"
  @e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  @e5 button "Connected devices" "Bluetooth, pairing"
  ...
  @e10 button "Display & touch" "Dark theme, font size, touch"
```

`@e10` is a ref. Tap it, and phonedriver waits for the Display screen to finish animating before returning the new list, which includes `@e24 switch "Dark theme" off`. Tap that switch, and the model gets back two lines instead of a whole screen:

```screen
$ phonedriver tap @e24
ok tapped @e24 "Dark theme"
~ @e23 button "Dark theme" "Will turn on when Bedtime starts" → "Will never turn off automatically"
~ @e24 switch "Dark theme" off → on
```

`+` appeared, `-` left, `~` changed. When more than 60% of the screen's refs turn over, you landed somewhere new, so phonedriver sends a fresh snapshot. When a screen refuses to stop moving before the timeout, it returns what it has and marks `settled: false`.

Refs are borrowed straight from the web world: [Playwright MCP](https://playwright.dev/mcp/snapshots) prints `[ref=e5]`, and Vercel's [agent-browser](https://github.com/vercel-labs/agent-browser) uses `@e1` over a local daemon. Mobile adds the recycled-row wrinkle.

::figure[recycled-row]

In my API 37 spike, scrolling a 1,000-row `ListView` four times kept node ID `-4294967273` alive while its text flipped from `"Row 13"` to `"Row 29"`. So phonedriver only lets a ref follow a platform ID when the role still matches and either the label is untouched or the bounds drifted less than 8 points. Every tap re-resolves against a fresh tree. If the button moved out, the call fails with the closest current match instead of tapping whoever took its chair.

## Why adb burns two seconds per glance

Every `adb shell uiautomator dump` greets your phone like a stranger. AOSP's launcher script runs `exec app_process ... Launcher`, opens a fresh `UiAutomationShellWrapper`, calls `waitForIdle(1000, 10000)` (insisting on one full second with zero accessibility events), and writes raw XML to disk.

Even a dead-still screen pays over a second for that ceremony. A screen with a live animation or ticker can sit there for ten seconds and fail with `ERROR: could not get idle state.` That happened on a real HONOR X8's main Settings page after 11.5 seconds. phonedriver read the same page in 110 ms.

The workaround has been hiding in plain sight since 2018. [scrcpy](https://github.com/Genymobile/scrcpy) showed that you can push a small dex jar over adb and run it with `app_process` as the shell user: no APK to install, no accessibility switch buried in Settings. phonedriver's Android helper uses the same trick and includes scrcpy's encoder files under Apache-2.0 (`NOTICE` lists all of them).

::figure[pipe-vs-cold]

On connect, the daemon pushes `/data/local/tmp/phonedriver-agent-<sha8>.jar` once, starts it over one `shell,v2,raw:` stream, and sends a 32-byte random token on stdin. Abstract Unix sockets on Android have no permissions at all; without that token, any installed app could connect to the socket and inherit shell-level touch injection. Holding the shell stream open gives cleanup for free: if the daemon crashes, `adbd` kills the helper in roughly a second.

I looked at two other designs and left them on the shelf. An installed `AccessibilityService` APK makes the user dig through Settings to switch it on and lingers as an app afterward. An instrumentation APK (Appium's UiAutomator2 route) needs installation and test signing, though it stays resident too. That makes UiAutomator2 the fairer speed rival to phonedriver; if you already run Appium suites or cloud device farms, [stick with Appium](/compare/appium/).

:::receipts Speed numbers & scope (11 November 2026)
- **Emulator (Android 17, API 37, software rendering, 50 runs after 20 warm-ups):** 1,000-row list read **12.7 ms** vs adb's **2,163.7 ms** (**170x**, 95% CI 162.0–177.4); Settings read **51.0 ms** vs **2,215.7 ms** (**43.5x**); 100-char typing **158.3 ms** vs **3,638.4 ms** (**23x**); tap-then-diff **514.0 ms** vs **2,477.2 ms** (**4.8x**); bare paced tap **28.7 ms** vs **69.4 ms** (**2.4x**); cold start to first read **1,176.0 ms**.
- **Real phone (HONOR X8, Android 13, USB, 9 November):** screen read **78.5 ms** vs **2,506.6 ms** (**32x**); screenshot **16.4 ms** vs **1,005.7 ms** (**61x**).
- **0.1.5 screenshot note:** The 9.1 ms emulator screenshot figure came from 0.1.1, which returned the latest cached video frame. In 0.1.5 screenshots wait on a device barrier for freshness ([D74]({{REPO}}/blob/main/docs/decisions.md#d74-a-screenshot-waits-for-the-frames-the-encoder-still-holds-and-a-hidden-display-on-a-software-encoder-is-lighter)). Full speed deep-dive: [170x faster than adb](/blog/how-phonedriver-reads-a-screen-170x-faster-than-adb/).
:::

## Four UiAutomation quirks learned the hard way

A shell process has no Android `Context`, so the helper builds `new UiAutomation(Looper, IUiAutomationConnection)` directly and calls the hidden `connect(int flags)` over reflection (12–43 ms). Four sharp edges showed up immediately:

- **Don't kick TalkBack out.** By default, `UiAutomation` unbinds every other accessibility service while it holds the floor. Passing `FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` lets TalkBack stay bound.
- **Clear the cache.** Because the helper subscribes only to window events, skipping `clearCache()` makes a read right after a tap return yesterday's label.
- **Prefetch saves binder trips.** Every tree walk pays roughly one binder round trip per 2 to 4 nodes. Switching to a depth-first `getChild` walk with prefetch cut Settings main (192 nodes) from 130 ms to 75 ms.
- **There can be only one.** Android allows one `UiAutomation` client at a time. While phonedriver is connected, `uiautomator dump` dies with exit 137. You can't run phonedriver and another accessibility driver on the same Android phone simultaneously.

## Knowing when the screen has stopped moving

Fixed sleeps are either sluggish or wrong. Accessibility content-change events are worse: apps fire them for invisible updates and ticking timers, so a clock on screen would stall you forever.

phonedriver watches the pixels instead. The helper mirrors the display into a MediaCodec H.264 stream, the daemon decodes it with OpenH264, and the settle detector diffs a 4×4-point luma thumbnail of each frame against the previous one.

My first version used a single threshold: 4,096 pt² (a 64×64-point square) to separate real page transitions from blinking carets and spinners. Page transitions worked great. Material switches did not: **zero of 20 toggle taps ever cleared 4,096 pt²**, so phonedriver kept reporting `changed: false` on switches that had visibly flipped.

One failing test later, settle became two phases. First, wait up to 500 ms (3 s after `launch`) for a window event or any movement above **128 pt²**, just past a blinking caret. Once something moves, wait for **250 ms of quiet** with no frame above **4,096 pt²**.

::figure[settle-scope]

Two real-world quirks surfaced after that. First, an emulator's software encoder doesn't drip frames evenly; it fires bursts of 10 fps and then goes mute for up to 5 seconds. A mid-animation pause longer than 250 ms can trick settle into returning early, while looping video runs out the 5 s clock and returns `settled: false`. On iOS 26 and below there is no frame stream at all, so the detector polls gray screenshots instead. Second, in 0.1.5 screenshots stopped grabbing whatever stale frame happened to sit in memory and started waiting on a freshness barrier so you never get a photo from *before* your tap.

## Turning 25 KB of XML into 14 lines

On the HONOR X8's Display & brightness page, `uiautomator dump` emitted 25,334 bytes of XML. phonedriver sent the model 461 bytes.

Four pruning rules do the heavy lifting: drop invisible nodes; hoist child `TextView` labels onto clickable wrapper layouts (on Android, the button you actually tap is almost always an anonymous `LinearLayout` wrapping two text views); collapse single-child decorative boxes; and render the on-screen keyboard as **one** `keyboard` line instead of sixty individual keys, so opening the keyboard stops looking like a brand-new screen in every diff.

When the accessibility tree fibs, pixels step in. A Chrome WebView checkbox reports neither checkable nor checked before or after a tap, which used to make agents take screenshots just to verify the tick. The daemon now inspects the checkbox's pixels directly. (A custom checkbox with a white or gray tick still reads `off`, because pixel heuristics have blind spots too.)

That tiny diff is why phonedriver used **6.9x fewer tokens** than mobile-mcp across nine real phone jobs, mostly because fewer turns stop the prompt history from snowballing. The full breakdown (including the two jobs phone-harness won) lives in [the nine-job benchmark post](/blog/phonedriver-vs-mobile-mcp-vs-phone-harness-vs-adb-real-phone-jobs/).

## iOS: my own runner

On iOS, [WebDriverAgent](https://github.com/appium/WebDriverAgent) is an XCTest bundle that wraps Apple's private accessibility APIs in an HTTP server. phonedriver uses the same mechanism with a smaller Objective-C runner: one test method that never returns and speaks length-prefixed JSON on TCP port 27753, leaving behind the WebDriver HTTP layer, XPath engine, and session machinery. (`NOTICE` credits WDA for the private headers.)

Before keeping a custom runner, I drew a line in the sand: if it wasn't at least **2.0x faster** than *fully tuned* WDA (`snapshotMaxDepth=30`, idle waits off, noisy attributes excluded), I would bin it and ship WDA.

| Screen (iPhone 17 Simulator, iOS 26.5) | phonedriver runner | WDA (depth 30 only) | WDA (fully tuned) | vs. tuned |
|---|---|---|---|---|
| List (150 elements) | 22.2 ms | 380.4 ms | 44.8 ms | **2.02x** |
| Taps (59 elements) | 12.9 ms | 108.8 ms | 26.1 ms | **2.02x** |
| Settings (168 elements) | 24.7 ms | 477.4 ms | 49.7 ms | **2.01x** |

It cleared my own bar by roughly one percent, and only after I ditched XCTest's key-path mapping and queried eight raw accessibility attributes directly (build one came in at 1.91x). The upkeep is real: private headers shift between Xcode releases, so occasional breakage after an Xcode update comes with the territory, and you still need a Mac with Xcode (plus an Apple Development certificate for a real iPhone).

In 0.1.5, the iOS Simulator gained native single-finger touch input and direct display-buffer settling, bypassing XCTest's ~240 ms event confirmation on supported gestures while typing, multi-touch, and physical iPhones stay on the runner ([D93]({{REPO}}/blob/main/docs/decisions.md)). A Simulator tap with its reply now takes 316 ms and a read 29 ms, the fastest of four tools on [simulator-use's benchmark](/benchmarks/#simulator); on a real iPhone a tap still takes 450 to 530 ms because XCTest confirms an event about 240 ms late.

## The tap that asks

An agent with a phone can do anything your thumb can do, including tapping `Buy with GPay`. Worse, untrusted text sitting on a webpage can try to talk the model into doing it (Simon Willison's [lethal trifecta](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/)).

So the check lives in the tool, not the prompt. When a tap targets a recognized payment or account-deletion label across 11 languages, the daemon halts with `needs_approval` and the MCP server pops a prompt straight to *you* via MCP elicitation. The model never sees or answers that form. It is a seatbelt rather than a vault: icon-only buttons aren't matched, and `autonomous: true` skips it. [The approval guard post](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/) walks through the whole design and every gap.

## What is still rough

- **Bare taps aren't instant.** Paced taps take 28.7 ms on the emulator, back-to-back taps stretch to 175.6 ms at p95 (queuing behind Android's button ripple), and physical iPhones still run 450–530 ms per tap.
- **Hardware coverage is small.** One Android phone (HONOR X8) was timed for two ops, the WDA comparison is Simulator-only, and the nine-job benchmark is Android-only.
- **Locked phones & `FLAG_SECURE`.** phonedriver never enters passcodes or passes biometrics. Capture-protected screens come back black in screenshots, though their accessibility text still reads.
- **Empty trees.** Custom canvases like the Photos editor expose no accessibility tree at all, so phonedriver falls back to OCR and screenshots.
- **OpenH264 patent note.** Because phonedriver compiles OpenH264 from source rather than shipping Cisco's binary, this build carries no H.264 patent license.

:::receipts Reproduce the runs or try it yourself
- **Try it:** `npx -y phonedriver init` (or `claude mcp add phonedriver -- npx -y phonedriver mcp` and `phonedriver doctor`).
- **Speed suite:** `PH_PYTHON=/path/to/venv/bin/python bash bench/speed/reproduce.sh emulator-5554` — raw samples in [`docs/bench/data/speed-2026-11-11/`]({{REPO}}/blob/main/docs/bench/speed.md).
- **Jobs suite:** `bench/agent-eval/run-jobs.sh` — raw transcripts and verdicts in [`docs/bench/jobs.json`]({{REPO}}/blob/main/docs/bench/jobs.json).
- **Still unmeasured:** how fast a resident Appium UiAutomator2 server reads the same 1,000-row list on the same emulator. If you run it, open an issue with the samples.
:::
