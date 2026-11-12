# Security

## Report a vulnerability

Report it in private through GitHub: open the [Security tab](https://github.com/hridaya423/phonedriver/security/advisories/new) of this repository and choose "Report a vulnerability". Do not open a public issue for it.

Say what you did, what happened, and which version (`phonedriver --version`), operating system and device you used.

## What counts

phonedriver runs on your computer and operates a phone that you connected to it. These are in scope:

- A way for a web page, an app or text on the phone's screen to make phonedriver run a command on the computer.
- A tap that pays, orders, transfers or deletes an account and goes through without the approval prompt.
- Text typed with `--secret` that reaches a log, a trace, an error or a reply.
- A connection from phonedriver to anything except the device and, for iOS developer disk images, Apple.
- A release file that does not match its published checksum.

A model that is told to do something harmful and does it with the tools is not a phonedriver vulnerability, unless a guard above should have stopped it.

## Supported versions

Only the latest release gets fixes.
