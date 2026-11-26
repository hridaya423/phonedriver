# Changelog

Every release of phonedriver, newest first. Binaries and checksums for each version are on [GitHub Releases](https://github.com/hridaya423/phonedriver/releases).

## Unreleased

- **Agents chain a step further** (D134): the server's instructions now say that a `then` path may name steps on screens not seen yet, and that a wrong guess stops there with the screen and costs one call. In 9 agent runs with it, 12 steps rode on an earlier tap against 3 in 9 runs without, about one model turn fewer on a job that walks menus.
- **Fixed: `setting open` showed the first screen again** (Android 17, D132): once one settings screen had been opened, `setting open` of another left the first in front and still answered `ok opened`. Settings hands its own link to the page it already shows, and that page ignores it. Each screen now starts in a cleared task, so the one asked for is the one shown. Seen on the API 37 emulator: SIMs, then Battery, Internet and Dark theme, each stayed on SIMs before and each opens now.
- **`setting open` takes the name on the screen** (D132): `setting open "Battery"` opens the screen with that title, and a setting's key (`mobile_network_list/mobile_data`) opens the screen it sits on. A word that names no screen answers with the screens that hold it (`open one of: top_level_apps (Apps)`), so the next call is the right one and no `find` goes between.
- **For contributors**: `CONTRIBUTING.md` says how to build, what CI runs and what a pull request needs; issues have a bug form that asks for `phonedriver doctor`'s output and an idea form; pull requests have a template.
- **`launch` goes down a saved path** (#41, D131): `launch Settings --path internet` (MCP `launch` with `path`) opens the app, goes down `paths/<app id>/internet.steps` and answers `arrived at "Internet"` with the screen, in one call. The MCP server names the saved paths in its instructions, so an agent knows them before its first call, and the line after `launch` says where each path arrives. In runs with real agents a saved path was taken in 1 of 9 runs before and in 16 of 17 after, at 1 to 2 calls where it was 3 to 5.
- **A replay does not wait for a ticking screen** (#41, Android): a replayed step, and a `batch` step, tells the daemon what the next step touches. When that target is in place and still, the reply stops waiting for every line of the page to agree, which a stopwatch or a ticker never lets happen and which cost a second a step; under a banner or a carousel that never stops, a step whose next target has just arrived no longer waits the whole 5 s. Every check is as it was, and a spinner, a page still sliding in, a covered target and a `type` wait as before. Plain quiet screens are no faster.
- **`replay --screens` keeps the screen that differed** (#61): a step that stops for its screen writes what it saw beside the kept picture as `05.now.png` and says so. A screen that differs is now looked at for 3 s before it counts, up from 1.5 s: an app on a slow machine drew nothing for 2.6 s in the middle of a switch's slide.
- **Fixed: `lane the phone's own screen of this device is already open from this daemon`** (#59): a screenshot that found the video stream gone reconnected while it still held the old connection, so the phone refused the new one. The daemon's log now says what the agent printed when a stream ends.
- **Named paths per app** (#41): a run saved as `paths/<app id>/<name>.steps` is a path of that app. It ends on a landmark, a wait for the screen it arrived on that `save` writes and a person can edit; `launch` names the app's paths in one line, with the blanks each needs; and `replay` of one answers `arrived at "Internet"` with the arrival screen and no step list. A path that ends anywhere else, even on a page whose title only holds the landmark's text, fails with the step, the screen, and nothing tapped after. No new tool, argument or file format; MCP `replay` saves and runs them.
- **Files in and out** (#52, Android): `files push cat.jpg` copies a file from the one folder you name in `PHONEDRIVER_FILES` to the device's Pictures or Download, where the gallery shows it, `files pull Download/a.pdf` brings one back, and `files list` shows the newest files in Download, Pictures and DCIM (`files list DCIM/Camera` for more of one folder). A reply is a receipt (path, size, SHA-256) or a list of paths with sizes and ages, never contents. Nothing is replaced, a physical phone asks its owner for every file and before the first listing, and without the folder set everything is refused. MCP: a `files` tool (`push`, `pull`, or no argument to list). An iOS Simulator lists and pulls its photo library.
- **A shorter tool list** (D130): what an agent reads of the MCP tool list on every turn is 6,877 characters, down from 7,366, with `files` as a tool of its own. `replay`'s `from` and `any_build`, `setting`'s `value` and `batch`'s `autonomous` are still accepted and are named by the hint that needs them. `setting`'s `restore` is now in the schema as a boolean, and `true` sent as a string is read as true. The server's instructions say that `snapshot` with `notifications` reads notifications, and the receipt of a pushed picture says the gallery shows it.
- **`setting restore` puts settings back** (#52): the daemon keeps what each key was before its first write, in a file, so it works after a crash too. A restore prints what it did to each key and leaves one that someone else changed since. `release` lists what is still changed and restores nothing, because the change may be the task. MCP: `setting` with `restore: true`.
- **A screenshot of one element** (#52): `screenshot --within @e7` (or its text; MCP `within`) is that element with 8 points around it, and says where its top left is on the screen, so a pixel of it is still a point to tap. `--marks` works inside it. A Settings row is 10 KB where the screen is 121 KB.
- **A date or a time in one call** (#52, Android): `tap "Date" --select 2026-12-18` (MCP: `tap` with `select`) opens the picker, sets it, reads the value back from the picker and only then confirms; a time is `19:45`. It sets a calendar that names its days and a time picker with a text input mode, which covers Android's own pickers, a web page's date and time fields, Compose's calendar and Clock. A picker it cannot set (wheels, a Compose clock face) is left open and the error shows the screen. `save` keeps the value.
- **Pickers of every kind, in any language, on Android and iOS** (#52): `tap` with `select` now also sets date and time wheels, a Compose clock face (a minute between two marks is dragged to and read back), Material Components date and time pickers, and iOS's compact date picker and wheels. The month names and AM and PM come from the device in the language the app is drawn in, so nothing depends on English: German, Japanese and Arabic are run on the emulator and on the Simulator. A date decades away goes through the picker's list of years (11 to 15 s) or, on iOS, its year wheel (17 to 24 s). It still refuses rather than guesses: a calendar that is not Gregorian, a picker in a language the device does not name, and a confirm button it does not know are left to you, and the reply says which. iOS picker wheels now read as `slider` with their value. No change to the tool list.
- **A long MCP call says what it is waiting for** (#52): a client that sends a progress token with a call gets a progress notification after 2 s and then every 5 s, such as `wait: the condition has not held yet · 7s of 30s` or `replay: step 4 of 12 · 7s`. A client that sends none gets nothing new, and no text given to the call is ever in the message.
- **A replay finds its device once** (#41): it used to look up the default device again for every step, 0.3 to 0.8 s a replay on a Mac with Xcode; every wait that guards a step is as it was. The build check now reads the app by its id: a run saved after `launch Settings` kept another package's build, and one saved after a launch by a name with a space kept none and was never checked.
- **`scroll --collect` reads a whole list in one call** (#52): it scrolls a page at a time and returns every row it passed, each once and in order, with why it stopped (the list ended, the limit, the scroll limit). MCP: `scroll` with `collect`. Scrolls in a `batch` return one diff, so rows in the middle were lost.
- **Notifications without opening the shade** (#52, Android): `notifications` lists app, title, text and age, newest first (MCP: `snapshot` with `notifications`). `wait notification:"code"` returns the one posted since the session's last action, never an older one still up. The text stays out of traces and saved runs.
- **`check` reads a value in the middle of a flow** (#52): `check "Total"` answers `"Total" = "€24.99"` and touches nothing; as a `batch` step it costs no model turn, and with `equals` it stops the batch before the next tap when the screen says anything else. A switch reads `on` or `off`, a settings row reads its summary, and a password field is never read. `save` keeps checks and `replay` runs them.
- **A tap that finds nothing answers with the screen** (#37): a `tap`, `type`, `scroll` or `check` whose target or `@ref` is gone carries what changed since the screen the agent last saw, so the next call acts from the error instead of calling `snapshot` first.
- **Small MCP fixes**: `logs` takes `since`, `replay` takes `screens` (a folder inside the server's own), `snapshot` lists `audit`, `devices` says what a lane is, and `scroll` and `swipe` list the four directions as fixed values.
- **Replays check what they type** (#41, #42): a replayed step whose field shows other text than typed, or that nothing confirms, stops the replay; a saved `type --submit` sends Enter only once the field shows the text; the first line says how many typed texts were verified.
- **Saved files hold up better** (#41): a field the keyboard pushed off the screen is saved by its label, and a failed `scroll --until` that moved the list is kept as the scrolls the next step needs.
- **Read-backs** (#41): `save` names typed values no later step reads back, and `wait` finds a phone number however the app formats it, never behind a country code it did not type.
- **Blanks with examples**: `{{day1:Tuesday}}` tells whoever fills the blank the wording the screen uses.
- **A replay checks the app's build**: `save` keeps the build of each app it launches, and `replay` stops before its first step when the app is now another build; `--any-build` runs it anyway (#41).
- **A replay checks the screen before each step**: a step runs only in the app it was saved in, and with a dialog in front exactly when there was one, so a surprise dialog or another app is never tapped (#41).
- **A replay no longer stops for a page that is sliding in** (#53): Android reports the arriving page as a window short of one edge, which read as a dialog and stopped 16 of 40 replays of plain Settings steps. A step now waits up to a second for the kind of screen it was saved on before it is refused, and a step done mid-slide is saved without a screen to compare.
- **`launch --restart` gets out of another app's screen** (#51): on Android, a page that belongs to another package (Settings' search, Default apps) stayed in front of every restart of Settings. A restart now clears the app's task as well as stopping it.
- **A `batch` step does what its tool does alone** (#50): `then` on a `tap` or `type` step taps the texts it names, and a `wait` step keeps its own `timeout_ms` (`--timeout` on the command line). A step that names a `device` is refused instead of running on the batch's device, and so is any other batch-wide option written on a command-line step.
- **The approval guard is harder to get past**: a button is judged by what it says when it is tapped, not when it was first seen; a label written in other characters (full-width letters, typographic quotes) is the same label; an approval needs one of the choices the prompt offered, and the prompt chooses none in advance. `autonomous` now covers a `batch` and a `then` chain as well as a single tap.
- **On a physical Android phone**, `shell` also refuses `input` taps, swipes, text and the keys `press` sends (those verbs do the same with the approval guard watching), intents that place a call, and `find` that deletes from the storage root; `open` refuses an `intent:` link that places a call. Installs, media keys, Settings screens and `find -exec` with a program that reads still run. `PHONEDRIVER_GUARD=off` lifts all of these, as `force` does on one call.
- **The Android agent** closes a connection that has not said hello within 5 seconds; any app on the phone could open one and hold a thread in the agent for as long as phonedriver ran.
- **Secrets**: a PIN or card security code field takes only `--secret` text; an argument error never quotes the value it was given; a question waiting for an approval keeps a hash of the call, not its text.
- **`wait` answers with what changed**, as an action does, instead of the whole screen. `launch` says when the app crashed at start and points at `logs`. "The tap may have missed" follows only a tap.
- **Fewer reads**: a connection lists its apps once instead of on every `launch`, and a launch turns off the last app's OCR.
- **Clearer errors**: a daemon bug in the middle of a call says so (`crashed`) instead of "disconnected"; an option a method does not take is refused by name; several connected devices are listed with their models; `phonedriver pair` stops with adb's own error instead of waiting two minutes; the npm launcher says why a download failed and when Node ignores a set proxy; `phonedriver` alone lists every command.
- **Steadier**: a rotation reported just before an action is kept; a client of another version no longer replaces a daemon someone else is using; the reads after a settle share one 10 s limit; an iPhone that cannot be described no longer empties `devices`; a failed Android serial or version read is tried again; `daemon.log` is moved aside past 4 MiB; an odd-sized iOS frame or a restarted iOS video stream no longer breaks screenshots.
- **The screen text**: an app's or a device's own name cannot add a line to a snapshot, and an empty field's placeholder prints as `hint "…"` so it is not read as typed text.
- **`save`** no longer writes an empty `action` into steps that are not actions (files saved before still replay), and `replay` over MCP reads only `.steps` files.

## 0.1.7

Released 21 November 2026.

A security release. It fixes seven privately reported problems and the items of a public review of 0.1.6, all by giulioc84 (#32, #33, #34, #35, #36, #38, #39). Update if you use the approval guard, `--secret`, an app scope or device list, a Simulator, or an iPhone over Wi-Fi.

- **The approval guard holds every way to press a button**: a swipe, drag, pinch, gesture, scroll, `press enter` or clearing a field that would press "Pay", "Delete" or "Send" now asks first, as a tap does. An approval covers only the button and details the person was shown. Ordinary gestures read the screen no more often than before.
- **Secrets stay secret**: a `--secret` text is masked before a reply is cut short, in traces and in replays; the daemon no longer inherits `PHONEDRIVER_SECRET_*`; a replayed secret goes only into a password or code field; screenshots, recordings and traces are readable by your account only.
- **`shell` stays on the device**: on a Simulator, `defaults` and `log show` or `log stream` work as before, and other programs, `simctl spawn` and the `simctl` forms that read or write a Mac file need `PHONEDRIVER_HOST_SHELL=1`. On an emulator, the console commands that act on your computer need it too. `open` with a `file:` link on a Simulator needs it as well.
- **iPhone**: the runner on the phone answers only the phonedriver that started it; from Windows its traffic goes through the encrypted tunnel; Wi-Fi pairing checks the phone's signature. Pair again with `phonedriver ios wifi on` if you paired over Wi-Fi with 0.1.6, and install this release's `PhonedriverRunner.ipa` (the 0.1.6 one is refused).
- **App scope and device list**: about ten ways past them are closed, including links that open another app, `shell` under a scope, and recordings.
- **Recordings** hold nothing from before `record start`, and a recording of a still screen shows that screen.
- **Steadier replies**: a slow app start no longer leaves the screen unsettled (#36); a reply whose screen is still loading says so (#35); a batch stops before its next step when the text it typed plainly did not land (#38); a ref no longer moves to another app's dialog button with the same id (#38); a field the keyboard scrolled away is found by whole words (#39).
- **Fewer tokens**: the tool list every turn re-reads is 7% shorter (#34).
- **Release files**: the iOS runner has a checksum, and every file has a build attestation (`gh attestation verify`).

## 0.1.6

Released 20 November 2026.

- **An iPhone from Windows**: with Apple Devices installed, phonedriver drives an iPhone or iPad on iOS 17 to 26 over the cable with no Mac. You sign and install the runner from this release (`PhonedriverRunner.ipa`) with Sideloadly, and `phonedriver ios setup --download-ddi` mounts the developer disk image itself. Contributed by Coreho (#23).
- **An iPhone on Wi-Fi from Windows**: `phonedriver ios wifi on` once on the cable, then unplug and keep driving the phone over Wi-Fi, as Xcode's "Connect via network" does. Nothing shows on the phone. `ios wifi off` forgets the pairing. Contributed by Coreho (#29).
- **`devices` answers while a phone is stuck connecting**: it no longer waits behind a call to a device that does not answer (seen for over 3 minutes with an iPhone that dropped off Wi-Fi) (#29).
- **Logins in CI**: a saved run replays a `--secret` step from an environment variable (`PHONEDRIVER_SECRET_1`, `_2`, ...), so a run that signs in replays unattended in the GitHub Action with `env: PHONEDRIVER_SECRET_1: ${{ secrets.APP_PASSWORD }}`. The secret is masked like any other.
- **Variables in saved runs**: `{{name}}` in a step is filled by `replay --var name=value` (the Action's `vars` input), and `{{run}}` is a short id new for each replay, for fresh data such as `test+{{run}}@example.com`.
- **`phonedriver network`** shows an Android device's networks, which one is the default, a VPN's owning app and the users it covers, DNS per network, and the always-on VPN and lockdown settings.
- **`wait vpn:up`, `wait vpn:down` and `wait log:<text>`** (with `--app`) wait for a VPN to come up or go down, or for a line in the device's log. A timeout says what was last seen. The VPN connection request counts as a system dialog. Asked for in #17.
- **`apps notify <app> '<json>'`** sends an app a push notification on the iOS Simulator, to test how the app handles one.
- **Your own app on a physical phone**: `PHONEDRIVER_APPS_UNDER_TEST=com.example.app` in the server's environment lets `apps clear`, `uninstall`, `grant` and `revoke` work on an app Android Studio installed.
- **`devices {use: "<id>"}`** sets the MCP server's default device for the rest of the session.
- **An app scope for a real test phone**: `PHONEDRIVER_APP_SCOPE=com.example.app` keeps the agent to your app. Other apps' windows are never read, and taps, `shell`, screenshots and recording are refused while one is on screen. System permission dialogs and the keyboard still work, and `press back`, `press home` and launching your app get back. It is not a sandbox; the reference lists what it does not cover (#28).
- **Phones an agent must never touch**: `PHONEDRIVER_DENY_DEVICES`, `PHONEDRIVER_ALLOW_DEVICES` or a `~/.phonedriver/devices` file hide a device as if it were not connected. Nothing lists it, probes it or pushes to it, and `--force` does not get through. A rule can name an Android phone's hardware serial, so it holds over USB and Wi-Fi (#26).
- **`doctor` asks before touching a phone**: without `--device` it checks the host, lists every device, and probes only emulators and Simulators no session is using. For a physical phone it prints the command to probe it, and `--all-devices` probes them all (#27).
- **Video on Samsung phones**: on a Galaxy A53 (One UI 8) no frame ever arrived, so screenshots timed out. The video now goes through an OpenGL relay on Samsung phones and wherever the encoder gives nothing. Found and fixed by DrogaBox (#22).
- **A tap under a web page's fixed bottom bar** (a tab bar on instagram.com in Chrome) goes to the part of the tile that shows, and `scroll --until` keeps going until the tile clears the bar (#16).
- **`shell` on an iOS 26 Simulator** works: its runtime has no `/bin/sh`, so each command now runs as one program (#15).
- **Replay fails on a failed check**: a `shell` step that exits non-zero fails the replay, unless the step expects that status (#13).
- **Fixes from an outside review** (#15, #16, #18, by giulioc84): `save` keeps the steps when the file cannot be written; an empty Compose field no longer reads as `"Empty"`; the install record on physical phones names only the app that was installed; an MCP session's state ends with its connection; `doctor` names the devices it will start the agent on; shell time limits, tap counts and holds are bounded; Simulator `apps clear` reports what it could not remove; the GitHub Action uses the Simulator it booted (#21); the MCP server's calls get the agent lease (#19); and the daemon's startup cleanup no longer follows a link to a folder (#20).

## 0.1.5

Released 17 November 2026.

- **`phonedriver init`** adds phonedriver to the agents installed on this machine (Claude Code, Codex, Cursor, VS Code and Gemini CLI) and then runs the doctor checks.
- **`phonedriver boot <name>`** starts an iOS Simulator or an Android virtual device that is off and waits until it has booted. With nothing connected, the error names what can be started.
- **Saved runs**: `phonedriver save run.steps` writes what the session did, and `phonedriver replay run.steps` does it again with no model. It stops at the first step that fails and shows the screen.
- **A GitHub Action**: `uses: hridaya423/phonedriver@v0.1.5` starts an Android emulator or an iOS Simulator, installs your app and replays saved runs on every commit. Each run is a row in the job summary, and its reply, the screen where it stopped and a video go in an artifact.
- **Screen checks in a replay**: `replay run.steps --screens screens/run` keeps a screenshot after each step the first time, and later stops at the first step whose screen shows different text, with the text that changed (`~ "General" → "About"`). The status bar is left out, with its clock, notification icons and battery, and a screen still moving is looked at again until it stops, so an animation or a notification does not fail a job. The Action takes the folder as `screens`.
- **`phonedriver page`** prints the web page in front as text, whole, without scrolling. On Android, it works in Chrome and in the web views of debug builds.
- **`phonedriver record start` and `record stop`** save an MP4 of the screen on Android and on the iOS Simulator, with a file of the actions done during it.
- **Settings by key on Android 17**: `phonedriver setting find`, `get` and `set` change a setting without opening its screen.
- **App permissions**: `phonedriver apps grant <app> <permission>` and `revoke` work without launching the app.
- **Crashes**: an action during which the app crashed says so in its reply, and `phonedriver logs` prints the app's last lines and its last crash.
- **Several agents on one Android phone**: a device id that ends in `+a`, `+b` and so on gives each agent a hidden screen of its own (Android 13 and later).
- **An Android phone in use stays awake**: while phonedriver has used a phone in the last 10 minutes it resets the phone's screen timeout every 10 seconds, on the phone's own screen and with `PHONEDRIVER_DISPLAY=hidden`, so the phone no longer locks while an agent thinks. The screen still dims on its usual schedule. `phonedriver release` stops it. Tested on a HONOR X8 and the emulator.
- **The emulator's console**: on an Android emulator, `phonedriver shell "emu geo fix -122.41 37.77"` sets the location, and any other console command works the same way (`emu sms send`, `emu power capacity`). On the iOS Simulator, `shell "simctl location set 37.77,-122.41"` does it.
- **Fingerprint and Face ID**: `press fingerprint` and `press wrong-fingerprint` on an Android emulator and on an iOS Simulator.
- **`press landscape` and `press portrait`** turn the screen.
- **Accessibility audit**: `phonedriver snapshot --audit` (MCP: `audit: true`) adds the screen's controls that have no name a screen reader can say and its tap targets under 44 by 44 points on iOS or 48 by 48 on Android.
- **`phonedriver compare a.png b.png`** says in text what changed between two screenshots, in 0.3 to 1.2 s on a Mac. The status bar's clock is left out.
- **Screens with no element list** are read from the screenshot on Windows hosts too, and on Linux hosts with Tesseract installed (the `tesseract-ocr` package), not only on a Mac.
- **Reading a screenshot on a Mac starts at once**: the first read in a process used to wait 21 to 31 s while macOS built the model for Vision's Accurate level, and on some Macs that level failed after 12 to 17 s every time. phonedriver now reads at Vision's Fast level, which needs no such model: about 20 ms a screen on an M4 Pro.
- **An iPhone's signature renews itself**: when the runner's signing has run out (7 days with a free Apple ID), the next command signs it again and installs it. `doctor` shows when it runs out.
- **Passwords**: text typed with `secret` is masked as `•••` in every later reply, error and trace line. An MCP `batch` can now carry a `type` step with `secret`, so one call fills a sign-in form and submits it.
- **Fewer tokens**: a reply says each thing once. A snapshot's header names the keyboard only when it is up and the settle time only when the screen did not settle; the keyboard coming up is one line (`keyboard shown over @e8`); a verified `type` does not repeat the text it typed; a tap's point is in the JSON reply's `at`, not its text. On iOS a snapshot no longer starts with a line naming the app, buttons that get their identifier late keep their refs, and a row whose text changes is one `~` line. On simulator-use's sign-in benchmark phonedriver reads the screen in 59 tokens and signs in step by step with 173 (it was 300); simulator-use took 71 and 192 in the same run ([benchmarks](https://phonedriver.dev/benchmarks/#simulator)).
- **Fixed**: `wait "hello"` and `scroll --until "hello"` did not find text typed into a field, though the screen showed it as `textbox "hello"`. They now look in what fields hold, never in a password field.
- **Fixed on iOS**: a system permission prompt over the home screen was listed twice, so `tap "Don’t Allow"` matched two elements.
- **Fixed on iOS**: `launch` of an app that asks for a permission as it starts, such as Maps, waited 13 s and said the app was not in front, because the system's prompt was. It now answers `launched Maps` with the prompt on the screen, 5 s sooner.
- **The iOS Simulator needs only Xcode**: the runner's Xcode project comes with phonedriver, so XcodeGen is no longer needed, and `doctor` no longer asks for it.
- **Fixed on iOS**: Xcode recorded a video of every phonedriver session on a Simulator, as it does for UI tests, and kept it in the Simulator: 1.2 GB on the Mac that ran the tests, up to 209 MB a session. The runner now records nothing.
- **A faster iOS Simulator**: a tap goes to the Simulator's own touch input instead of through XCTest, and the settle watches the Simulator's screen as it is drawn instead of asking XCTest for screenshots. On simulator-use's sign-in benchmark a tap takes 0.32 s (it was 0.84), a read 29 ms (117), the sign-in step by step 1.76 s (3.63) and in one batch 1.78 s (4.35). Typing still goes through XCTest, which types exactly; sent as hardware keys instead, quotes came out curled and capitals lowercase. Taps with several fingers, in an app on its side, and under a notification banner still go through XCTest.
- **Faster swipes on iOS**: the runner looked up the screen's size and the orientation again for every point of a gesture. It now does so once, and a swipe on the Simulator takes about 0.24 s less (1.27 s against 1.51 s at the median, a debug build); a tap's input took 0.06 s less.
- **Fixed on iOS**: with the device on its side and an app that stays upright, taps landed where the screen would have been had the app turned. They are now turned by the app's orientation.
- **Fixed on iOS**: on a CI machine with a Simulator just booted, the first command could fail after 60 s waiting for the runner, because SpringBoard took 41 s to answer its first question. The runner now asks it once before it says it is ready, and phonedriver waits up to 3 minutes for a Simulator's runner. The Action keeps the daemon's log with the runner's when a run stops.
- **Fixed on iOS**: every start of the runner left a folder in Xcode's DerivedData, and a runner that was stopped left 120 to 250 MB of the Simulator's system logs in it; 84 such folders took 940 MB on the Mac that ran the tests. The runner's logs now stay under `~/.phonedriver/runner`, replaced at each start, and no system logs are gathered. Delete `~/Library/Developer/Xcode/DerivedData/PhonedriverRunner-*` to get back what earlier versions left.
- **Fixed on iPhone**: `launch Settings` matched CarPlay's hidden settings app as well on iOS 27. Apps that iOS hides from the home screen are no longer listed.
- **Fixed on Android**: when the connection to the phone broke during an action, the reconnect could be refused with `lane the phone's own screen of this device is already open from this daemon`. It now waits up to 5 s for the phone to let go of the old connection.
- **Fixed on Android**: a screenshot taken right after an action could show the screen from before it.
- **Fixed on Android**: with `PHONEDRIVER_DISPLAY=hidden` on a locked phone, `launch` waited 9 s and blamed the app, and opening the hidden screen turned the phone's screen on. It now says the phone is locked and leaves it asleep. The hidden screen works only while the phone is unlocked. Tested on a HONOR X8.
- **Fixed on Android**: a request the phone never answered waited ten minutes. It now ends as a lost device.
- The third action in a row that changes nothing says so and tells the agent to stop repeating it.
- `launch` waits for an app that is slow to start, and a submit that left the keyboard up says so.
- Every MCP tool refuses an argument it does not know, so a misspelled argument is an error, not silently ignored.
- `phonedriver screenshot shot.png` takes its file as a plain argument.

## 0.1.4

Released 14 November 2026.

- **Fixed on Android 12**: every command that reads the screen failed with `NoSuchMethodException: AccessibilityInteractionClient.clearCache [int]`. The agent now uses whichever `clearCache` the device has. Tested on an Android 12 (API 31) emulator. Reported in #7.
- **An iPhone in use stays awake**: while phonedriver has used an iPhone in the last 10 minutes it resets the phone's Auto-Lock timer every 10 seconds, so the phone no longer locks while an agent thinks. `phonedriver release` stops it. Tested on an iPhone 13 Pro Max with Auto-Lock at 30 seconds.
- **Fixed on iPhone**: a tap that waited for a notification banner could answer with an error although the touch had landed. It now answers `ok`.
- The approval guard is now tested on a physical iPhone: a held, declined or unanswerable tap on Place order reached the app zero times.

## 0.1.3

Released 13 November 2026.

- **iPhone without the cable**: `phonedriver ios wifi on` lets a physical iPhone or iPad answer over Wi-Fi. Unplug it and it stays in `phonedriver devices` while it shares a network with the Mac. Driven on an iPhone 13 Pro Max, iOS 27.0.1; an iPhone 14 Plus on iOS 26.1 was found and answered, with no taps run. A sleeping iPhone stops answering until it wakes. `phonedriver doctor` says which link each device uses.
- **An iPhone the signing team has never seen**: the first command used to wait 60 seconds and point at an Xcode log. It now says the device is not registered and names the fix, `phonedriver ios register`. A locked iPhone at runner start is reported as locked.
- **A locked iPhone says so**: an action on a locked iPhone used to answer "no element matches". It now answers that the device is locked, as Android does.
- **Notification banners on iOS**: a tap that starts under a banner waits for the banner to leave, so it no longer opens the notification.
- **No team ID to look up**: phonedriver reads the signing team from this Mac's Apple Development certificate, so `PHONEDRIVER_TEAM` is needed only with certificates for several teams. The old hint pointed at the ID in parentheses after your name, which is not the team. `phonedriver ios setup` and `phonedriver doctor` print the team they found.
- **`phonedriver ios setup` finishes the job**: after its checks it builds, installs and starts the runner, so the next command answers at once (0.3 s on an iPhone 13 Pro Max) instead of starting the runner itself.
- **MCP annotations**: every tool says whether it only reads the screen or can change something, so a client can run `snapshot`, `screenshot` and `wait` without a permission prompt. `then` is now in the `tap` and `type` schemas, so an agent sees it without the skill.
- **A command you ran by hand no longer blocks your agent**: a device a terminal session has left idle for 30 seconds goes to the next session that asks. It used to stay held for 10 minutes.
- `phonedriver release` with several devices connected gives up every device the session holds. It used to ask which device you mean.
- **Slow links**: when the phone is far away (a mobile network, a relayed tunnel), the screen stream drops from 8 Mbit/s to 400 kbit/s at a smaller size, screen trees travel compressed, and Android tells the host when its screen or lock changes, so a tap makes one round trip fewer. On a link shaped to 3G the median tap on an emulator went from 2.8 s to 1.4 s. On a HONOR X8 the mean went from 2.6 s to 1.7 s and the run sent half the bytes.
- `phonedriver untether` stops at once when the Android phone is on mobile data only, and changes nothing on it.
- `phonedriver untether` waits for adb on the phone to restart. It used to fail about one run in five on a HONOR X8 and work when run again.
- `phonedriver pair` and `phonedriver untether` are now tested on a physical phone (HONOR X8). On a Mac, both restart an adb server that macOS keeps off the local network.
- A phone connected in more than one way (cable, Wi-Fi, port 5555) is one line in `phonedriver devices`, so one phone no longer asks which device you mean. `phonedriver doctor` checks such a phone once.
- `phonedriver ios setup` mounts the developer disk image again with Xcode 27, whose `devicectl` dropped the option the old command passed.

## 0.1.2

Released 12 November 2026.

- **A path of taps in one call**: `tap` and `type` take `then`, a list of texts to tap next, in order. `tap {"target":"More options","then":["Settings","Dark theme"]}` opens the menu, taps Settings and taps Dark theme, and returns one numbered reply. A text that is not on the screen stops the path and returns that screen. A tap that needs approval is still held for the human, and `then` is refused together with `secret`.
- The skill (`phonedriver skill`) tells agents to send a path they can predict as one call.

## 0.1.1

Released 10 November 2026.

- **Fixed on Windows**: `npx -y phonedriver` failed on its first run in a Git Bash shell. The launcher now unpacks the download with the `tar.exe` that Windows ships in `System32`. Before, GNU tar came first on `PATH` there, read `C:\...` as a remote host and could not open the zip.
- A manual CI job installs the published npm package on every release platform (macOS arm64 and x64, Linux x64 and arm64, Windows x64) and runs it.

## 0.1.0

Released 10 November 2026. The first release.

- **Android**: emulators and physical phones and tablets over adb. No app to install and no accessibility service to switch on.
- **iOS**: the Simulator, and physical iPhones and iPads from a Mac with Xcode.
- **14 tools**, the same over MCP and as CLI commands: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard` and `shell`.
- **Text snapshots**: a screen as numbered lines. Every action waits for the screen to settle and returns only the lines that changed.
- **Approval guard**: a tap on Pay, Buy now, Place order, Transfer or Delete account is held until the human approves it.
- **Secrets**: text typed with `--secret` never reaches logs, traces, errors or replies.
- **No telemetry**, checked by a test that runs the tool in a sandbox that kills any connection that is not local.
- **Install**: `npx -y phonedriver`, Homebrew (`brew install hridaya423/tap/phonedriver`) and release archives for macOS, Linux and Windows.
