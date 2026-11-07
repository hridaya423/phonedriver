---
title: How to control a real iPhone or iPad with phonedriver
nav: Real iPhone
description: Pair an iPhone or iPad, set up Developer Mode and a signing team, and let an agent drive it with phonedriver. Tested on an iPhone 14 Plus and an iPad Pro.
---

A physical iOS device needs the runner signed with your Apple developer team. A free Apple ID works, and the signing expires every 7 days. phonedriver needs a Mac with Xcode.

We tested an iPhone 14 Plus (iOS 26.1) and an iPad Pro (iOS 27.0) through the CLI and MCP. Screenshot, snapshot and the settled tap diff are within budget. A tap misses its 80 ms budget, at 454 to 534 ms at p50. The 12-scenario suite was not run on devices.

## Set it up

Plug the device in and unlock it, then:

```sh
phonedriver ios setup                    # pairing, Developer Mode, developer disk image
export PHONEDRIVER_TEAM=<your team ID>   # Xcode > Settings > Accounts, or: security find-identity -v -p codesigning
phonedriver --device ios:<udid> snapshot
```

Without a team, the command stops with exit code 3 and says so. The daemon reads `PHONEDRIVER_TEAM` when it starts, so run `phonedriver daemon stop` after setting it.

Mounting the developer disk image is the one thing phonedriver does that talks to Apple. Nothing else leaves your machine.

## What differs from Android

- There is no shell on a physical iPhone.
- Settle is detected by polling screenshots, not a video stream, so each action costs a little more.
- iOS 27 devices have an experimental CoreDevice tier, off by default: `PHONEDRIVER_IOS_TIER=coredevice`. It is not the supported path.

## What stays with you

Unlocking, trusting the computer and Developer Mode prompts are yours. The error says what to do on the device and the command line exits with code 3.

## Safety

The [approval guard](/docs/approval-guard/) is on by default. Read its limits before you give an agent a phone with accounts on it.
