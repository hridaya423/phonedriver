---
title: How to control a real iPhone or iPad with phonedriver
nav: Real iPhone
description: Set up an iPhone or iPad and let an agent drive it with phonedriver, from a Mac or from Windows, with or without the cable. Tested on an iPhone 13 Pro Max, a 14 Plus, a 16e and an iPad Pro.
updated: 2026-11-21
seo: How to let an AI agent control a real iPhone or iPad over MCP
---

To let an AI agent control a real iPhone or iPad, plug it into a Mac with Xcode and run `phonedriver ios setup`, or from Windows follow [iPhone from Windows](#iphone-from-windows). The agent then reads the screen as text and taps, types and swipes through phonedriver's own XCTest runner. The runner is signed with your Apple developer team. A free Apple ID works. Its signature lasts 7 days, and the next command after that signs the runner again by itself.

I tested an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0) through the CLI and MCP. Screenshots, snapshots and the diff after a tap meet the targets I set. A bare tap does not: it takes 454 to 534 ms at p50 against a target of 80 ms. The 12-scenario suite was not run on devices.

## Set it up

Plug the device in and unlock it, then:

```sh
phonedriver ios setup                    # signing team, pairing, Developer Mode, developer disk image, then the runner
phonedriver --device ios:<udid> snapshot
```

`ios setup` prints one line for each step and says what to do for a step that needs you. phonedriver reads the signing team from this Mac's Apple Development certificate. To make one, sign in under Xcode > Settings > Accounts, choose Manage Certificates and add an Apple Development certificate. With certificates for several teams, set `PHONEDRIVER_TEAM` to the ID of the one to use.

Without a team, the command stops with exit code 3 and says so. If the iPhone has never been used with that team, the command asks for `phonedriver ios register`. That lets Xcode add the iPhone to the team's Apple developer account, which holds 100 iPhones a year. If you set `PHONEDRIVER_TEAM`, run `phonedriver daemon stop` afterwards: the daemon reads it when it starts.

Mounting the developer disk image is the one thing phonedriver does that talks to Apple. Nothing else leaves your machine.

## iPhone from Windows

phonedriver drives an iPhone or iPad on iOS 17 or later from Windows, with no Mac. Xcode is what builds the runner on a Mac, so on Windows you install a runner built for each release instead:

1. Install **Apple Devices** from the Microsoft Store (or iTunes). Plug the phone in, unlock it and tap Trust.
2. Download `PhonedriverRunner.ipa` from the release of the phonedriver you run ([the latest]({{REPO}}/releases/latest/download/PhonedriverRunner.ipa); `phonedriver doctor` gives the link for yours). GitHub's macOS machines build it unsigned with each release.
3. Install it with [Sideloadly](https://sideloadly.io), signed with your Apple ID. A free Apple ID's signature lasts 7 days, and Sideloadly can sign it again. phonedriver never sees your Apple ID. On the phone, trust it under Settings > General > VPN & Device Management.
4. Turn on Settings > Privacy & Security > Developer Mode and restart the phone.
5. Run `phonedriver ios setup --download-ddi`. It downloads Xcode 27.2's developer disk image once from [doronz88/DeveloperDiskImage](https://github.com/doronz88/DeveloperDiskImage) at a pinned commit, checks each file's SHA-256, and mounts it. With the three files from a Mac's Xcode, use `--ddi <folder>` instead. Then it finds the runner and starts it.
6. Optional: with the phone still plugged in and unlocked, `phonedriver ios wifi on`, then unplug it. It stays in `phonedriver devices` while both are on the same network and the phone is awake. Everything goes through the pairing's encrypted tunnel, and phonedriver checks that the phone on the network is the one it paired with. A pairing made before 0.1.7 cannot be checked, so phonedriver asks you to plug the phone in and run `phonedriver ios wifi on` once more.

Tested on an iPhone 16e (iOS 26.5) from Windows 11. On the cable: setup, `snapshot`, `screenshot`, `tap`, `type` and `press back`. Over Wi-Fi: `doctor`, `snapshot`, `screenshot`, `apps`, `launch`, `tap` and `press back`. A tap takes 1.3 to 1.5 s there, against 0.45 to 0.53 s from a Mac, because the screen's settle is read from polled screenshots. A locked phone drops off Wi-Fi now and then, and the next command connects again in 10 to 19 s. Linux takes the same path with usbmuxd, but it has not been tried on a phone.

## Without the cable

After the setup, with the device still on the cable:

```sh
phonedriver ios wifi on
```

Unplug it. While the iPhone and the Mac are on the same Wi-Fi network, `phonedriver devices` still lists it and every verb works. `phonedriver doctor` says `over Wi-Fi` or `over USB`. The iPhone answers only computers it already trusts, and `phonedriver ios wifi off` stops it.

From a Mac over Wi-Fi, the traffic between phonedriver and its runner on the phone is not encrypted. That traffic includes the screen and what the agent types, `--secret` text too, so anyone who can read your network can read it. Nobody else can drive the runner: phonedriver proves a token the runner was started with, and the runner refuses everyone else. For secrets, use the cable.

I tested this on an iPhone 13 Pro Max (iOS 27.0.1) on one network. The app on the phone logged the taps and the typed text with no cable attached. An iPhone 14 Plus on iOS 26.1, which announces itself the older way, stayed in `phonedriver devices` and answered with the cable out. I did not run taps on that phone over Wi-Fi, so taps over Wi-Fi on an iOS before 26.4 are not tested.

A sleeping iPhone stops answering on Wi-Fi. Both phones left the list when the screen went off and came back when woken.

## A locked iPhone

phonedriver does not unlock an iPhone and never enters a passcode. On a locked iPhone each command stops with `The device is locked` and you unlock it by hand.

phonedriver keeps an iPhone awake while it is in use: for 10 minutes after each command it resets the Auto-Lock timer every 10 seconds, and `phonedriver release` stops that. This came with 0.1.4. In 0.1.3 and earlier, set Settings > Display & Brightness > Auto-Lock to Never before a long task, and set it back after.

## What differs from Android

- There is no shell on a physical iPhone.
- Settle is detected by polling screenshots, not a video stream, so each action costs a little more.
- iOS 27 devices have an experimental CoreDevice tier, off by default: `PHONEDRIVER_IOS_TIER=coredevice`. It is not the supported path.

## Notification banners

A banner covers the top of the app for a few seconds, and a tap under it would open the notification. phonedriver waits for the banner to leave before it touches that part of the screen, up to 8 seconds. I tested this on the Simulator. For a long job, Do Not Disturb on the phone is still the safer setting.

## What stays with you

Unlocking, trusting the computer and Developer Mode prompts are yours. The error says what to do on the device and the command line exits with code 3.

## Safety

The [approval guard](/docs/approval-guard/) is on by default. Read its limits before you give an agent a phone with accounts on it.

## Questions

### Does phonedriver use WebDriverAgent?

No. It builds its own XCTest runner on your Mac. WebDriverAgent is an XCTest runner too, so the mechanism is the same. Mine leaves out the WebDriver HTTP server, the XPath engine and the session model. On an iPhone 17 simulator it read a screen 2.0 times faster than fully tuned WebDriverAgent at p50. If you already run Appium, WebDriverAgent is the better choice. See [phonedriver vs Appium](/compare/appium/).

### Do I need a paid Apple developer account?

No. A free Apple ID works. Its signature lasts 7 days; when it has run out, the next phonedriver command signs the runner again and installs it, with the phone attached and unlocked. A paid account's signature lasts a year.

### Can I control an iPhone from Windows or Linux?

From Windows, yes, on the cable or over Wi-Fi: see [iPhone from Windows](#iphone-from-windows). Linux runs the same code but is untested on a phone. The iOS Simulator still needs a Mac.

### Can the agent unlock my iPhone?

No. phonedriver never enters a passcode and cannot pass Face ID. On a locked iPhone each command stops with `The device is locked`, and you unlock it by hand.
