---
title: What phonedriver sends over the network, and what it does not
nav: No telemetry
seo: Does phonedriver send telemetry? No, and a test proves it
meta: phonedriver sends zero telemetry. Every network connection it makes, the macOS sandbox test that kills any stray socket, and how other tools compare.
description: Zero telemetry, enforced by a sandbox test that SIGKILLs the process if it opens an unexpected socket. Every connection phonedriver makes—and how to verify it yourself.
updated: 2026-11-21
---

An agent driving your phone sees your messages, photos, and banking screens. Calling home with usage analytics in a tool like that feels wrong, so phonedriver sends **zero telemetry**—no install ID, no usage pings, no crash reports, no update checks. It isn't a toggle; it's a rule enforced in CI. After the first download of the binary it talks to your device and to nothing else, with the exceptions for iPhones listed below.

## Every connection phonedriver makes

| Connection | When | Leaves your machine? |
|---|---|---|
| Local daemon socket | Every command | No |
| Local `adb` server & Android device | Android commands | No (unless remote) |
| `adb` server to your router at port 9, where nothing answers | Only `phonedriver pair` and `untether` on macOS, to tell whether macOS keeps the adb server off the local network | No |
| `usbmuxd` & USB iPhone | Physical iOS | No |
| Local mDNS & iPhone LAN IP | After `phonedriver ios wifi on` | No |
| iOS Simulator | Simulator commands | No |
| Apple signing server | Mounting an iOS developer disk image on a physical iPhone: on a Mac, Xcode's `devicectl` asks; off a Mac, phonedriver sends the chip ID and nonces to `https://gs.apple.com/TSS/controller` over HTTPS and follows no redirect (plain HTTP before 0.1.7) | Yes (Apple) |
| GitHub, `doronz88/DeveloperDiskImage` at a pinned commit | Only `phonedriver ios setup --download-ddi`, once, to fetch the iPhone developer disk image off a Mac; each file is checked against a pinned SHA-256 | Yes (GitHub) |
| GitHub Releases | First `npx -y phonedriver` run of a new version only | Yes (GitHub) |

That is the entire list. The GitHub Releases fetch belongs to the `npx` launcher—it downloads the platform binary and `.sha256` checksum, verifies the hash, caches it, and never touches the network again. Install via Homebrew or a release tarball and even that download is gone.

## The test that keeps it honest

Promises are cheap; `SIGKILL` is louder. `crates/cli/tests/network.rs` executes the CLI, daemon, and MCP server inside macOS `sandbox-exec` with a profile that allows localhost and Unix sockets and kills the process instantly on any outside connection:

```console
(deny network-outbound (with send-signal SIGKILL))
(deny network-inbound (with send-signal SIGKILL))
(allow network-outbound (remote ip "localhost:*"))
(allow network-outbound (remote unix-socket))
```

A control test proves the profile actually kills an outbound socket; the session test fails if any phonedriver process dies by signal ([Decision D25]({{REPO}}/blob/main/docs/decisions.md)).

**What the test doesn't cover:** it runs on macOS only, uses a local fake `adb` server (so the Apple DDI mount is verified by code review), and macOS's profile doesn't intercept system-resolver DNS lookups. phonedriver connects to raw addresses rather than resolving hostnames, except for the `--download-ddi` fetch, which `curl` makes. That fetch runs only when you ask for it, so the test does not run it.

## What stays on disk (and what doesn't)

Logs, traces, screenshots, and recordings live under `$PHONEDRIVER_HOME` (`~/.phonedriver`) and are never uploaded. On macOS and Linux only your account can read them: folders are created 0700, files 0600. Text typed via `--secret` or MCP `secret: true` shows as `•••` in logs, traces, errors, and replies—even when the field reformats it (a card number in groups of four), when a long text is cut mid-secret, and in the text a screen comparison reads. A screenshot or recording still shows whatever the phone shows ([troubleshooting](/docs/troubleshooting/) lists what each file holds).

Keep in mind: screen text and screenshots returned by phonedriver still go to your AI client (Claude Code, Codex, Cursor), which sends them to your chosen model provider under its privacy terms.

## How other phone tools compare

| Tool | Telemetry |
|---|---|
| **phonedriver** | None |
| mobile-mcp | Anonymous usage telemetry via PostHog & Scarf, on by default (`MOBILEMCP_DISABLE_TELEMETRY=1` opts out) |
| phone-harness 0.3.3 | Sends scripts, outputs, and helper calls to PostHog by default (`DO_NOT_TRACK=1` opts out; undocumented in 0.3.3 README) |
| adb | None |

## Verify it yourself

On macOS, run an installed `phonedriver` binary under the exact same sandbox profile—any external network call kills it immediately:

```sh
cat > /tmp/local-only.sb <<'PROFILE'
(version 1)
(allow default)
(deny network-outbound (with send-signal SIGKILL))
(deny network-inbound (with send-signal SIGKILL))
(allow network-outbound (remote ip "localhost:*"))
(allow network-outbound (remote unix-socket))
(allow network-inbound (local unix-socket))
PROFILE
sandbox-exec -f /tmp/local-only.sb phonedriver snapshot
```

The same rule holds for this website: no cookies, no analytics, no third-party scripts.
