---
title: How to control a real Android phone with phonedriver
nav: Real Android phone
meta: Turn on USB or Wi-Fi debugging, plug in an Android phone, and let an agent drive it—even on a hidden second display while you keep using the phone.
description: Connect a real Android phone over USB or Wi-Fi debugging, optionally give the agent a hidden second screen, and see what has been tested on hardware.
seo: How to let an AI agent control a real Android phone over MCP
updated: 2026-11-20
---

Plug in an Android phone or tablet with USB debugging switched on, and phonedriver drives it straight over `adb`. No APK to install, no accessibility service to hunt down in Settings, no root required.

## Over USB

1. On the phone, enable **Developer options**, then **USB debugging**.
2. Plug it in and accept the **"Allow USB debugging"** prompt.
3. Verify phonedriver sees it:

```sh
phonedriver devices
phonedriver doctor --device android:<serial>
```

`doctor` without `--device` lists the phone but does not start the agent on it.

Your phone shows up as `android:<serial>`. The daemon pushes its tiny helper jar over `adb` and runs it as the shell user (the same trick `scrcpy` uses).

## Over Wi-Fi

On Android 11+, you can lose the cable. The quickest path is one QR scan:

```sh
phonedriver pair      # scans from Wireless debugging → Pair device with QR code
phonedriver untether  # switches a USB-plugged phone to adb over TCP (handy for Tailscale)
```

Or pair manually through `adb` using the addresses shown under **Developer options → Wireless debugging**:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phonedriver devices
```

(The connection port changes whenever you toggle Wireless debugging off and on. If a device goes `offline`, `phonedriver doctor` points out why. A phone connected over both USB and Wi-Fi appears once in `phonedriver devices`.)

## A hidden screen for the agent

On Android 13+, set `PHONEDRIVER_DISPLAY=hidden` (or pass `--display hidden`) so the agent works on a virtual display while your own screen stays untouched. The phone has to be unlocked: while it is locked the hidden screen shows the lock screen too, so phonedriver says the phone is locked and leaves it asleep.

Typing into native app fields stays invisible on your screen. Typing into web pages still triggers Android's system clipboard toast (your clipboard is restored afterward), and `press lock` / `press notifications` aren't available in hidden mode. On a HONOR X8, the first hidden-screen launch took 8.4 s while the virtual display and encoder spun up, then ~1.1 s afterward. Some OEM ROMs (Xiaomi, Samsung, Oppo—not yet tested) may restrict virtual displays.

### Several agents on one phone

Add `+` and a short name to the device id, such as `--device emulator-5554+a` and `+b`, and each agent works on a hidden screen of its own with its own lease. Two or more agents can then use one phone or emulator at the same time while its own screen stays free. `phonedriver devices` lists each lane with the session that holds it. An app runs in one place at a time, so give each agent a different app. It needs Android 13 or later and has been tested on the API 37 emulator.

## What has been tested

Hardware testing so far covers one physical Android phone: a **HONOR X8** (MagicOS, Android 13) over both USB and Wi-Fi. On USB, screen reads took **78.5 ms** vs. `uiautomator dump`'s **2,506.6 ms (32x)**, and screenshots took **16.4 ms** vs. `screencap`'s **1,005.7 ms (61x)**. On the phone's main Settings page, `uiautomator dump` gave up after 11.5 seconds with `"could not get idle state"` while phonedriver read it in 110 ms. Other Android phones follow the exact same path; if yours behaves weirdly, open an issue.

## What stays in your hands

Unlocking the phone, approving USB debugging, and passing biometrics stay strictly yours. Whenever one blocks an action, phonedriver exits with code 3 and tells the agent what you need to do on the phone.

While the agent works, phonedriver keeps the phone awake: for 10 minutes after each command it resets the screen timeout every 10 seconds, on the phone's own screen and on the hidden one, so the phone does not lock under the agent. The screen still dims on its usual schedule. `phonedriver release` stops this and lets the phone sleep again. This comes with 0.1.5.

On physical Android devices, destructive commands (wiping app data or uninstalling apps phonedriver didn't install) are refused unless `--force` is passed. For an app you are developing, name its package in `PHONEDRIVER_APPS_UNDER_TEST` in the environment the daemon starts with, and `apps clear`, `uninstall`, `grant` and `revoke` work on it ([how](/docs/claude-code/#an-app-you-are-developing)). The [approval guard](/docs/approval-guard/) is also on by default—read [what it misses](/docs/approval-guard/#what-it-does-not-catch) before handing an agent a phone signed into banking or shopping apps.

## Keep the agent in your app

`PHONEDRIVER_APP_SCOPE=com.example.app,com.example.other` in the environment the daemon starts with limits the agent to those apps. It does not need a wiped phone, but it is not a sandbox.

- A screen of another app (the home screen too) reads as `app <package> (outside the app scope)` with no elements, and `snapshot`, `find`, `wait` and OCR show none of its text. Traces and saved steps hold only what the agent was shown.
- `tap`, `type`, `swipe`, `scroll`, `clipboard set` and the other actions are refused while another app's window is on screen, a heads-up notification included. `press back`, `press home` and `launch` of an app in the scope still work.
- `screenshot` is refused while a window of another app is on screen, because a picture cannot be cut by window.
- `shell`, `record`, `clipboard get`, `press recents` and `press notifications` are refused. `apps` lists only your apps, and `launch`, `logs` and app changes refuse the rest. `open` needs `--app` with one of your packages.
- The system's permission and install dialogs, the VPN consent dialog and the keyboard stay usable.

Not covered: the package name of the app in front is shown, notification icons and the keyboard's suggestions appear in a screenshot, an app of yours can open another app before phonedriver sees it (what follows is hidden), a share sheet from an unusual maker may be let through as a dialog, and an iPhone takes no screenshot under a scope. The daemon reads the list when it starts: run `phonedriver daemon stop` after changing it. [The reference](https://github.com/hridaya423/phonedriver/blob/main/docs/reference.md#app-scope) has the full list.

## Questions

### Does phonedriver install an app on my Android phone?

No. No APK and no accessibility service—just a small jar pushed to `/data/local/tmp` and started via `app_process` as the shell user, like `scrcpy`.

### Do I need root?

No. Everything runs under the standard `adb` shell user.

### Which Android versions has it been tested on?

Emulators on API 26 and API 37 pass the 21-scenario suite; API 31 (Android 12) was verified in 0.1.4; physical hardware testing covers a HONOR X8 on Android 13. Wi-Fi debugging needs Android 11+ and hidden displays need Android 13+.

### How fast is it on a real phone?

On a HONOR X8 over USB, reading the screen took 78.5 ms vs. 2,506.6 ms for `uiautomator dump` (32x faster), and screenshots took 16.4 ms vs. 1,005.7 ms for `screencap` (61x faster). Full emulator table in [phonedriver vs adb](/compare/adb/).
