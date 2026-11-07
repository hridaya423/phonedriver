# Developer and user experience audit

Date: 2026-11-06. Commit audited: `e06054d` (main). The agent-experience audit is `docs/audits/ax-audit.md` and is not repeated here.

## Method

I played a newcomer who has only the README. I cloned the repo into a temp directory and followed the README on two machines.

- **MacBook** (Rust 1.99.1, Xcode 27, Android build-tools 37, no booted device, one paired physical iPhone that I did not touch): clone, `cargo build --release`, `doctor`, `devices`, error and empty states, `blaze build`.
- **Mac mini** (iOS 26.5 Simulator, Rust, Xcodegen, Android command-line tools with build-tools 35 and 36 only, no JDK on PATH, no adb): a copy of the clone, built in its own directory, with `PHONEDRIVER_HOME=~/dxh` so the daemon others use was not touched. Mine was killed afterwards.

Not done: win-pc emulators (busy), physical devices (not mine), any model call. So the Android agent push and the Android first-run progress are read from code, not timed. All commands ran with stdout/stderr captured; raw outputs are in `/private/tmp/claude-501/-Users-hridyaagrawal-Honey-phone-use/4cb1136a-fafb-4ccd-b147-78d0e944ff9f/scratchpad/dxux/out/`. Output below is verbatim unless marked `...`.

## The newcomer journey, timed

| # | Step (README says) | Machine | Time | Result |
|---|---|---|---|---|
| 1 | `git clone` | MacBook | 0.2 s | ok |
| 2 | Read README to "Try it": step 1 says "Build phonedriver (see Install)" then `claude mcp add` | - | - | the reader goes down to Install first |
| 3 | `cargo build --release` | MacBook | 67 s (cold target dir) | ok. Built without `ANDROID_HOME` set, because `android-agent/build.sh` defaults to a Homebrew path the README does not mention |
| 4 | `cargo build --release` | Mac mini | 19 s then **fail** | `build.sh: line 14: /opt/homebrew/share/android-commandlinetools/build-tools/37.0.0/aidl: No such file or directory`. The mini has build-tools 35 and 36. The error never names `ANDROID_HOME`, `ANDROID_BUILD_TOOLS` or the version it wanted |
| 5 | Workaround: `PHONEDRIVER_AGENT_JAR=<jar>` (README) | Mac mini | 15 s | ok, but a newcomer has no jar to point at: none is published, and the only source is another machine's build |
| 6 | `target/release/phonedriver version` | both | 0.3 s | `phonedriver 0.1.0`. `phonedriver --version` is rejected (`error: unexpected argument '--version' found`, tip: `--session`) |
| 7 | `phonedriver doctor` | Mac mini | 3 s first (runner build), 0.4 s on the MacBook | ok, readable. 6 ok/warn lines with fixes. The adb warning on an iOS-only Mac shows a raw errno |
| 8 | `phonedriver devices` | Mac mini | 0.15 s | clean table |
| 9 | First `snapshot` (wipes `PHONEDRIVER_HOME`: daemon start + iOS runner build) | Mac mini | 4.0 s, silent | works. No line says what the 4 seconds are. The second `snapshot` takes 0.17 s |
| 10 | `launch Settings`, `tap General`, `scroll down`, `find About` | Mac mini | 0.1 to 3 s each | works, and diffs read well. First tap from a cold launch reports `settled in 3000ms` (the timeout), with no hint |
| 11 | `claude mcp add phonedriver -- /path/to/phonedriver mcp` | - | not run (changes the user's Claude config) | I checked by hand that `phonedriver mcp` answers `initialize` and `tools/list` over stdio with 11 tools. The README gives no way to confirm the server connected, and no `PHONEDRIVER_HOME` or `--device` advice for the first run |
| 12 | `phonedriver skill` | MacBook | 0 s | prints SKILL.md. Nothing says where an agent should have it (Claude Code skills folder, `AGENTS.md`, or MCP `instructions` only) |
| 13 | iPhone path: `PHONEDRIVER_TEAM` | MacBook | - | `snapshot` with no `--device` picked the one paired iPhone and failed: `error: connection error: set PHONEDRIVER_TEAM to the Apple developer team that signs the runner (for example DV6NC6KRYL)` |
| 14 | `phonedriver ios setup` | Mac mini (no device) | 0 s | `no iOS device is connected over USB`, exit 1. Not run against the MacBook's iPhone (physical). README never mentions `ios setup` |
| 15 | Development: `blaze build` | MacBook | 2 m 32 s cold | ok. README does not say where blaze comes from; CI uses `cargo test` |

Time to first successful tap, from the clone: about 1 minute 30 seconds on a machine that matches the author's SDK layout, and not reachable without outside help on the Mac mini (step 4).

## Findings, ranked

Rank = how many people it stops or slows. Effort: S under an hour, M under a day, L more.

### Blockers

**1. The build fails on any machine without Android build-tools 37.0.0, and says so cryptically. (blocks every iOS-only user, and most Android users; M)**

Evidence, Mac mini, clean clone:

```
$ cargo build --release
error: failed to run custom build command for `phonedriver-android v0.1.0`
  --- stderr
  .../android-agent/build.sh: line 14: /opt/homebrew/share/android-commandlinetools/build-tools/37.0.0/aidl: No such file or directory
  thread 'main' panicked at crates/android/build.rs:57:9:
  android-agent/build.sh failed
```

The README says "needs the Android SDK (`ANDROID_HOME` with build-tools and platform 37), `d8`, and JDK 17". It does not say the version is pinned to exactly `37.0.0` (override `ANDROID_BUILD_TOOLS`) and `android-37.0` (`ANDROID_PLATFORM`), that the script silently falls back to `/opt/homebrew/share/android-commandlinetools`, or that a Mac mini with build-tools 35 and 36 fails. A build with a bad `ANDROID_HOME` prints the same bare bash error. Also: an iOS-only user must install an Android toolchain and JDK to use a Simulator.
Fix: in `build.sh`, check the tool paths up front and print `need build-tools 37.0.0 under $SDK (found: 35.0.0 36.0.0); set ANDROID_HOME or ANDROID_BUILD_TOOLS, or set PHONEDRIVER_AGENT_JAR` and which of `java`, `d8` is missing. Better and larger: allow any installed build-tools at or above 34 (the jar is `--min-api 26`), and attach a prebuilt `phonedriver-agent.jar` to releases and to a `--no-default-features`-style iOS-only build (L).

**2. `PHONEDRIVER_AGENT_JAR` is offered as the escape hatch but no jar exists to download. (blocks the same users; M, one step after finding 1)**

README: "To skip that step, set `PHONEDRIVER_AGENT_JAR` to a built `phonedriver-agent.jar`." The jar only comes out of `build.sh`. I had to scp it from the MacBook. Fix: cargo feature `android` on by default so `cargo build --release --no-default-features --features ios` builds an iOS-only binary with no SDK, JDK or d8 (M). Until then, delete the claim or say where to get the jar.

**3. A paired physical iPhone becomes the default device and the first command fails with advice that is not about the problem. (blocks every user who plugs in an iPhone and tries the README; S to M)**

```
$ phonedriver snapshot       # one paired iPhone, no simulator booted
error: connection error: set PHONEDRIVER_TEAM to the Apple developer team that signs the runner (for example DV6NC6KRYL)
hint: Run `phonedriver doctor` to check the connection.
exit=1
```

Problems: the hint points at `doctor`, but the action is "set `PHONEDRIVER_TEAM`"; no text says where a team id comes from (Xcode > Settings > Accounts, or `security find-identity -v -p codesigning`); `DV6NC6KRYL` looks like a real team id used as an example; "connection error" is wrong, nothing failed to connect; and the README mentions `PHONEDRIVER_TEAM` only inside the Supported devices table. `ios setup` is not in the README at all. Fix (S): drop the real id from the example, drop the `doctor` hint for this error, say how to find the team id. Fix (M): a README section "iPhone or iPad" with `ios setup`, `PHONEDRIVER_TEAM`, the weekly re-sign for free accounts, and the first-run time.

### High (slow most people)

**4. A wrong `--device` is reported as "no device is connected". (every typo and every stale id; S)**

```
$ phonedriver --device sim:BAD snapshot       # one simulator is booted
error: no device is connected
hint: Connect a device or start an emulator, then run `phonedriver devices`.
exit=4
$ phonedriver --device nonsense snapshot
error: no device is connected   (same)
```

Fix: when `--device` or `PHONEDRIVER_DEVICE` names an id that is not in the list, print `no device "sim:BAD"; connected: sim:280D26B7... (iPhone 17)` or `none`. Exit 4 is fine.

**5. A bad argument to `scroll` says the daemon is stale. (S)**

```
$ phonedriver scroll sideways
error: this daemon does not know the action `scroll`: it is older than the client, so stop it and run the command again to start a new one
exit=1
```

The daemon was started minutes earlier from the same binary and `scroll down` worked. The direction was invalid. A user will kill and restart the daemon in a loop. Fix: validate direction in the CLI (use a clap `ValueEnum`, which also lists the choices in `--help`) and make the daemon's unknown-action message name how to stop it (finding 6).

**6. There is no way to stop or restart the daemon, and killing it leaves the iOS runner running. (S to M)**

`phonedriver daemon --help` offers `--detach` only. Nothing in README or help says how to stop. The "older than the client" error says "stop it". I stopped mine with `pkill -f "...phonedriver daemon"` and found its `xcodebuild test-without-building` still alive (as `docs/architecture.md` section 8 predicts: "a daemon killed alone leaves `xcodebuild` and the runner running"). Fix: `phonedriver daemon stop` (S: signal the pid from the socket or a pidfile; M: also end runners it started). Also say the same words in the version-mismatch error.

**7. The first run of the iOS runner shows nothing for 4 seconds, and on a cold Xcode cache likely much longer. (every iOS user's first command; S)**

```
$ rm -rf $PHONEDRIVER_HOME; time phonedriver snapshot
...
phonedriver snapshot  ... 3.974 total          # no line on stderr while it waited
```

The build log is `~/.phonedriver/runner/<hash>/build.log`; nothing tells the user. 4 s is with warm Xcode module caches (the mini); a first build on a fresh Mac was not measured and is probably tens of seconds. The same applies to the Android agent push (read from code, not timed: no `eprintln` besides errors in `crates/cli/src`). Fix: on a TTY, when a step passes 1 s, print one stderr line, such as `building the iOS runner for iPhone 17 (first run only, log: <path>)` and `starting the Android agent on emulator-5554`. Keep stdout clean for agents.

**8. Env vars are mostly undocumented and have no list. (everyone who debugs; S)**

Code uses `PHONEDRIVER_HOME`, `SESSION`, `DEVICE`, `SOCKET`, `TEAM`, `AGENT_JAR`, `SECRET`, `TRACE_FULL`, `TRACE_SCREENS`, `IOS_TIER`, `PORT`, `BUILD`, `UDID`, `KEEP_SCREENSHOT`, `TEST_DEVICE`, `TEST_IOS_UDID`. The README names `AGENT_JAR`, `DEVICE`, `SOCKET`, `TEAM`, `TEST_DEVICE`. `--help` shows only those wired through clap. `PHONEDRIVER_HOME` (the one every automation needs; also limited by the `sun_path` length) appears only in `docs/architecture.md`. Fix: one table in the README (name, default, who needs it). Mark the internal ones (`BUILD`, `PORT`, `UDID`, `KEEP_SCREENSHOT`) as not public or remove.

**9. "How do I debug a failure" is nowhere in the README or help. (everyone with a problem; S)**

Traces (`$PHONEDRIVER_HOME/traces/<ulid>.jsonl`, `--trace-full`, `--trace-screens`), `daemon.log`, and `runner/<udid>.log` exist and are well designed, but are described only in `docs/architecture.md` line 266. A user whose tap does nothing does not know they exist. I read them: they work (`{"method":"snapshot","ms":3963,"ok":true,...,"settle_ms":250}`). Fix: a "When something goes wrong" README section with the four paths, the env vars to turn on full traces, and the exit codes.

**10. Exit codes are in SKILL.md only. (script writers; S)**

`client.rs` maps 3 (person must act), 4 (device missing or busy), 1, and clap gives 2. Only SKILL.md says "the command line exits with code 3". Not in `--help` or README. Fix: a short "Exit codes" block in README and `phonedriver --help` `after_help`.

### Medium

**11. The README's "Try it" order sends the reader to Install before it, and gives no check that MCP connected. (every MCP user; S)**

Step 1 is "Build phonedriver (see Install), then `claude mcp add ...`" with a `/path/to/phonedriver`. Fix: put Install first; give the add line with the real path (`$(pwd)/target/release/phonedriver`); tell users to run `claude mcp list` or `/mcp` and to expect `phonedriver ✓ connected`; add the `phonedriver` binary onto PATH (`cargo install --path crates/cli`, not verified by me) because the CLI examples later use bare `phonedriver`. Also say where the skill goes (a Claude Code skill folder, `AGENTS.md`, or "the MCP `instructions` already carry a short version").

**12. Help text is uneven. (everyone who reads `--help`; S to M)**

- Global flags (`--session`, `--json`, `--no-settle`, `--timeout`, `--expect-change`, `--share`) are repeated on every subcommand, including `devices`, `doctor`, `mcp`, `daemon` and `ios setup`, where they do nothing. The one `tap` option a user wants (`--role`) is buried in 12 lines.
- Flags with no description: `snapshot --diff --interactive --within --max-lines --fixture-dir`, `type --into --mode --submit --secret`, `scroll --until --max`; positional args `<TARGET>`, `<DIRECTION>`, `<CONDITION>` have none.
- Naming: `snapshot --within` and `scroll --in` both mean the element to restrict to (`--in <WITHIN>`), and the MCP `scroll` calls it `in`. `--device` takes "the device id or serial" while `devices` prints `ID`.
- `doctor --help` says "Check Xcode, booted simulators and their runners" but the README says it also checks adb, Android agent, UiAutomation, the video stream and the socket.
- `phonedriver` alone prints a different hand-written usage (names only, exit 0) than `--help`.
- No examples, no link to the docs, no shell completions.

Fix: `global = true` flags into a "Global options" heading, `hide = true` where irrelevant; fill each missing `help =`; rename to one flag (keep the old as an alias); `after_help` with three examples and the doc link. (Mostly S; the flag rename is M because SKILL.md, MCP and the bench scripts use `--in`.)

**13. Failure messages with the wrong noun. (every miss; S)**

```
$ phonedriver press bogus
error: no element matches "bogus"
hint: Use one of these instead: home; back; recents; ...
$ phonedriver type hi            # nothing focused
ok typed 2 chars · NOT verified · settled in 341ms · no element changed
hint: the field does not show the text; retry with `--mode keys`
exit=0
```

`press bogus` should say `unknown key "bogus"`. The second begins with `ok` and exits 0 on a failure (the AX audit covers the agent side; for a script writer the exit code is wrong). Fix: reword; and exit 1 (or a distinct code) when `verified` is false, or print `warn` rather than `ok`.

**14. The `type --secret x` error ends with an unrelated usage line. (S)**

```
error: with --secret the text must come from stdin or PHONEDRIVER_SECRET, not from the command line ...

Usage: phonedriver [OPTIONS] [COMMAND]
```

The usage line is the top-level one, not `type`'s. Fix: show `echo -n "$PW" | phonedriver type --secret --into @e3` in the error.

**15. `phonedriver --version` is rejected and `-V` too. (S)**

`error: unexpected argument '--version' found` with the tip `a similar argument exists: '--session'`. Everyone tries it first. Fix: clap's `version` attribute; keep the `version` subcommand.

**16. Remote setup is documented but fragile. (remote users; S)**

README's `ssh -L` socket forward works as documented in principle; I did not run it. Errors I did run are good: `PHONEDRIVER_SOCKET=/tmp/nope.sock phonedriver devices` says `(PHONEDRIVER_SOCKET=/tmp/nope.sock; if it is an ssh forward, check that the ssh process is still running)`. But a long `PHONEDRIVER_HOME` gives `local socket name length exceeds capacity of sun_path of sockaddr_un` (names the path but not the limit, about 104 bytes on macOS). Fix: say the limit in the message. Add the version-must-match hint to the "older than the client" message (finding 5). Also AGENTS.md's TCP forward for adb (`ADB_SERVER_SOCKET`) is not in the README.

**17. `devices` and `apps` readability. (S)**

- `apps` prints unaligned two-column names with internal bundles first (`EscrowSecurityAlert`, `Emoji`, `Color`, `CarPlay`). The first 14 lines contain no app a person would launch except Maps/Messages/Health. Fix: sort user-facing apps first or add `--all`; align columns.
- `clipboard` with an empty clipboard prints a blank line and exit 0.
- `launch Settings` after a cold start reports `settled in 3000ms · new screen`, which is the timeout value, not a measured settle; nothing says it timed out.
- Output has no colors or TTY detection anywhere (checked `doctor` under `script`: zero escape codes). That is a fine default for agents; for people, `ok`, `warn`, `fail` and the `+`/`-`/`~` diff would read faster with color on a TTY and `NO_COLOR` honored. Effort M, low priority.

### Low

**18. Contributor path is thin. (contributors only; S to M)**

No `CONTRIBUTING.md`. README's Development section says `blaze build` without saying what blaze is or where to get it (it is not in the Rust toolchain); CI runs `cargo fmt`, `clippy`, `cargo test --workspace`, which is a different path. `BLAZE_TESTS=all` is unexplained. The testbeds (`testbeds/android`, `testbeds/ios`) have no README: how to build them, which tests need them, and `PHONEDRIVER_TEST_DEVICE` are only partly described. `blaze build` cold took 2 m 32 s. Fix: a 15-line "Contributing" section: tools to install, the three commands, how to run one device test, and what the testbeds are.

**19. `docs/` has no index, and one doc is stale. (S)**

`docs/release-checklist.md` says "There is no LICENSE file, no git remote, and no CI", but `LICENSE` and `.github/workflows/ci.yml` exist and README says AGPL. `docs/` mixes design (`architecture.md`, `decisions.md`, `protocol.md`), plans, spikes, audits and bench, with no `docs/README.md`. Fix: five-line index; fix or delete the stale state list.

**20. Windows and Linux are not mentioned. (S)**

The README never says which host OS builds work. The video decoder crate links objc2 frameworks, and `cargo build` on Linux was not tried. Fix: one line in Install ("macOS builds and is tested; other hosts: not tested"), or check.

## Quick wins (all S, about 3 hours total, ordered by value per minute)

1. Name the missing tool in `android-agent/build.sh`: check `$BT/aidl`, `d8`, `javac` and print `ANDROID_HOME`, found build-tools, `ANDROID_BUILD_TOOLS`. (15 min, finding 1)
2. Document the `37.0.0` pin, the Homebrew default and the three override variables in README Install. (5 min, finding 1)
3. Wrong `--device` message: list the connected ids. (15 min, finding 4)
4. Validate `scroll` direction with a clap `ValueEnum`. (10 min, finding 5)
5. `PHONEDRIVER_TEAM` error: remove the real team id, remove the `doctor` hint, say where to find the id. (10 min, finding 3)
6. README "Try it" order and an MCP connected check (`claude mcp list`). (10 min, finding 11)
7. README "When something goes wrong": traces, `daemon.log`, runner log, exit codes. (15 min, findings 9, 10)
8. README env var table. (15 min, finding 8)
9. clap: `version`, fill every missing `help =`, rename `--in` or alias it to `--within`, correct `doctor`'s description. (30 min, findings 12, 15)
10. `press bogus` wording, `--secret` error example. (10 min, findings 13, 14)
11. A stderr line when a step takes over 1 second (runner build, agent push). (30 min, finding 7)
12. `docs/README.md` index and fixing `release-checklist.md`'s state paragraph. (10 min, finding 19)

## What already works well

- `doctor` is the best onboarding surface: one line per check, a concrete fix under each warning, and it ran in 0.4 s. It spotted the unbuilt runner and the paired iPhone.
- The daemon starts on demand, and `PHONEDRIVER_HOME` isolation worked first time (a second daemon next to another agent's, with no interference).
- Errors usually carry a hint: `tap "Nonexistent thing"`, `wait "Nope" --timeout 1500` (`timed out after 1593 ms waiting for "Nope" to appear / hint: Increase --timeout`), `launch Nonexistentapp` (`hint: Use phonedriver apps`).
- Remote errors name `PHONEDRIVER_SOCKET` and the ssh forward.
- `devices` and `--json` agree, and `devices` names the default device and lease owner.
- SKILL.md is short, accurate and matches the CLI verbs.
