---
title: How to control a real iPhone or iPad with phonedriver
nav: Real iPhone
description: Plug in an iPhone or iPad, run phonedriver ios setup once (from a Mac or from Windows), and optionally unplug the cable over Wi-Fi. Tested on iPhone 13 Pro Max, 14 Plus, 16e, and iPad Pro.
updated: 2026-11-21
seo: How to let an AI agent control a real iPhone or iPad over MCP
---

Plug an iPhone or iPad into a Mac with Xcode and run `phonedriver ios setup`, or from Windows follow [iPhone from Windows](#iphone-from-windows). That signs and starts phonedriver's minimal XCTest runner with your Apple developer team—a free Apple ID works fine (its signature lasts 7 days, and the next command after that signs the runner again by itself).

Tested on an iPhone 13 Pro Max (iOS 27.0.1), an iPhone 14 Plus (iOS 26.1), an iPad Pro (iOS 27.0), and, from Windows, an iPhone 16e (iOS 26.5). Snapshots, screenshots, and settled diffs feel snappy; bare taps sit at 454–534 ms at p50 because of XCTest's confirmation delay.

## First-time setup

Plug the device in, unlock it, and run:

```sh
phonedriver ios setup                    # signing team, pairing, Developer Mode, DDI, runner
phonedriver --device ios:<udid> snapshot
```

`ios setup` prints each step and tells you when a prompt needs your thumb on the phone. It detects your signing team automatically from your Mac's Apple Development certificate (create one in **Xcode → Settings → Accounts → Manage Certificates** if you don't have one yet). With multiple teams, export `PHONEDRIVER_TEAM` and run `phonedriver daemon stop` so the daemon picks it up.

If the iPhone hasn't been registered to that team yet, `ios setup` will prompt you to run `phonedriver ios register`. Mounting Apple's developer disk image (DDI) is the single network call phonedriver makes to Apple. On Windows, `--download-ddi` fetches that image once from GitHub instead ([every connection](/docs/no-telemetry/)); nothing else leaves your machine.

## iPhone from Windows

phonedriver drives an iPhone or iPad on iOS 17 or later from Windows, with no Mac. Xcode is what builds the runner on a Mac, so on Windows you install a runner built for each release instead:

1. Install **Apple Devices** from the Microsoft Store (or iTunes). Plug the phone in, unlock it and tap Trust.
2. Download `PhonedriverRunner.ipa` from the release of the phonedriver you run ([the latest]({{REPO}}/releases/latest/download/PhonedriverRunner.ipa); `phonedriver doctor` gives the link for yours). GitHub's macOS machines build it unsigned with each release. From 0.1.7 its checksum is beside it (`PhonedriverRunner.ipa.sha256`), and `gh attestation verify PhonedriverRunner.ipa --repo hridaya423/phonedriver` shows it came from the release workflow.
3. Install it with [Sideloadly](https://sideloadly.io), signed with your Apple ID. A free Apple ID's signature lasts 7 days, and Sideloadly can sign it again. phonedriver never sees your Apple ID. On the phone, trust it under Settings > General > VPN & Device Management.
4. Turn on Settings > Privacy & Security > Developer Mode and restart the phone.
5. Run `phonedriver ios setup --download-ddi`. It downloads Xcode 27.2's developer disk image once from [doronz88/DeveloperDiskImage](https://github.com/doronz88/DeveloperDiskImage) at a pinned commit, checks each file's SHA-256, and mounts it. With the three files from a Mac's Xcode, use `--ddi <folder>` instead. Then it finds the runner and starts it.
6. Optional: with the phone still plugged in and unlocked, `phonedriver ios wifi on`, then unplug it. It stays in `phonedriver devices` while both are on the same network and the phone is awake. Everything goes through the pairing's encrypted tunnel, and phonedriver checks that the phone on the network is the one it paired with. A pairing made before 0.1.7 cannot be checked, so phonedriver asks you to plug the phone in and run `phonedriver ios wifi on` once more.

Tested on an iPhone 16e (iOS 26.5) from Windows 11. On the cable: setup, `snapshot`, `screenshot`, `tap`, `type` and `press back`. Over Wi-Fi: `doctor`, `snapshot`, `screenshot`, `apps`, `launch`, `tap` and `press back`. A tap takes 1.3 to 1.5 s there, against 0.45 to 0.53 s from a Mac, because the screen's settle is read from polled screenshots. A locked phone drops off Wi-Fi now and then, and the next command connects again in 10 to 19 s. Linux takes the same path with usbmuxd, but it has not been tried on a phone.

## Cut the cable (Wi-Fi)

Once setup succeeds, while still plugged in:

```sh
phonedriver ios wifi on
```

Unplug the cable. As long as the iPhone and Mac share a Wi-Fi network, `phonedriver devices` keeps listing it and every command works (`phonedriver doctor` reports `over Wi-Fi` or `over USB`; `phonedriver ios wifi off` turns it back off). Note that a sleeping iPhone drops off Wi-Fi until woken, and taps over Wi-Fi were verified on iOS 27.0.1 (older iOS < 26.4 discovery was verified without full tap runs).

One caveat from a Mac over Wi-Fi: traffic between phonedriver and its runner on the phone is not encrypted. It includes the screen and what the agent types, `--secret` text too, so anyone who can read your network can read it. Nobody else can drive the runner—phonedriver proves a token the runner was started with, and the runner refuses everyone else. For secrets, use the cable.

## Lock screen & sleep

phonedriver never enters passcodes and cannot pass Face ID. If the phone locks, commands stop with `The device is locked` and exit code 3 until you unlock it.

While active (0.1.4+), phonedriver nudges the Auto-Lock timer every 10 seconds for 10 minutes after each command so your phone doesn't fall asleep mid-task; `phonedriver release` ends that immediately.

## How a physical iPhone differs from Android

- **No shell:** Physical iPhones don't expose a device shell.
- **Polled settle:** Settle watches polled screenshots rather than a continuous H.264 stream, adding slight overhead per action.
- **Notification banners:** If a banner drops over the top of the screen, phonedriver waits up to 8 seconds for it to clear before tapping underneath it (turning on Do Not Disturb for long sessions is still smart).
- **Experimental CoreDevice tier:** iOS 27 devices have an opt-in `PHONEDRIVER_IOS_TIER=coredevice` path, off by default.

The [approval guard](/docs/approval-guard/) is on by default—read [what it misses](/docs/approval-guard/#what-it-does-not-catch) before letting an agent loose on your daily phone.

## Questions

### Does phonedriver use WebDriverAgent?

No—it builds its own small Objective-C XCTest runner without the WebDriver HTTP server or XPath engine (crediting WebDriverAgent for private headers). On an iPhone 17 Simulator it reads screens 2.0x faster than fully tuned WebDriverAgent ([phonedriver vs Appium](/compare/appium/)).

### Do I need a paid Apple developer account?

No. A free Apple ID works. Its signature lasts 7 days; when it has run out, the next phonedriver command signs the runner again and installs it, with the phone attached and unlocked. A paid account's signature lasts a year.

### Can I control an iPhone from Windows or Linux?

From Windows, yes, on the cable or over Wi-Fi: see [iPhone from Windows](#iphone-from-windows). Linux runs the same code but is untested on a phone. The iOS Simulator still needs a Mac. Android works on macOS, Linux, and Windows.

### Can the agent unlock my iPhone?

No. phonedriver never types passcodes or touches Face ID. When a phone is locked, commands stop cleanly so you can unlock it yourself.
