# Security

## Report a vulnerability

Report it in private through GitHub: open the [Security tab](https://github.com/hridaya423/phonedriver/security/advisories/new) of this repository and choose "Report a vulnerability". Do not open a public issue for it.

Say what you did, what happened, and which version (`phonedriver --version`), operating system and device you used.

## What counts

phonedriver runs on your computer and operates a phone that you connected to it. These are in scope:

- A way for a web page, an app or text on the phone's screen to make phonedriver run a command on the computer.
- A tap, swipe, key or other action that pays, orders, transfers or deletes an account and goes through without the approval prompt.
- Text typed with `--secret` that reaches a log, a trace, an error or a reply.
- A screenshot, recording or trace that another account on the computer can read, on macOS or Linux.
- A connection from phonedriver to anything except the device, Apple for iOS developer disk images, and the pinned disk image download that `phonedriver ios setup --download-ddi` makes when you ask for it.
- A release file that does not match its published checksum or its build attestation.

A model that is told to do something harmful and does it with the tools is not a phonedriver vulnerability, unless a guard above should have stopped it. Neither is what `shell` does on the computer after you started the daemon with `PHONEDRIVER_HOST_SHELL=1`, which allows exactly that.

## iPhones and iPads: what is protected

- Only the phonedriver that started the runner on the phone can drive it. It must prove the token the runner was started with; anyone else on the network who connects is refused, and phonedriver's own connection stays.
- On the cable, the runner's traffic goes through usbmuxd. Off a Mac on Wi-Fi it goes through the Wi-Fi pairing's encrypted tunnel, and phonedriver checks that the device on the network is the one it paired with on the cable before it sends anything.
- Not protected: on a Mac over Wi-Fi, the runner's traffic, typed text included, crosses the network unencrypted. Someone who can read the network can read the screens and what is typed, `--secret` text too. Use the cable for secrets.

## Supported versions

Only the latest release gets fixes.
