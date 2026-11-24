# Reference

Moved out of the README. Commands use the current binary name, `phonedriver`.

## Install

One line gets you the prebuilt binary. Each artifact has the Android agent inside.

The shortest route is `npx`. The `phonedriver` npm package is a small launcher: on first run it downloads the binary for your platform from GitHub Releases, checks its SHA-256, caches it and runs it. It contacts GitHub only for that download and sends no telemetry.

```sh
npx -y phonedriver init                                   # add it to every agent on this machine, then check the devices
npx -y phonedriver --version                              # try it
claude mcp add phonedriver -- npx -y phonedriver mcp      # or add it to Claude Code by hand
```

`init` looks for five agents and prints one line for each:

```console
$ npx -y phonedriver init
Claude Code  added, for every project
Codex        already there
Cursor       added to /Users/you/.cursor/mcp.json
VS Code      not found
Gemini CLI   not found

Restart your agent so that it loads phonedriver, then ask it: Open Settings and turn on dark mode.
```

The doctor checks follow (`--register-only` leaves them out, `--dry-run` changes nothing). Claude Code and Codex are asked through their own `mcp add` (Claude Code with `--scope user`). Cursor (`~/.cursor/mcp.json`), VS Code (the user profile's `mcp.json`) and Gemini CLI (`~/.gemini/settings.json`) get the entry in their file; the file's other keys are kept, and its keys come back in alphabetical order. A file that is not plain JSON, or that already has another `phonedriver` entry, is not touched: the line says why and prints the entry to paste. Started through `npx`, the entry is `npx -y phonedriver mcp` (`cmd /c npx ...` on Windows); started as an installed binary, it is `phonedriver mcp`, or the binary's full path when that name is not on `PATH`.

The installers below put a permanent `phonedriver` on your `PATH` instead.

```sh
# macOS and Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hridaya423/phonedriver/releases/latest/download/phonedriver-installer.sh | sh

# Homebrew (macOS and Linux)
brew install hridaya423/tap/phonedriver
```

```powershell
# Windows
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hridaya423/phonedriver/releases/latest/download/phonedriver-installer.ps1 | iex"
```

The shell and PowerShell installers put `phonedriver` in `$CARGO_HOME/bin` (`~/.cargo/bin`) and add it to your `PATH`. The iOS Simulator runner needs no download: its sources are inside the binary, and the first command that needs it builds it on your Mac with Xcode.

To build from source instead, use Rust 1.99 or later.

```sh
cargo build --release
```

The build packs the Android agent, so it needs the Android SDK with build-tools **37.0.0** and platform **android-37.0** (`ANDROID_HOME`, falling back to `ANDROID_SDK_ROOT`, then `/opt/homebrew/share/android-commandlinetools`), `d8` and JDK 17. Override the versions with `ANDROID_BUILD_TOOLS` and `ANDROID_PLATFORM`. If anything is missing the build says exactly what and how to get it. To skip that step, set `PHONEDRIVER_AGENT_JAR` to a built `phonedriver-agent.jar` (`bash android-agent/build.sh` makes one; none is published yet).

**iOS only, no Android toolchain:**

```sh
cargo build --release -p phonedriver --no-default-features
```

That binary has no Android support (`phonedriver doctor` skips the Android checks). Run the tests with the default features. The binary is `target/release/phonedriver`.

```console
$ target/release/phonedriver version
phonedriver 0.1.7
```

To use a binary you built instead of the npm package, register it with your client by its full path: `claude mcp add phonedriver -- "$PWD/target/release/phonedriver" mcp`.

Run `phonedriver doctor`. It checks adb, each Android device, the agent, UiAutomation, the video stream, Xcode, the simulators, and the permissions of the daemon socket. Each line says `ok`, `warn` or `fail`, and a fix follows every line that is not `ok`. Android allows one UiAutomation client, so while phonedriver's agent runs, `adb shell uiautomator dump` and other UiAutomation tools are killed (exit 137); the `uiautomation` line says so, and `phonedriver daemon stop` frees it. Without `--device` it checks the host and lists every device, and it probes (starts the agent or the runner on) only emulators and Simulators that no live session holds; a physical phone or tablet, and any device another live session holds, is listed with one `not probed` line that says why and gives `phonedriver doctor --device <id>`. `--all-devices` probes physical devices too, still skipping devices another live session holds. `--device <id>` probes that device and nothing else, even if a session holds it. Devices the [device list](#devices-an-agent-must-not-touch) hides are not shown. `phonedriver init` runs the default form.

```console
$ phonedriver doctor
ok    adb server       0029 at 127.0.0.1:5037
warn  android devices  none connected
      fix: Connect a phone with USB debugging on, or start an emulator; `adb devices` lists what adb sees.
ok    xcode            Xcode 27.0, Build version 27A266a
warn  simulators       none booted
      fix: `phonedriver boot` lists the ones that can be started; `phonedriver boot <name>` starts one.
ok    daemon socket    /Users/you/.phonedriver/run/daemon.sock mode 600 in a directory of mode 700
```

On a machine with a phone attached, the phone's line is `warn  agent android:R5CT123  not probed: a physical device, which doctor leaves alone unless asked`, followed by `fix: Run `phonedriver doctor --device android:R5CT123` to probe it.`

The CLI runs the same loop. `phonedriver skill` prints [`SKILL.md`](../SKILL.md), the instructions an agent reads before it uses the tools.

```sh
phonedriver devices
phonedriver snapshot
phonedriver tap "Dark theme"
phonedriver wait settled
```

### Start a simulator or an emulator

With nothing running, phonedriver starts an iOS simulator (Xcode installed) or an Android virtual device (one made in Android Studio):

```console
$ phonedriver boot
ID                                        MODEL      OS
sim:E02FE8A6-5F1E-405E-A263-2E828218BE5F  iPhone 17  iOS 26.5
$ phonedriver boot "iphone 17"
booted sim:E02FE8A6-5F1E-405E-A263-2E828218BE5F · iPhone 17 · iOS 26.5
```

The name is the model in any case, or the id; when several simulators have the name, the one with the newest system starts. It boots without a window (`open -a Simulator` shows it). Over MCP it is `devices` with `boot: "iPhone 17"`. When nothing is connected, every command's error names what can be started (`no device is connected; can be started: iPhone 17 (iOS 26.5)`), so an agent can start one and go on. Decision D77.

Android virtual devices are in the same list, and start the same way:

```console
$ phonedriver boot
ID                       MODEL                OS
avd:Pixel_API_37         Pixel_API_37         Android API 37
avd:Pixel_Tablet_API_37  Pixel_Tablet_API_37  Android API 37
avd:Pixel_API_31         Pixel_API_31         Android API 31
avd:Pixel_API_26         Pixel_API_26         Android API 26
$ phonedriver boot pixel_api_37
booted android:emulator-5554 · Pixel_API_37 · Android API 37
```

phonedriver runs the SDK's `emulator` program (from `ANDROID_HOME`, `ANDROID_SDK_ROOT`, the folder Android Studio installs to, or the `PATH`) and returns when Android says it has booted. The emulator keeps running after phonedriver's daemon stops; `adb -s emulator-5554 emu kill` stops it. A first boot can take minutes, and on a slow or busy machine the first command after it may still fail while Android settles; a later boot resumes from the emulator's saved state in seconds. With `ADB_SERVER_SOCKET` set (adb on another machine), no virtual device is listed. Decision D79.

### iPhone or iPad

A physical iOS device needs the runner signed with your Apple developer team (a free Apple ID works; its signature lasts 7 days, and the next command after that signs the runner again by itself). Plug the device in, unlock it, then:

```sh
phonedriver ios setup                  # signing team, pairing, Developer Mode, developer disk image, then the runner
phonedriver --device ios:<udid> snapshot
```

phonedriver reads the team from this Mac's Apple Development certificate. To make one, sign in under Xcode > Settings > Accounts, choose Manage Certificates and add an Apple Development certificate. With certificates for several teams, set `PHONEDRIVER_TEAM` to the ID of the one to use. The ID is the certificate's organizational unit. It is not the ID in parentheses that `security find-identity` prints after your name.

Without a team, the command stops with exit code 3 and says so. If the device has never been used with that team, the command stops with exit code 3 and asks for `phonedriver ios register`, which lets Xcode add the device to the team's Apple developer account (an account holds 100 iPhones a year). With a booted Simulator and an iPhone that has no team, the Simulator is the default device. If you set `PHONEDRIVER_TEAM`, run `phonedriver daemon stop` afterwards: the daemon reads it when it starts.

#### iPhone without the cable

After the setup above, with the device still on the cable:

```sh
phonedriver ios wifi on
```

Unplug it. While the device and the Mac are on the same Wi-Fi network, `phonedriver devices` still lists it and every verb works. `phonedriver doctor` says `over Wi-Fi` or `over USB`. The device answers only computers it already trusts. `phonedriver ios wifi off` stops it.

On a Mac over Wi-Fi, what phonedriver sends to the runner on the phone and what the runner sends back, screens and typed text included (`--secret` text too), crosses the network unencrypted. Only phonedriver can drive the runner, because it must prove the token the runner was started with (decision D106), but anyone who can read the network's traffic can read that stream. Use the cable for secrets, or a network you trust. Off a Mac, the runner's traffic goes through the Wi-Fi pairing's encrypted tunnel, and phonedriver checks that the device on the network is the one it paired with on the cable. A pairing made before 0.1.7 cannot be checked: plug the phone in once and run `phonedriver ios wifi on` again.

Tested on an iPhone 13 Pro Max (iOS 27.0.1) on one network. An iPhone 14 Plus (iOS 26.1) was found and answered over Wi-Fi; taps on it were not run. A sleeping device stops answering on Wi-Fi until it wakes. phonedriver does not unlock an iPhone. From 0.1.4 it keeps an iPhone awake for 10 minutes after each use and stops on `phonedriver release`; in 0.1.3, set Settings > Display & Brightness > Auto-Lock to Never for a long task. The experimental CoreDevice tier needs the cable.  See decision D47.

## Environment variables

| Name | Default | Meaning |
|---|---|---|
| `PHONEDRIVER_HOME` | `~/.phonedriver` | Where the daemon keeps its socket (`run/`), `daemon.log`, `traces/`, `tmp/` and the iOS `runner/` builds. Set it to run a separate daemon. On macOS the socket path must stay under about 100 bytes. |
| `PHONEDRIVER_SOCKET` | under `PHONEDRIVER_HOME` (else `$XDG_RUNTIME_DIR/phonedriver`) | The daemon's socket or named pipe. Point it at an `ssh -L` forward to use a remote machine's devices. |
| `PHONEDRIVER_DEVICE` | none | The device to use when `--device` is not given. Also read by `phonedriver mcp`. `<id>+<name>` is a lane of an Android device ([several agents on one phone](#several-agents-on-one-android-phone)). |
| `PHONEDRIVER_DISPLAY` | `main` | `hidden` makes the Android agent work on a second screen the person holding the phone cannot see (Android 13 and later; `--display hidden` does the same). Read by the CLI, `phonedriver mcp` and `doctor`. A call in the other mode reconnects, which takes a few seconds. Tested on an emulator and a HONOR X8 (Android 13); some makers' builds may refuse it. The phone has to be unlocked: on a locked phone the call fails with "The device is locked" and the phone stays asleep. While the agent works, the phone's own screen is kept on, so the phone does not lock under it (D86). |
| `PHONEDRIVER_SESSION` | `default` | The session name, as `--session`. |
| `PHONEDRIVER_TEAM` | the team of this Mac's Apple Development certificate | Apple developer team ID that signs the iOS runner on a physical device. Needed only with certificates for several teams. |
| `PHONEDRIVER_IOS_TIER` | `runner` | `coredevice` selects the CoreDevice tier on iOS 27 devices. Experimental. |
| `PHONEDRIVER_GUARD` | on | `off` stops phonedriver asking the human before a tap that spends money or cannot be undone (`Place order`, `Pay`, `Delete account`), and lifts every check on a physical device as `force` does: `shell` runs `input`, call intents, wipes and deletes from the storage root, `setting` writes what the device asks its user to confirm, and `open` takes a link that places a call. Set it for `phonedriver mcp` or the CLI only when you grant the agent full autonomy. |
| `PHONEDRIVER_ASK` | none | A comma list of more kinds of tap to ask the human about, besides payments and account deletion: `send` (Send, Reply, Post, Share), `delete` (Delete, Remove, Erase, Clear all) and `install` (Install, Uninstall). For example `PHONEDRIVER_ASK=send,delete`. Read by `phonedriver mcp` and the CLI; a name that is not one of these stops the command with exit code 2. `PHONEDRIVER_GUARD=off` and `autonomous: true` skip these questions too. |
| `PHONEDRIVER_ALLOW_DEVICES` | none | A comma list of devices the daemon may use, for example `android:emulator-5554,android:R5CT123`. Every other device behaves as if it were not connected. See [Devices an agent must not touch](#devices-an-agent-must-not-touch). The daemon reads it when it starts: run `phonedriver daemon stop` first if one is running. |
| `PHONEDRIVER_DENY_DEVICES` | none | A comma list of devices the daemon never lists, connects to or pushes the agent to, even with `--force`. Same entries and same rules as the allow list; a device named here is hidden even when the allow list names it too. |
| `PHONEDRIVER_APPS_UNDER_TEST` | none | A comma list of package names, such as `com.example.app,com.example.other`: apps of yours that Gradle or Android Studio installed, not phonedriver. On a physical Android phone `apps clear`, `uninstall`, `grant`, `revoke` and `launch --grant` touch only apps phonedriver installed; the packages listed here count as installed by you. The daemon reads it when it starts, so set it in the environment of the command that starts it (for an MCP client, the server's `env`; see [Claude Code](/docs/claude-code/#an-app-you-are-developing)) and run `phonedriver daemon stop` first if a daemon is running. It is the person's setting and there is no tool argument for it, so an agent cannot add an app to the list. |
| `PHONEDRIVER_APP_SCOPE` | none | A comma list of package names, such as `com.example.app`: the only apps the agent may read or act in (see [App scope](#app-scope)). Not the same as `PHONEDRIVER_APPS_UNDER_TEST`, which says which apps phonedriver may clear or uninstall on a physical phone and restricts nothing else. The daemon reads it when it starts; a client that sets a scope the running daemon was not started with is refused until `phonedriver daemon stop`. There is no tool argument for it, so an agent cannot set or clear it. |
| `PHONEDRIVER_HOST_SHELL` | off | `1` lets `shell` act on this computer as well as the device: on an iOS Simulator it runs any program (without it only `defaults` and `log show|stream`, with no file named) and the `simctl` forms that read or write a Mac file, on an Android emulator it sends any console command. Read when the daemon starts; see [Shell](#shell) and decision D108. |
| `PHONEDRIVER_SECRET` | none | The text for `type --secret`, instead of stdin. The daemon shows it as `•••` wherever it later appears on a screen it reads or in a command's output, until the daemon stops; a screenshot still shows what the phone shows. |
| `PHONEDRIVER_REPLAY_SECRET_ANY_FIELD` | off | `1` lets `replay` type a secret it reads from `PHONEDRIVER_SECRET_*` into any field. Without it a replayed secret goes only into a password or one-time code field (D105). Read by the process that replays: the CLI, or the MCP server. |
| `PHONEDRIVER_TRACE_FULL` | off | `1` keeps observation text and the text of text targets in the traced requests (the daemon reads it at start). Replies are traced as returned either way: a summary or an error can name an element, as in `tapped @e6 "Busy"` or `no element matches "Pay"`. |
| `PHONEDRIVER_TRACE_SCREENS` | off | `1` copies every screenshot next to its trace file. |
| `PHONEDRIVER_AGENT_JAR` | built by `android-agent/build.sh` | Build time only: use this prebuilt Android agent jar. |
| `ADB_SERVER_SOCKET` | `tcp:127.0.0.1:5037` | The adb server to use, for example a tunnel to another machine. |
| `PHONEDRIVER_TEST_DEVICE`, `PHONEDRIVER_TEST_IOS_UDID`, `PHONEDRIVER_BENCH`, `PHONEDRIVER_BIN` | none | Tests and benchmarks only. |

The runner's own `PHONEDRIVER_PORT`, `PHONEDRIVER_BUILD` and `PHONEDRIVER_UDID` are set by phonedriver for the iOS runner and are not for you.

### Devices an agent must not touch

On a shared adb server (a rack of test phones, a staff phone left charging) the fence is a device list the daemon enforces. A device the list does not allow, or denies, behaves as if it were not connected: `devices` does not list it, `doctor` does not probe it, every call that names it (`--device`, `PHONEDRIVER_DEVICE`, `devices {use}`, a replay or the GitHub Action) gets the same "no device" error as for a typo, `--force` does not change that, and the agent is never pushed to it.

Entries are the ids `phonedriver devices` shows (`android:emulator-5554`, `android:192.168.1.20:5555`, `ios:<udid>`, `sim:<udid>`), with or without the prefix, or an Android phone's hardware serial (`ro.serialno`, the number in `adb devices` over USB), so one rule hits the same phone over USB and over Wi-Fi. (Emulators from one system image share a serial: name an emulator by its id.) The serial is read from the phone only when an entry could name it and the id did not already settle the question: one read-only `getprop`, and nothing is pushed. A device that a deny entry names by id is not asked anything, not even its Android version. A read that fails or comes back empty hides the phone from a deny list and does not admit it to an allow list, and it is asked again on the next call.

A Simulator the list hides is not offered by `boot` and is never started. An AVD gets its id only once it runs, so before a boot only `deny avd:<name>` hides it; once it runs, it is judged by its id like any other device. When a source cannot be asked (no Xcode, no adb server), the "no device" error lists only the devices the list shows. When the daemon cannot say what the list hides (the file has an error), `doctor --device` probes nothing.

Three places set the list, and they combine as follows: a device is hidden when any deny entry from any place names it, and, for each place that has an allow list, when that list does not name it. So a machine-wide file's allow list cannot be widened by an MCP client's `env`.

- `PHONEDRIVER_ALLOW_DEVICES` and `PHONEDRIVER_DENY_DEVICES` in the environment that starts the daemon (for an MCP client, the server's `env`).
- The file `devices` in the phonedriver home (`~/.phonedriver/devices`, or under `PHONEDRIVER_HOME`), for a setting that holds for every agent on the machine. It is read again whenever it changes, so adding a `deny` line takes effect on the next call.

```
# the lab rack
allow android:emulator-5554
allow android:R5CT123
deny  android:0123456789ABCDEF   # Maria's phone
```

A line that is not `allow <id>` or `deny <id>` makes every device unavailable and the error names the file and the line: a list that cannot be read must not leave a phone open. `phonedriver devices` on a terminal ends with `1 device hidden by the device list`; the MCP `devices` reply says nothing about hidden devices.

The list covers what goes through the daemon. `pair`, `untether` and `ios setup`, `ios wifi` and `ios register` are commands a person runs on a device by name, and they talk to the device without it. The list is the person's setting, as with `PHONEDRIVER_APPS_UNDER_TEST` (D100): an agent with a shell on the computer can start its own daemon without it.

## App scope

`PHONEDRIVER_APP_SCOPE=com.example.app,com.example.other` limits the daemon to those apps, so a phone that is signed in to real accounts can be handed to an agent that tests one app (decision D102). The daemon enforces it under every verb; it is the person's setting in the environment the daemon starts with (for an MCP client, the server's `env`), and no tool argument sets or clears it. It differs from `PHONEDRIVER_APPS_UNDER_TEST`: that variable lets phonedriver clear or uninstall apps Gradle installed, and restricts nothing the agent can read or tap. Use both for an app under test on a real phone.

**In scope without being listed:** the system's permission, installer, VPN consent and crash dialogs that phonedriver already recognises as alerts, the keyboard window, and the status and navigation bars of the system UI (`com.android.systemui`; never read, so no notification text). Another app's window shaped like a bar is outside the scope.

**A screen with no app in the scope.** A tree that shows no window of an app in the scope (an empty tree, which is also what a dark screen or a tree read too early looks like) counts as outside: input and screenshots are refused until a tree names the app.

**Reads.** `snapshot`, `find`, `wait`, `page`, OCR, `compare` and the replies of actions show nothing from a window of an app outside the scope. The header says `app <package> (outside the app scope)` and lists no elements. The home screen is such an app: it lists apps and may carry widgets, so its content is never shown, while `press home` still works. `screenshot` is refused while any window of another app is on screen (a notification shade, a share sheet, a split-screen app, a picture-in-picture window): the picture cannot be cut by window, so it is refused, not blanked. The check runs before and after the capture. Traces, saved steps and `replay --screens` come from these replies and screenshots, so they hold nothing else.

**Actions.** `tap`, `type`, `clear`, `swipe`, `scroll`, `gesture`, `drag`, `pinch`, `press` keys, `clipboard set`, batch steps and `then` paths are refused while a window of an app outside the scope is on screen. `press back`, `press home`, `press lock` and `launch` of an app in the scope are allowed, so the agent can get back. `press recents` and `press notifications` are refused. `launch` and `open` of an app outside the scope are refused. `open <url>` needs `--app <package>` with a package in the scope, because a link can open any app. An `intent:` URL that names a component or a selector is refused, since Android starts that activity whatever app is named, and the agent refuses an `open` whose link would open in another app. `clipboard get` is refused, since the clipboard may hold what was copied in another app.

**Pages.** `page` reads the web page of the app in the scope that the check found on screen, not whatever app is in front a moment later.

**Apps, logs, network, settings.** `apps` lists only apps in the scope, and `stop`, `clear`, `uninstall`, `grant`, `revoke` and `notify` on any other app are refused as if it did not exist. `logs` and `wait log` read only an app in the scope; `wait log` without `--app` uses the scope's only package and is refused when the scope names several. On the Simulator the log is filtered by the app's own container folder, so another app whose bundle has the same name is not read. `network` is allowed (it shows device state, not app content), with the owner of a VPN outside the scope shown as "an app outside the app scope". `setting` is allowed: it reads and writes typed system settings, not app content. `apps install` is allowed; it adds or updates an app, and the app cannot be used unless it is in the scope.

**Refused whole.** `shell` is refused: `am start`, `input` and `content query` go around every rule above. `record` and `snapshot --capture` are refused: a video and the raw tree hold every window.

**iPhone and iPad.** The runner reports the front app and SpringBoard alerts only, so phonedriver cannot see a notification banner or another app's window over the front app. Reads and actions use the front app's bundle id the same way. `screenshot`, and therefore OCR, `compare` of the screen and `replay --screens`, are refused under a scope. A device whose front app is unknown (the CoreDevice tier has no tree) counts as outside the scope. `open` is refused under a scope on an iPhone, iPad or Simulator, because iOS opens a link in whatever app claims it; `launch` opens the app. `press back` and hiding the keyboard tap the front app's own buttons on iOS, so they are refused while an app outside the scope is in front; `press home` leaves it.

**Known gaps.** The scope narrows what phonedriver reads and does; it is not a sandbox, and these are open:

- The package of an app outside the scope that is in front is named in the header and in refusals. The status bar's notification icons are in a screenshot, so it shows which apps have notifications.
- The keyboard is in scope: its suggestion strip can show text from the clipboard or a password manager, and a screenshot takes it.
- A window that appears after the check and before the input or the capture is not caught for input; a screenshot is checked again after the capture.
- Windows are told apart by the package that owns them. A system dialog is let through when phonedriver recognises it; the share sheet is withheld by its package or resource ids (`android:id/resolver_list`, `com.android.intentresolver`), so a maker's variant with other ids would be let through as a dialog. A window thinner than 5 percent of the screen on one side is taken for an edge strip.
- An app in the scope can hand off to another (a link, a share). The handoff itself runs; what follows is hidden and refused until `press back` or `press home`.
- `logs` of an app in the scope are the app's own and can hold anything it logs. `setting` and `network` are device-wide.
- An agent with another way to the phone (its own adb, another MCP server) is not stopped.

## Debugging

When something does not work:

- `phonedriver doctor` checks the toolchain, devices and the socket, and says how to fix each problem.
- `$PHONEDRIVER_HOME/daemon.log` (default `~/.phonedriver/daemon.log`) holds the daemon's own messages and startup errors; past 4 MiB it is moved to `daemon.log.1` when the next daemon starts.
- `$PHONEDRIVER_HOME/traces/<id>.jsonl` has one line per request: method, parameters, the reply's summary, `ms` and settle time. Typed text is recorded only as a length, and so is the text of a text target. The reply's summary and an error's message are recorded as returned, so they can name elements from the screen (`tapped @e6 "Busy"`, `no element matches "Pay €49.99"`, an approval refusal's `On screen:` lines): read trace lines before you share them. Start the daemon with `PHONEDRIVER_TRACE_FULL=1` to keep observation text and text targets, and `PHONEDRIVER_TRACE_SCREENS=1` to keep screenshots. A running daemon keeps its options: `phonedriver daemon stop` first.
- iOS: `$PHONEDRIVER_HOME/runner/<udid>.log` is the runner's output, and `runner/<hash>/build.log` is its first build.
- `phonedriver daemon stop` stops the daemon and the iOS runners it started. The next command starts a new one.

Exit codes: `0` done, `1` the command failed, `2` the command line is wrong, `3` a person must act on the device (unlock, trust, allow), `4` no matching device or it was disconnected, `5` an action that spends money or cannot be undone needs the human's approval (add `--autonomous` only when the user has granted full autonomy). On a terminal, a first command that takes more than a second says what it is doing (building the runner, starting the agent).

## Device on another machine, Wi-Fi and shell

### Use a device on another machine

The daemon listens on a Unix socket and nowhere else. To drive a simulator or phone that is plugged into another machine, forward that socket over `ssh` and point the CLI at the local end:

```sh
ssh -N -L /tmp/phonedriver-mini.sock:/Users/you/.phonedriver/run/daemon.sock mini &
export PHONEDRIVER_SOCKET=/tmp/phonedriver-mini.sock
phonedriver snapshot
```

On the other machine, run any `phonedriver` command once, or `phonedriver daemon`, so that the socket exists. Both machines need the same `phonedriver` version. If the forward is down, the error names `PHONEDRIVER_SOCKET`.

Two things differ from a local run. `phonedriver screenshot` prints a path on the machine that has the device; `phonedriver screenshot shot.png` (or `-o shot.png`) writes the file on your machine instead. The MCP `screenshot` tool returns the image itself, so it works unchanged.

### Several agents on one Android phone

Add `+` and a short name to a device id and the session works on a lane: a hidden screen of its own on that phone (Android 13 and later). Each lane has its own lease, so several agents use one phone or emulator at the same time, and the phone's own screen stays free:

```sh
PHONEDRIVER_SESSION=one phonedriver --device emulator-5554+a launch settings
PHONEDRIVER_SESSION=two phonedriver --device emulator-5554+b launch clock
phonedriver devices
```

```console
ID                       MODEL                OS                   TIER   LEASE          DISPLAY
android:emulator-5554    sdk_gphone64_x86_64  Android 17 (API 37)  agent  -              main
android:emulator-5554+a  sdk_gphone64_x86_64  Android 17 (API 37)  agent  one (idle 3s)  hidden (display 13)
android:emulator-5554+b  sdk_gphone64_x86_64  Android 17 (API 37)  agent  two (idle 3s)  hidden (display 12)
```

For MCP, give each agent its own lane in its config: `"env": { "PHONEDRIVER_DEVICE": "emulator-5554+a" }`. With one device connected, `+a` is enough. The name is 1 to 8 lowercase letters or digits. A lane closes, and the apps on it with it, when its session releases it (`phonedriver release`) or has made no call for ten minutes.

On an emulator a lane's video is two pixels a point at 15 frames a second, because an emulator's encoder is software and falls behind at full size; screenshots and text read from pixels are the same as on the main screen, and a recording of a lane is that size.

An app runs in one place at a time. If two lanes launch the same app, it moves to the second one and the first is left on its home screen, so give each lane a different app. The clipboard is one for the phone, so lanes take turns to paste. Tried on one emulator (API 37) with two lanes beside the main screen; not on a physical phone.

### Android over Wi-Fi

An Android 11 or later phone on the same network needs no cable. On the phone, turn on Developer options, then Wireless debugging. `phonedriver pair` shows a QR code in the terminal (the flow Android Studio uses):

```sh
phonedriver pair
```

On the phone choose Wireless debugging, then Pair device with QR code, and scan it. phonedriver finds the phone on the network, pairs, connects, and prints `connected: android:192.168.1.20:41233`. It waits 2 minutes for the scan. Exit code 3 means nobody scanned; 1 means adb's mDNS is unavailable or pairing failed, and the message says what to do. Tested on a HONOR X8 (Android 13).

If the QR does not work (the phone and computer are on different networks, a guest network, or a router that blocks mDNS), pair with the code instead. On the phone choose Pair device with pairing code. Then:

```sh
adb pair 192.168.1.20:37099
adb connect 192.168.1.20:41233
phonedriver devices
```

The first address and the six-digit code are on the pairing screen; the second address is on the Wireless debugging screen (the port changes each time wireless debugging is switched on). The phone then appears as `android:192.168.1.20:41233`. If it shows `offline`, `phonedriver doctor` says what to check.

#### Away from your Wi-Fi

Wireless debugging works only on the Wi-Fi it was switched on for. To use a phone from anywhere you can reach it, for example over [Tailscale](https://tailscale.com), switch the phone to classic adb over TCP while it is connected by cable or already over Wi-Fi:

```sh
phonedriver untether
```

It lists the phone's addresses (`wlan0 (Wi-Fi)`, `tun0 (Tailscale)`), stops if the phone is on mobile data only, runs `adb tcpip 5555`, connects to the first address that answers and prints `connected: android:<ip>:5555`. With more than one device pass `--device android:<serial>`. Tested on a HONOR X8 (Android 13). On a Mac, an adb server that macOS keeps off the local network is restarted by both commands. A phone connected in more than one way (cable, Wi-Fi, port 5555) is one line in `phonedriver devices`; the cable wins, and any of its ids still works with `--device`.

This is opt-in on purpose. adb then listens on port 5555 on every network the phone joins, until it reboots. A new computer still has to be allowed on the phone's screen, but do this only on networks you trust, or with Tailscale on both ends and the phone's other networks untrusted. `adb -s <ip>:5555 usb` or a reboot turns it off. See decision D30.

### Settings by key

On Android 17, the Settings app answers the shell user's `cmd app_function` calls, and phonedriver uses them so an agent can change a setting without opening its screen:

```
phonedriver setting find dark theme                          # keys, what each accepts, and screens
phonedriver setting get dark_ui_mode/dark_ui_activated       # dark_ui_mode/dark_ui_activated = false
phonedriver setting set dark_ui_mode/dark_ui_activated true  # ok dark_ui_mode/dark_ui_activated = true
phonedriver setting find "dark theme|media volume"           # several searches in one call
phonedriver setting set <key> <value> <key> <value>          # several writes in one call
phonedriver setting open dark_ui_mode                        # shows that settings screen
```

The API 37 emulator lists 182 settings on 196 screens; 105 can be written. A write answers with the value the device reports afterwards, and a refusal carries the device's reason. On a physical device, a setting the device marks as needing its user's confirmation (NFC, mobile data, the camera and microphone switches) answers `unsupported` without `--force`. Older Android, iOS and the Simulator answer that the device has no settings by key: use `shell` there. Decision D57.

### Logs and crashes

On Android, `phonedriver logs` prints the last 40 log lines of the app in front (`logs <app>` for another, `--grep "error|warn"`, `--head 0` for all, `--since 300` to read five minutes back) and, first, its last crash. An action during which the app in front crashed carries the crash in its own reply:

```
ok tapped @e6 "Busy" · new screen
alert android: "phonedriver testbed keeps stopping"
...
crashed: dev.phonedriver.testbed stopped during this action: android.app.RemoteServiceException$CrashedByAdbException: shell-induced crash at android.app.ActivityThread.throwRemoteServiceException(ActivityThread.java:2581)
hint: `phonedriver logs dev.phonedriver.testbed` prints its last log lines
```

iOS devices and the Simulator answer that logs cannot be read. Decision D58.

### Reading a whole list

`scroll down --collect` scrolls a list a page at a time and prints every row it passed, each once and in the list's order:

```
$ phonedriver scroll down --collect
ok collected 22 rows in 3 scrolls down: the list ended · new screen
  button "Google" "Services & preferences"
  button "Network & internet" "Mobile, Wi‑Fi, hotspot"
  …
  button "Tips & support" "Help articles, phone & chat"
screen now:
…
```

A row is its snapshot line without the `@ref`, followed by the lines of what it holds (`button "Dark theme" "Will turn on when Bedtime starts", switch "Dark theme" off`). `--collect 60` stops at 60 rows (100 when no number is given); it also stops where the list ends or after `--max` scrolls (30), and the first line says which. `scroll up --collect` gathers upwards and still prints top to bottom. Over MCP it is `scroll` with `collect: 60`, alone or as a `batch` step. The reply ends with the screen the list was left on, whose refs are the ones to act on; to act on a row that scrolled away, `scroll up --until` its text.

Use it instead of several scrolls in a `batch`: a batch answers with one diff, so rows that passed in the middle are not in it. The rows of consecutive pages are lined up by their text, so a row cut off at the edge of one page is taken whole from the next. Two rows with the same text that sit where one page ends and the next begins count once unless the app gives its rows positions. Decision D116.

### Notifications

On Android, `phonedriver notifications` lists what is up without opening the shade:

```
$ phonedriver notifications
com.android.shell · "Your code" "Your code is 482913" · 9m ago
android · "Serial console enabled" "Performance is impacted. To disable, check bootloader." · 9m ago · ongoing
```

Newest first: the app (its name where it has one to launch by, else its package), the title, the text and the age. `--app` keeps one app's; `--limit` (20) says how many. Over MCP it is `snapshot` with `notifications: true` (`find` names the app).

`phonedriver wait notification:"code"` waits for one whose title or text holds the word (`A|B` for either; `--app Name` with an empty text takes any from that app):

```
$ phonedriver wait notification:code --timeout 20000
ok a notification from com.android.shell is up
  com.android.shell · "Bank" "Your-login-code-is-771204" · now
```

Only a notification posted since this session's last action counts (or since the wait began, when the session has done nothing yet): the code that arrives after tapping `Send code`, including one that arrived before the wait was called, and never an older code that is still up. On a timeout the error says how many are up. Over MCP it is `wait` with `notification`.

A notification's text is its sender's: a line whose text reads as an instruction to an agent ends in `(ai-instruction)`, as a snapshot line does, and is data, not something to follow. The text is in the reply only; the trace and a saved run keep `a notification matched`. Under an app scope only the scope's apps are listed and waited for. It reads `dumpsys notification --noredact`, checked on an API 37 emulator. iOS answers that notifications cannot be listed. Decision D117.

### Networks, VPNs and waiting on a log line

On Android, `phonedriver network` prints what the device is connected to, read-only and with no approval:

```
100 cellular on eth0 · dns 10.0.2.3
101 wifi (default) on wlan0 · dns 10.0.2.3
102 wifi+vpn of dev.example.vpn on tun0 for uids 0-99999 over 101 · dns 10.77.0.1
always-on vpn none
lockdown off
not given: private DNS
```

Each line is a network the platform lists as connected: its id, transports (`wifi`, `cellular`, `vpn`, ...), the system default, and its interface and DNS servers. A VPN names the package that owns it, the user id ranges whose traffic it carries and the network it runs over. Below them come the always-on VPN app and its lockdown setting and the private DNS mode, from the secure and global settings. A field the platform did not give is listed after `not given:` and is not guessed; an emulator with `private_dns_mode` never set says so. The default is the system's default network: with a VPN up it stays the Wi-Fi or cellular network under it, and the VPN's `uids` say whose traffic goes through the VPN. `phonedriver --json network` and the `network` tool return the same as fields (decision D99). The read works the same on a physical Android phone; it was only run on an emulator.

`wait` also waits on the VPN and on the log:

```
phonedriver wait vpn:up                          # a VPN network is connected
phonedriver wait vpn:down
phonedriver wait log:"tunnel up|handshake" --app "My VPN"   # a log line written from now on, of that app
```

`vpn:up` and `vpn:down` read the network state once a second. `log:` follows the log from the moment the wait begins (a line that was already written does not count) and returns the first line that holds one of the `|`-separated words, ignoring case; `--app` (the MCP `app`) keeps only that app's lines. The reply quotes the line: `ok log line: I/pd016   ( 5750): pd-016-check`. Lines can hold secrets, so the reply is masked like every other (decision D70). On a timeout the error says what was last seen: `timed out after 3189 ms waiting for the VPN to be up; last seen: cellular, wifi (default)`, or `waiting for a log line with "x" (164 lines arrived, none matched)`. Matching is on the whole line, so a pattern that also appears in a command line `adbd` logs (the `shell` that wrote the line) matches that first. Android follows `logcat`; the iOS Simulator follows `log stream` at debug level, run the way `shell` runs programs, with `--app` scoped to the app's process; a physical iPhone or iPad answers that its log cannot be followed. On iOS `network` and `wait vpn:` answer that the device's networks cannot be read.

A VPN app asks for consent the first time it starts a connection: a "Connection request" dialog from `com.android.vpndialogs`, which phonedriver reports like any system dialog (`alert com.android.vpndialogs: "Connection request"`) and `tap OK` answers. On an emulator you can skip it: `phonedriver shell "appops set <package> ACTIVATE_VPN allow"` pre-authorises the app, so the dialog never appears (checked on an API 37 emulator: with `default` the dialog came up, after `allow` the VPN connected with no dialog). `appops set <package> ACTIVATE_VPN default` takes it back.

### Screens with no element list

A web view with accessibility off, a Flutter screen or a game gives no list of elements. phonedriver then reads the screenshot's text on the machine the daemon runs on and adds the lines it finds, marked `(ocr)`; `tap "Send"` taps one. `snapshot --pixels` asks for this on any screen. macOS and Windows use the text recognition the system ships. Linux uses Tesseract when the `tesseract` program is on the `PATH` (`apt install tesseract-ocr`); without it, `--pixels` answers that pixels cannot be read and names the package. Decisions D61, D64 and D65.

### What changed between two screenshots

On a screen with no element list, looking again costs a second picture. `compare` answers in text:

```console
$ phonedriver screenshot before.png
$ phonedriver tap "Auto-Capitalisation"
$ phonedriver compare before.png
changed: 1 region, 0.6% of the screen
region 304,592 64x40 (no text changed)
$ phonedriver tap "Auto-Capitalisation"
$ phonedriver compare before.png
same
```

That is a switch that moved and moved back. Where text differs, the reply has a line for each text that changed (`~ "Will turn on when Bedtime starts" → "Will never turn off automatically" at 132,545`), appeared (`+ "General" at 203,86`) or went (`- "Settings" at 84,143`), with the point to tap. At most twelve lines, then `and 14 more`. With one file the screen now is compared with it; with two files nothing is asked of a device. `--image out.png` also writes the later picture with the regions boxed. Over MCP, `screenshot` with `compare: true` compares the screen with the last screenshot that call returned for the same device and sends no image.

The text is read from the pictures by the same recognition as above, so it can misread small or stylized type. A time in the top tenth of the picture that changed (the status bar's clock) is left out; other status bar text that changed is reported like any other. Both pictures must be the same size; take them without `--native` so that the points can be tapped. Decisions D76 and D89.

### Accessibility problems on a screen

`snapshot --audit` adds the screen's accessibility problems after its lines:

```console
$ phonedriver snapshot --audit
...
audit: 2 findings
@e15 searchbox "Search": 336x38pt, under 44x44pt
@e16 button "Dictate": 17x22pt, under 44x44pt
```

Two checks, from the element list alone: a control (a button, field, switch, tab or anything that can be tapped or typed into) with no label, hint or text inside it, which a screen reader has nothing to say for (`@e1 button: no name a screen reader can say`); and a tap target smaller than 44 by 44 points on iOS or 48 by 48 on Android, the size Apple's and Google's guidelines ask for. A target cut off by the screen's edge or by the list around it, and a link, are not measured. At most 20 lines, then `and 3 more`. Over MCP, `snapshot` takes `audit: true`. Decision D90.

### Reading a web page

On Android, `phonedriver page` prints the text of the web page in front, whole, without scrolling:

```
$ phonedriver page --grep "born|died"
page "Ada Lovelace - Wikipedia" https://en.wikipedia.org/wiki/Ada_Lovelace
Born
Augusta Ada Byron
10 December 1815
Died
27 November 1852 (aged 36)
...
```

It is the text a person can read on the page, in reading order, with blank lines dropped. A long page is cut at a line end before 8,000 characters and the reply says how much is left; `--all` prints everything. `--grep "a|b"` keeps the lines with one of the words and the two lines after each, because a page often puts a label and its value on separate lines. In MCP it is `snapshot` with `page: true`, with `find` for the words; MCP has no `all`, so a long page is narrowed with `find`.

It works for Chrome and for a web view inside an app that is a debug build (or that turned web debugging on), because it reads through the debugging socket those open. Any other app answers that it has no page that can be read. It only reads: taps and typing still go through the screen, so the approval guard sees them. iOS has no equivalent yet. Decision D71.

### Reading a value

`phonedriver check "Total"` reads what the screen shows for a label and touches nothing:

```
$ phonedriver check "Network & internet"
ok "Network & internet" = "Mobile, Wi‑Fi, hotspot"
$ phonedriver check "Dark theme"
ok "Dark theme" = "off"
$ phonedriver check "Brightness level" --equals "0%"
error: "Brightness level" shows "83%", not "0%"
```

The value is, in this order: `on` or `off` for a switch or checkbox; a field's text; the second text on the element's own line (a settings row's summary); the rest of a label that holds the text and more (`Total: €24.99`); the text on the same row to its right, or the plain text right under it. An element that is there with nothing to read answers `"Order placed" is shown`. A text that names a row and the switch inside it reads the switch. A password field is never read. `--equals` compares without regard to case or runs of spaces and fails with exit 1 when the screen says anything else. `--as total` adds the name to the line, for a script that reads several.

Its use is inside a `batch`, where it saves a model turn and stops a flow that went wrong before the next tap:

```
phonedriver batch 'tap "Checkout"' 'check "Total" --equals "€24.99"' 'tap "Pay"'
```

Over MCP it is a `batch` step: `{"tool":"check","target":"Total","equals":"€24.99"}`. A check is saved by `save` and replayed, so a saved run can prove a value and not only that a text is somewhere on the screen. A value that looks like an instruction to an agent is marked `(ai-instruction)`, as a snapshot line is. Decision D113.

### When a tap finds nothing

A `tap`, `type`, `scroll` or `check` whose target is not on the screen, or whose `@ref` is gone, answers with what the screen shows now, as lines against the screen the agent last saw, so the next call can use a ref from the error itself:

```
$ phonedriver tap "Dark theme"
error: no element matches "Dark theme"
hint: Use what the screen below shows.
screen now, against what you last saw:
app com.google.android.apps.nexuslauncher
@e33 text "Gmail"
…
```

When nothing changed the hint says so and no screen is repeated. A ref the session never saw still answers that refs are per session. Decision D114.

### Saving and replaying a run

`phonedriver save run.steps` writes what the session has done so far to a file, and `phonedriver replay run.steps` does it again with no model:

```
$ phonedriver save run.steps
saved 7 steps to run.steps
1. launch Settings
2. tap "Network & internet"
...
the file holds one step a line: delete the line of a step that was a detour
$ phonedriver replay run.steps
replayed 7 steps in 18.1 s
1. restarted Settings
2. tapped @e4 "Network & internet"
...
screen now:
...
```

Only calls that worked are saved: actions, waits, shell commands, app installs and permission changes, and settings written by key. A `scroll --until` that failed after moving the list is saved as the plain scrolls that moved it, since the step after it may need the list there. An element is saved by its text and role, not by its `@ref`, so the file works in a later session. Where no text singles the element out it is saved by position, and `save` says how many such steps there are. `save` lists every step it wrote, so a step that was a wrong turn can be seen and its line deleted from the file. After `save` the session's list starts again empty, so save once before a flow (to a file you throw away) and once after it. A text field the keyboard pushed off the screen is saved by its label. `save` also names each typed value that no later step reads back: to check what the app kept, not only what the field showed, end the file with a `check` of its label that `--equals` it, or a `wait` for it, on a screen that shows it. A phone number matches however the app formats it (`wait "0655555552"` finds `065-555-5552`, not `+1 065-555-5552`).

`save` keeps the build of each app a step launches (`"build": "4.37.1 (437001)"`), and a replay whose app is now another build stops before its first step and names both; `--any-build` (MCP `any_build: true`) runs it anyway (D109). A physical iPhone reports no build, so its replays are not checked. Each action step also keeps the app it was done in and whether a dialog was in front (`"on": {"app": "com.android.contacts", "over": false}`), and a replay stops before touching anything when the app in front is another or a dialog is in front where there was none, or none where there was one (D110).

A replay stops at the first step that fails, that changes nothing where it changed the screen when it was saved, that typed text its field does not confirm (a field that shows `+10655555552` for `0655555552`, or nothing on screen to read), or that is a `shell` command with an exit status other than the step expects. A saved `type --submit` is replayed as the typing, then `press enter` once the field shows the text, so a wrong value is never submitted. The first line says how many typed texts were verified (`--json`: `verified`). It prints the steps that ran, the screen, and the step to go on from:

```
error: step 3 of 7 (tap "Intranet") failed: no element matches "Intranet"
1. restarted Settings
2. tapped @e4 "Network & internet"
screen now:
...
hint: Do step 3 yourself, then replay the same file from step 4 for the 4 steps left.
$ phonedriver tap "Internet"
$ phonedriver replay run.steps --from 4
```

#### Named paths of an app

A run saved as `paths/<app id>/<name>.steps`, in the folder you run phonedriver in (over MCP, the server's folder), is a path of that app: a way to a screen that an agent takes in one call. The app id is the one the screen's `app` line shows.

```
$ phonedriver save paths/com.android.settings/internet.steps
saved 4 steps to paths/com.android.settings/internet.steps
1. launch Settings
2. tap "Network & internet"
3. tap "Internet"
4. wait "Internet"
the file holds one step a line: delete the line of a step that was a detour
it is a path: the last step waits for "Internet", the screen this run ended on, and `launch` names the path. Edit that line if another text tells the screen better.
$ phonedriver replay paths/com.android.settings/internet.steps
arrived at "Internet" · 4 steps in 8.5 s
app com.android.settings (Settings)
@e77 group scrollable↑
  @e78 group "Internet"
...
$ phonedriver launch Settings
ok launched Settings · new screen
...
paths: internet · replay paths/com.android.settings/<name>.steps
```

The last line of the file is the landmark, what must be on the screen on arrival:

```
{"landmark":true,"method":"wait","params":{"condition":{"appears":{"role":"other","text":"Internet"}},"timeout_ms":5000}}
```

`save` takes it from the screen the run ended on: the first heading, else the first label that is not a control, and never what a field holds. It is a guess at the screen's title, so read it once. Change `text` to the label that tells the screen apart (a whole label; `A|B` is either), drop `role` to accept any kind of element (`other` is what a snapshot prints as `group`), and raise `timeout_ms` for a page that loads slowly. A path that ends anywhere else fails with the step, the steps that ran and the screen, and taps nothing after:

```
error: the path did not arrive at "Internet": step 3 of 3 (wait "Internet") failed: the screen holds it only inside "Network & internet"
1. restarted Settings
2. tapped @e4 "Network & internet"
screen now:
...
hint: Every step before it ran, and nothing was tapped after it. Go on from the screen above; if the app changed, save the path again, or edit the last line of paths/com.android.settings/internet.steps to what the screen shows now.
```

A path is a steps file in every other way: blanks (`{{query:pizza}}`, shown by `launch` as `search(query)`), secrets as environment variables, the build check, the screen check before each step, `--from`. To see an app's paths, list the folder; to remove one, delete its file. Delete the landmark line and the file is a plain run again. `save` refuses a folder named after an app the run was never in, and a screen with no text to tell it by. Decision D118.

A `shell` step expects exit 0, and a command that runs past its time limit fails the step. `save` keeps `"expect_exit": 3` in the step of a command that exited 3 when it was saved, so a replay of the same run passes; add `"expect_exit": 1` by hand to a step that checks for a failure, or `"expect_exit": null` to a step that is only run for its effect and whose exit status does not matter. A step is saved without `--force`, so a replay on a physical phone is refused again at a command that needs it.

Text typed with `--secret` is never in the file. `save` writes the step with the name of an environment variable instead, numbered in order, and says which to set:

```
$ phonedriver save login.steps
saved 3 steps to login.steps
1. tap "Sign in"
2. type secret $PHONEDRIVER_SECRET_1 into "Password"
3. tap "Continue"
the secret text is not in the file: set PHONEDRIVER_SECRET_1 before replaying (rename a variable in the file if you like)
$ PHONEDRIVER_SECRET_1=hunter2-test phonedriver replay login.steps
```

The line in the file is `{"method":"act","params":{"action":{"kind":"type","secret":true,"secret_env":"PHONEDRIVER_SECRET_1","into":{...}}}}`. `replay` reads the variable from the environment of the process that runs it (the `phonedriver` command, or for MCP the MCP server, so set it in the server's `env`, never in a tool argument), and types it as a secret: masked in replies and traces, never in an error, a log or a file (D70). The name may be changed in the file, but it must begin `PHONEDRIVER_SECRET_`, so an edited file cannot make a replay type another variable of the process. The daemon types a replayed secret only into a field it sees as a password or one-time code field, and refuses the step before typing otherwise; `PHONEDRIVER_REPLAY_SECRET_ANY_FIELD=1` where the replay runs allows any field (D105). When the variable is not set (or is empty) the replay stops at that step as before, and the hint names it. A file saved by an earlier version, whose secret step has no variable, still stops there for you to type it. Other typed text is in the file.

#### Variables in a saved run

`{{name}}` in a step's text (what is typed, an `open` URL, a `shell` command, a target's text) is filled in when the file is replayed. `save` never writes one; edit them into the file:

```
$ phonedriver replay signup.steps --var plan=pro
replayed 6 steps in 14.2 s (run 3fa91c07)
1. typed into @e4
```

`--var name=value` is repeatable (MCP: `vars: {"name": "value"}`). `{{run}}` is built in: a short id, new in every replay and the same in all its steps, for data that must be fresh, such as `test+{{run}}@example.com`; the reply names the run when the file uses it. A name used in the steps that run but not given is refused before the first step, naming every one missing. A blank may hold an example after a colon, the wording the screen uses, which that message shows: `tap "{{day1:Tuesday}}"` is refused without a value as `needs 1 variable nobody gave: day1 (like "Tuesday")`, so an agent does not pass `Tue` to a button that says `Tuesday`. A name is letters, digits, `_`, `-` and `.`; anything else between braces is typed as it is, and a value is not read for names again, so `--var x='{{x}}'` types the braces. Variables are not secrets: they are in replies and failure messages. Use the environment variable above for a secret. In a `shell` step a value is shell text, so quote it in the file and give only values you trust. `secret_env` is never filled. A step other than an action runs on the replay's own device and lease: a `device` or `share` in the file is dropped. In MCP, `replay` takes `file`, `from`, `vars` and `save: true`; a relative `file` is in the server's working directory, `save` writes only a `.steps` file there, through no `..` or link, and over nothing but an earlier steps file; a replay reads any `.steps` file, there or elsewhere. Decisions D60, D98 and D105.

`--screens <dir>` checks what each step left on the screen. The first replay keeps a screenshot after every step in the folder (`01.png`, `02.png`, ...); later replays compare each step's screen with its screenshot, as `compare` does, and stop at the first step whose text changed:

```
$ phonedriver replay settings.steps --screens screens/settings
replayed 5 steps in 10.5 s
...
kept 5 screens in screens/settings; a replay with --screens screens/settings compares each step's screen with them
$ phonedriver replay settings.steps --screens screens/settings
error: step 5 of 5 (wait "Name") failed: its screen is not the one in screens/settings/05.png
changed: 2 regions, 35.7% of the screen
~ "General" → "About" at 201,85
+ "Name" at 59,161
...
hint: If the new screen is right, delete screens/settings/05.png and replay again to keep it.
```

Pixels that changed with no text in them (a pressed row, a scroll bar) do not stop a replay; the step's line says how many regions changed. The status bar is left out (its clock, notification icons, battery and signal): the Android agent and the iOS runner report how far down it reaches. A screen that differs is looked at again every quarter second until it has stayed the same for 1.5 s or 5 s have passed, so a switch still sliding or a picture the video encoder has not sharpened yet does not stop the replay; a screen about to be kept must first look the same twice. A screenshot of another size stops the replay, so keep the screens on the device type the replays run on. Text is read with the recognition of section 9, so on Linux it needs Tesseract. Decision D89.

#### Replaying runs in GitHub Actions

The repository is also a GitHub Action. It starts an Android emulator on a Linux runner or an iOS Simulator on a macOS runner, installs your app, replays each steps file, and fails the job at the first run that stops. Each run's reply goes in the job summary, and the reply, the screen and the app's logs of a run that stopped go in an artifact, with a video of every run when `record` is `true`:

```yaml
jobs:
  replay:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: ./gradlew assembleDebug
      - uses: hridaya423/phonedriver@v0.1.7
        with:
          app: app/build/outputs/apk/debug/app-debug.apk
          runs: e2e/*.steps
          record: true
```

Inputs: `runs` (paths or globs, one a line), `platform` (`android` or `ios`), `app`, `api-level` (default 35), `device` (a Simulator name; default the first iPhone), `record`, `vars`, `screens` (a folder with a subfolder of kept screens for each run, named as its steps file; a run with no subfolder keeps its screens in the artifact under `screens/`), `version` (default `latest`), `binary` (a phonedriver already on the runner) and `artifact` (the artifact's name). Outputs: `passed` and `failed`. Write the steps files with `phonedriver save` after a session on your own machine. A step that typed a secret names an environment variable (`PHONEDRIVER_SECRET_1`); give it to the Action step with `env:`, for example `PHONEDRIVER_SECRET_1: ${{ secrets.APP_PASSWORD }}`, and the replay types it. `vars` fills `{{name}}` in the files, one `name=value` a line; `{{run}}` needs no input.

### Fingerprint and Face ID on an emulator or Simulator

On an Android emulator, `phonedriver press fingerprint` touches the fingerprint sensor with the finger a test enrolls, and `phonedriver press wrong-fingerprint` with another. Enroll by opening fingerprint setup in Settings and pressing `fingerprint` until it says the fingerprint was added; after that the same key passes a fingerprint prompt or unlocks the emulator, and `wrong-fingerprint` fails it. When phonedriver is on another machine than the emulator (adb through `ADB_SERVER_SOCKET`), copy the file `.emulator_console_auth_token` from the home folder of whoever started the emulator into your home folder here: the emulator's console asks for it. A physical phone answers that its sensor takes its owner's finger. Decisions D63 and D68.

On an iOS Simulator the same two keys answer Face ID (or Touch ID): `press fingerprint` shows a face that matches, `press wrong-fingerprint` one that does not, and the system says `Face Not Recognised`. A Simulator with no face enrolled is enrolled by the first press, so when an app says Face ID is not set up, press `fingerprint` once and ask again. An iPhone answers that its Face ID takes its owner. Decision D69.

### Turning the screen

`phonedriver press landscape` turns the screen on its side and `phonedriver press portrait` turns it upright; the reply is the screen as it is afterwards.

```console
$ phonedriver press landscape
ok pressed landscape · new screen
device sim:E02FE8A6-5F1E-405E-A263-2E828218BE5F · iPhone 17 · iOS 26.5 · 874x402pt · landscape
```

An app that supports one orientation only stays as it is, and so does an iPhone's home screen. On Android both keys turn auto-rotate off (`accelerometer_rotation`), because Android ignores a set rotation while it is on; turn it on again in the quick settings or with `setting`. A lane keeps the shape it was made with. Decision D78.

### Recording the screen

On Android and on the iOS Simulator, `phonedriver record start` begins a video of the screen and `phonedriver record stop` finishes it and prints the file:

```
$ phonedriver record start -o run.mp4
recording the screen to /Users/me/run.mp4; `phonedriver record stop` ends it
$ phonedriver launch settings
$ phonedriver scroll down
$ phonedriver record stop
/Users/me/run.mp4
15.2 s, 1080x2424, 150 frames, 3.6 MB
steps: 2 with their times in /Users/me/run.mp4.steps.jsonl
```

The video is the H.264 stream the phone already sends, in an MP4 file, with no sound. It starts at the first full picture (a keyframe) the phone sends after `record start`, which phonedriver asks the phone's encoder for, so the file holds nothing that was on screen before the start. A screen that changed in the last second sends it within about 100 ms. A still screen needs none: the phone sends a full picture each time the screen stops changing, so a recording started on a still screen opens with that screen, and one that never changes shows it for its whole length. Only when an encoder ignores the request and pictures keep coming for a second does the file start from the last keyframe before the start, as releases before 0.1.7 always did, and then it can hold a few seconds of what was on screen before. The steps file has one line for each action that succeeded during the recording, such as `{"summary":"scrolled down","t_ms":11119}`: the summary line of the action's reply and when it began, in milliseconds on the video's clock. Text typed with `--secret` is not in it. Without `-o` the file goes under `~/.phonedriver/tmp`, where a starting daemon deletes files older than a day. In MCP, `screenshot` takes `record: "start"` and `record: "stop"`.

A turned screen ends the recording, and `record stop` says so. Stop a recording before you stop the daemon: a daemon that is killed leaves a file players cannot open. The Simulator records itself (`xcrun simctl io recordVideo`): it writes a frame only when the screen changes, so the video of a still screen has one frame, and a turned screen does not end it. An iPhone answers that recording it is not built. Decisions D59 and D66.

### Shell

`phonedriver shell "cmd uimode night yes"` runs a command on the device and prints the exit code, the output (cut at 40 lines; `--grep`, `--head`) and what changed on screen. On Android it runs in the agent already on the device, not in a new `adb shell`. On an iOS Simulator, `shell "simctl ui appearance dark"` fills in the device for `simctl` subcommands that act on the Simulator alone: `ui`, `openurl` (not a `file:` URL), `privacy`, `location`, `status_bar`, `launch`, `terminate`, `uninstall`, `listapps`, `appinfo`, `get_app_container`, `pbcopy`, `pbpaste`, `keychain reset` and `io enumerate`. The Simulator's own settings and log need nothing set either: `defaults read -g AppleLocale`, `defaults write com.example.app Flag -bool YES` and `log show --last 1m` run as one program in the Simulator, with no pipes or `;`, and no argument that names a file. A Simulator's programs run on the Mac as your Mac user and reach its files, so by default `shell` refuses anything else: another program, `simctl spawn`, and the forms that read or write a Mac file (`install`, `addmedia`, `push`, `io screenshot` and `recordVideo`, `keychain add-cert` and `add-root-cert`, `launch --stdout` and `--stderr`). The verbs do those jobs: `install`, `apps notify`, `screenshot`, `record`. To allow them, start the daemon with `PHONEDRIVER_HOST_SHELL=1` (run `phonedriver daemon stop` first if one is running). Then any command runs inside the Simulator through `simctl spawn`, through its shell where it has one; iOS 26 Simulators have no shell, so there a command is one of the Simulator's own programs with its arguments (`defaults read -g AppleLocale`, `log show --last 1m`, `launchctl list`, `notifyutil -p <name>`), with no pipes, `;`, redirection or `$`. On an Android emulator, a command that starts with `emu` goes to the emulator's console instead of the device's shell, as `adb emu` does: `shell "emu geo fix -122.41 37.77"` puts the emulator at that longitude and latitude, `emu sms send 5551234 hello` delivers a text message, `emu power capacity 15` sets the battery. The console takes `geo`, `sms`, `power`, `network speed`, `delay` and `status`, `sensor`, `rotate`, `finger` and `avd name` and `status`; the rest, which include commands that write files on the computer or end the emulator (`screenrecord`, `network capture`, `redir`, `kill`, `avd snapshot`), need `PHONEDRIVER_HOST_SHELL=1` too. A console command is one line: a line break in it is refused. A phone has no console. A physical iPhone has no shell. A physical device refuses wipes and removing apps phonedriver did not install unless you pass `--force`. See `docs/bench/history/shell.md` and decisions D21 and D108.

## Development

```sh
blaze build
BLAZE_TESTS=all blaze test
cargo clippy --all-targets -- -D warnings
```

Tests that need a device are ignored by default. Set `PHONEDRIVER_TEST_DEVICE` (for example `android:emulator-5554` or `sim:<udid>`) and pass `--ignored`. [`AGENTS.md`](AGENTS.md) holds the rules for working in this repository.

<details>
<summary>Brand</summary>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="design/logo/display-dark.svg">
    <img alt="The phonedriver wordmark: on the left the letters are orange and ink screen dots, which grow to the right until they become solid letters." src="design/logo/display.svg" width="100%">
  </picture>
</p>

The mark is an invisible chauffeur: a peaked cap floating over a phone and a white glove that taps it, with nobody in between. The glove is the pointing-hand cursor wearing a chauffeur's glove. The wordmark is lettering drawn for phonedriver; in its display form, shown here, it starts as screen dots and resolves into solid letters, because phonedriver reads a screen as text instead of screenshots. [`design/logo/`](design/logo) holds the figure, the icon, the lockup and the display wordmark; [`design/brand.md`](design/brand.md) has the rules; `cd site && node scripts/brand.ts` rebuilds every logo, banner and social image from `site/lib/chauffeur.ts` and `site/lib/brand.ts`. The README charts are built on the [Sodium](https://design.how/systems/sodium) design system (MIT, see [`NOTICE`](NOTICE)) by `python3 design/build/build.py`, with text converted to outlines.

</details>

