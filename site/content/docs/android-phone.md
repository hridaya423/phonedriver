---
title: How to control a real Android phone with phonedriver
nav: Real Android phone
meta: Turn on USB or Wi-Fi debugging, connect an Android phone and let an agent operate it with phonedriver. What has and has not been tested.
description: Turn on USB debugging or Wi-Fi debugging, connect an Android phone or tablet, and let an agent operate it with phonedriver. Notes on what has and has not been tested.
---

A real Android phone or tablet uses the same code path as the emulator. We have tested it on emulators only. No physical Android phone is in our benchmarks, and the iOS devices we did test are covered on [the iPhone page](/docs/iphone/). Treat real Android hardware as likely to work and not yet proven.

## Over USB

1. On the phone, turn on Developer options, then USB debugging.
2. Plug the phone in and accept the "Allow USB debugging" prompt.
3. Check phonedriver sees it:

```sh
phonedriver devices
phonedriver doctor
```

The phone appears as `android:<serial>`. Nothing is installed on it: the daemon pushes a small agent over adb and starts it as the shell user. No accessibility service has to be switched on.

## Over Wi-Fi

An Android 11 or later phone on the same network needs no cable. On the phone, turn on Developer options, then Wireless debugging, and choose Pair device with pairing code. Then:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phonedriver devices
```

The first address and the six-digit code are on the pairing screen. The second address is on the Wireless debugging screen, and its port changes each time wireless debugging is switched on. The phone then appears as `android:192.168.1.20:41233`. If it shows `offline`, `phonedriver doctor` says what to check. We tested this pairing flow between an emulator and a host over TCP, not on a real network.

## What stays with you

Unlocking the phone, a USB debugging prompt and a biometric prompt are yours. When one blocks the agent, the error says what to do on the device and the command line exits with code 3. The agent tells you and continues when you say it is done.

## Safety on a real phone

A physical Android device refuses wipes and removing apps phonedriver did not install, unless the command passes `--force`. The [approval guard](/docs/approval-guard/) is on by default, which matters more on a phone with real accounts than on an emulator. Do not hand an agent a phone with a payment app signed in until you have read what the guard does not catch.
