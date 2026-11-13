# Changelog

Every release of phonedriver, newest first. Binaries and checksums for each version are on [GitHub Releases](https://github.com/hridaya423/phonedriver/releases).

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
