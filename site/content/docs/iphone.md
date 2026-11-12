---
title: How to control a real iPhone or iPad with phonedriver
nav: Real iPhone
description: Pair an iPhone or iPad, set up Developer Mode and a signing team, and let an agent drive it with phonedriver. Tested on an iPhone 14 Plus and an iPad Pro.
---

A physical iOS device needs the runner signed with your Apple developer team. A free Apple ID works, and the signing expires every 7 days. phonedriver needs a Mac with Xcode.

I tested an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0) through the CLI and MCP. Screenshots, snapshots and the diff after a tap meet the targets I set. A bare tap does not: it takes 454 to 534 ms at p50 against a target of 80 ms. The 12-scenario suite was not run on devices.

## Set it up

Plug the device in and unlock it, then:

```sh
phonedriver ios setup                    # pairing, Developer Mode, developer disk image
export PHONEDRIVER_TEAM=<your team ID>   # Xcode > Settings > Accounts, or: security find-identity -v -p codesigning
phonedriver --device ios:<udid> snapshot
```

Without a team, the command stops with exit code 3 and says so. The daemon reads `PHONEDRIVER_TEAM` when it starts, so run `phonedriver daemon stop` after setting it.

Mounting the developer disk image is the one thing phonedriver does that talks to Apple. Nothing else leaves your machine.

## Without the cable

After the setup, with the device still on the cable:

```sh
phonedriver ios wifi on
```

Unplug it. While the iPhone and the Mac are on the same Wi-Fi network, `phonedriver devices` still lists it and every verb works. `phonedriver doctor` says `over Wi-Fi` or `over USB`. The iPhone answers only computers it already trusts, and `phonedriver ios wifi off` stops it.

I tested this on an iPhone 13 Pro Max (iOS 27.0.1) on one network. The app on the phone logged the taps and the typed text with no cable attached. It needs iOS 26.4 or later.

## What differs from Android

- There is no shell on a physical iPhone.
- Settle is detected by polling screenshots, not a video stream, so each action costs a little more.
- iOS 27 devices have an experimental CoreDevice tier, off by default: `PHONEDRIVER_IOS_TIER=coredevice`. It is not the supported path.

## What stays with you

Unlocking, trusting the computer and Developer Mode prompts are yours. The error says what to do on the device and the command line exits with code 3.

## Safety

The [approval guard](/docs/approval-guard/) is on by default. Read its limits before you give an agent a phone with accounts on it.
