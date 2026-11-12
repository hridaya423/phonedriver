---
title: What phonedriver sends over the network, and what it does not
nav: No telemetry
seo: Does phonedriver send telemetry? No, and a test proves it
meta: phonedriver sends no telemetry. Every connection it makes, the sandbox test that fails if one is added, and how mobile-mcp and phone-harness differ.
description: phonedriver sends no telemetry and no usage data. This page lists every connection it makes, the test that fails if anyone adds another, where logs and traces are kept, and how to check it yourself.
updated: 2026-11-11
---

phonedriver sends no telemetry: no install id, no usage events, no crash reports. After the first download of the binary, it talks to your device and to nothing else, with one exception for iPhones that is listed below. An agent that drives your phone sees your messages, your photos and your bank's login screen, so this is a rule of the project and not a setting.

## Every connection phonedriver makes

| Connection | When | Leaves your machine |
|---|---|---|
| The local daemon socket | Every command | No |
| The local adb server and the Android device | Android devices | No, unless the device is remote |
| usbmuxd and the iPhone | Physical iPhones and iPads | No |
| Multicast DNS on the local network, and the iPhone's address | Only after `phonedriver ios wifi on`, to find and reach that iPhone | No |
| The iOS Simulator | Simulators | No |
| Apple's signing server | Mounting a developer disk image on a physical iPhone | Yes, to Apple |
| GitHub Releases | The first `npx -y phonedriver` run of a version, to download the binary | Yes, to GitHub |

That is the whole list. There is no update check, no analytics endpoint and no error upload.

The GitHub download belongs to the npm launcher, not to phonedriver. The launcher fetches the binary for your platform and its `.sha256` file, compares the two, caches the binary, and runs it. Later runs start the cached binary with no network. Install with Homebrew or from a release archive and the launcher is not involved.

## The test that keeps it true

A promise of no telemetry is only worth something if it breaks loudly when someone adds a request. `crates/cli/tests/network.rs` runs the `phonedriver` commands and `phonedriver mcp` through macOS `sandbox-exec` with a profile that allows connections to localhost and Unix sockets and kills the process on any other network operation:

```console
(deny network-outbound (with send-signal SIGKILL))
(deny network-inbound (with send-signal SIGKILL))
(allow network-outbound (remote ip "localhost:*"))
(allow network-outbound (remote unix-socket))
```

The daemon starts as a child of the command, so it is inside the sandbox too. A fake adb server on a local socket stands in for the device. A control test shows the profile does kill a connection to an outside address, and the session test fails if any process dies by signal. The reasoning is decision D25 in [the design decisions]({{REPO}}/blob/main/docs/decisions.md).

## What the test does not cover

- It runs on macOS only. Linux has no equivalent that keeps a local fake server reachable from inside the sandbox.
- The session in the test has no real device and no iOS disk image mount, so the Apple exception is checked by reading the code, not by the sandbox.
- A DNS lookup through the system resolver is not a connection and would not be caught. phonedriver resolves no names: adb and usbmuxd are addresses.

## What stays on your machine

Logs and traces are files under `$PHONEDRIVER_HOME` (default `~/.phonedriver`). They are never uploaded. Text typed with `--secret`, or `secret: true` over MCP, never reaches logs, traces, errors or replies, so a password or a one-time code is not written to disk at all. See [troubleshooting](/docs/troubleshooting/) for what the files hold.

The screen text and screenshots that phonedriver returns go to your agent, and your agent sends them to its model provider. That traffic is your MCP client's, and its privacy terms apply to it. What phonedriver adds on its side is [the approval guard](/docs/approval-guard/), which holds taps that spend money.

## How the other tools compare

| Tool | Telemetry |
|---|---|
| phonedriver | None |
| mobile-mcp | Anonymous usage telemetry through PostHog and Scarf, on by default. Opt out with `MOBILEMCP_DISABLE_TELEMETRY=1` (its README) |
| phone-harness 0.3.3 | Sends the script, its output and helper calls to PostHog by default. Opt out with `DO_NOT_TRACK=1`. Its README does not mention it |
| adb | None |

The phone-harness row is from reading `src/phone_harness/telemetry.py` at commit `4bebff1` on 2026-11-05, and the project may have changed since. More in [phonedriver vs mobile-mcp](/compare/mobile-mcp/) and [phonedriver vs phone-harness](/compare/phone-harness/).

## Check it yourself

On a Mac, run any command under the same profile. A process that opens an outside connection is killed and the command fails:

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

Use an installed binary for this, not `npx`, because the launcher's first download is an outside connection. On any system, the source is open under AGPL-3.0 at [the repository]({{REPO}}).

This website follows the same rule: it sets no cookies and loads no third-party scripts.
