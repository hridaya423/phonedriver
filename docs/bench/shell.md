# `shell` against raw `adb shell` (2026-11-06)

Setup: Pixel_API_26 emulator (`emulator-5556`, `-gpu swangle_indirect`) on win-pc, reached from the MacBook through the adb tunnel (`ssh -L 5038:127.0.0.1:5037 win-pc`). Release build of `phone-use` with the new agent op. The daemon and agent were already running (first call starts them). 50 runs after 5 warm-ups, p50 / p90 / min in ms; script `bench.py` run from the session scratchpad, not kept. `phone-use shell --no-settle` is compared with `adb -s emulator-5556 shell` started as a process per call, both over the same tunnel.

| command | raw `adb shell` | `phone-use shell` (CLI process + daemon + agent) | agent round trip only (daemon-measured) |
|---|---|---|---|
| `echo hi` | 130 / 218 / 113 | 59 / 142 / 52 | 23 / 31 / 16 |
| `settings get system screen_brightness` | 152 / 229 / 131 | 85 / 146 / 68 | 38 / 104 / 28 |

An MCP client skips the CLI process, so it pays the last column plus the daemon hop. The difference is the transport setup `adb shell` repeats on every call (connect to the server, select the transport, open a shell service, spawn `sh`); the agent's control stream is already open. Caveat: the tunnel to the adb server adds its own latency to both sides, and a loop run during other agents' emulator work had a few failed `adb` invocations (exit 255), which count as short samples for raw adb.

Settle costs extra only for commands that can change state: `cmd uimode night yes` returned in 260 ms with the screen part (`settled in 260ms · new screen`, Settings re-created by the theme change); read-only commands skip it.
