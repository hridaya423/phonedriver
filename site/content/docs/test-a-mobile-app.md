---
title: How to have an AI agent test your mobile app
seo: How to have Claude Code test an Android or iOS app on a device
nav: Test your app
description: Install a build on an emulator, Simulator, or phone, let Claude Code or Codex hunt for bugs, and save working flows as zero-model CI replay tests.
meta: Install a build on an emulator, simulator or phone, ask an agent to test it and report bugs, then save working flows as deterministic replay tests.
updated: 2026-11-20
date: 2026-11-14
---

Hand an agent a fresh build and it can install the app, walk through flows like a stubborn QA tester, and hand back bug reports with reproduction steps—no test IDs or brittle scripts required.

On two planted-bug benchmarks (a sign-up form and a shopping cart on Android, 10 November 2026), Claude Sonnet 5.5 found every planted bug in **6 of 6 runs** (median 121,854 and 92,570 tokens). Once a flow works, you can freeze it into a zero-model replay file and run it in CI.

## Install the build

Connect an [Android emulator](/docs/android-emulator/), [Android phone](/docs/android-phone/), or [iOS Simulator](/docs/ios-simulator/), then install and launch:

```sh
phonedriver apps install app-debug.apk      # or YourApp.app on iOS Simulator
phonedriver launch "My App" --restart
```

(Installing directly onto a physical iPhone isn't implemented yet—install from Xcode, then let phonedriver launch and drive it. Over MCP, the agent can call `apps` and `launch` itself.)

Three flags that keep test runs clean:

- `phonedriver launch "My App" --grant notifications,camera` — pre-grants runtime permissions so system dialogs stay out of the way.
- `phonedriver launch "My App" --restart` — force-stops first so every run starts on screen one.
- `phonedriver apps clear "My App"` — wipes app data for fresh first-run testing. On physical devices it works only for apps phonedriver installed and for packages you list in `PHONEDRIVER_APPS_UNDER_TEST` (for an app Gradle or Android Studio installed; set it in the MCP server's `env`, and [the Claude Code page](/docs/claude-code/#an-app-you-are-developing) shows how). The agent cannot add to that list.

On a phone with real accounts on it, set `PHONEDRIVER_APP_SCOPE=com.example.app` in the MCP server's `env` so the agent reads and touches only your app ([what it covers](/docs/android-phone/#keep-the-agent-in-your-app)).

## Check what the model will see

Before unleashing an agent, run `phonedriver snapshot` on your home screen:

```screen
$ phonedriver snapshot
app com.android.settings (Settings)
@e1 button "Search Settings"
@e2 group scrollable↓
  @e3 button "Google" "Services & preferences"
  @e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"
```

If your buttons and inputs show up with clean labels, the agent flies through them in text. If a WebView or custom canvas comes back sparse or empty, phonedriver falls back to OCR (`text "Web form" (ocr)`) and screenshots automatically.

## Ask for a bug hunt

Wire up your client ([Claude Code](/docs/claude-code/), [Codex](/docs/codex/), [VS Code](/docs/vs-code/), or [Cursor](/docs/cursor-and-mcp-clients/)) and ask plainly:

> Open my app, sign up with a new account, and try weird inputs. Report every bug you hit with steps to reproduce it.

> Walk through onboarding in dark mode and light mode, and flag any screen where text is hard to read.

Every tap settles and returns just the diff, so the agent sees what broke immediately. For password or OTP fields, plain `type` is refused—the agent passes test passwords via `secret` so they stay out of logs and traces. Flipping dark mode takes one shell call:

```sh
phonedriver shell "cmd uimode night yes"        # Android
phonedriver shell "simctl ui appearance dark"   # iOS Simulator
```

## Keep the run as a test

Once an agent nails a flow, save it as a deterministic test that runs with **zero LLM tokens**:

```sh
phonedriver save signup.steps
phonedriver replay signup.steps
phonedriver replay signup.steps --screens screens/signup
```

Replay halts on the first failing step and shows where it stopped. With `--screens`, the first replay captures baseline screenshots and later runs flag changed text (ignoring status-bar clocks). Secret values are stripped from saved `.steps` files, so replay pauses when it reaches one. You can replay the same `.steps` files on every commit with [the GitHub Action](/docs/replay-in-ci/).

While the agent works, `phonedriver record start` and `record stop` save an MP4 of the screen with a timed action log; it works on Android and the iOS Simulator (not physical iPhones). `phonedriver compare before.png after.png` says in text what changed between two screenshots.

## Accessibility

`phonedriver snapshot --audit` flags controls with no name a screen reader can say and tap targets under 44 by 44 points on iOS or 48 by 48 on Android, each with its ref. It reads the element list only, so it doesn't judge color contrast or cut-off text. Ask the agent to audit every screen it passes and you get a list per screen.

## Permissions, crashes and the screen

`phonedriver apps grant <app> camera` and `apps revoke <app> location` change a permission without opening the app. `phonedriver apps notify <app> '{"aps":{"alert":"Hi"}}'` (or `--file payload.json`) sends the app a push notification so you can test what it does with one. It works on the iOS Simulator, which delivers it the way Apple's push service would; the app must have asked for notification permission, and a payload without an `aps` object is refused. A physical iPhone and an Android phone cannot be sent one this way, and the reply says why.

On Android, an action during which the app crashed says so in its reply, with the exception and the line that threw it, and `phonedriver logs` prints the app's last lines. `phonedriver wait log:"signed in" --app "My App"` waits for a line your app logs. `press landscape` and `press portrait` turn the screen, and on an emulator or a Simulator `press fingerprint` and `press wrong-fingerprint` answer a fingerprint or Face ID prompt.

## Running headless

Every benchmark on this site ran unattended (`claude -p` against `-no-window` emulators). In headless runs nobody can click Allow on the [approval guard](/docs/approval-guard/), so payment taps pause safely unless explicitly authorized.

## React Native, Expo, and Flutter

I haven't personally tested a React Native, Expo, or Flutter app yet, because phonedriver doesn't care which framework built the binary—it reads whatever accessibility tree Android or iOS exposes. Run `phonedriver snapshot` on your build; if controls are missing, [open an issue]({{REPO}}/issues) with the snapshot and I'll dig in.

:::receipts Benchmark numbers & limits
- **Planted-bug benchmark (10 November 2026, Claude Sonnet 5.5, Android emulator):** `qa-signup` (email banner drops last char; Create ignores tap after rejected submit) passed **3/3** at **121,854** median tokens (mobile-mcp: 2/2, 549k; phone-harness: 1/2). `qa-shop` (stale total after Decrease; `SAVE10` subtracts flat 10.00) passed **3/3** at **92,570** tokens (mobile-mcp: 2/2, 473k; phone-harness: 2/2). Full method on [benchmarks](/benchmarks/#jobs).
- **Limits:** `FLAG_SECURE` screens produce black screenshots (text still reads); a tap alone takes 450 to 530 ms on a real iPhone against 29 to 39 ms on an Android emulator, so a long iPhone flow is slower; a saved replay repeats the exact steps one agent took, found again by their text, and doesn't self-heal like a live agent—for suites your team writes and reviews as code on cloud device farms, see [phonedriver vs Appium](/compare/appium/).
:::

## Questions

### Can Claude Code test an app on the iOS Simulator?

Yes. Boot a Simulator on a Mac with Xcode, install your `.app` bundle with `phonedriver apps install`, and ask Claude to test it.

### Can the agent test on a real phone?

Yes—Android over USB or Wi-Fi with nothing installed, and iPhone from a Mac after `phonedriver ios setup`. For unattended runs, use an emulator or a spare phone with no real accounts signed in.

### How many tokens does an agent test run use?

On the two Android bug-finding benchmarks (Claude Sonnet 5.5), medians were 121,854 tokens for the sign-up form and 92,570 for the shop screen. Your own app and prompt complexity will vary.
