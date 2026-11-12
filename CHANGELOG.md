# Changelog

Every release of phonedriver, newest first. Binaries and checksums for each version are on [GitHub Releases](https://github.com/hridaya423/phonedriver/releases).

## Unreleased

- **iPhone without the cable**: `phonedriver ios wifi on` lets a physical iPhone or iPad answer over Wi-Fi. Unplug it and it stays in `phonedriver devices` while it shares a network with the Mac. Tested on an iPhone 13 Pro Max, iOS 27.0.1. `phonedriver doctor` says which link each device uses.
- **An iPhone the signing team has never seen**: the first command used to wait 60 seconds and point at an Xcode log. It now says the device is not registered and names the fix, `phonedriver ios register`. A locked iPhone at runner start is reported as locked.
- **Notification banners on iOS**: a tap that starts under a banner waits for the banner to leave, so it no longer opens the notification.
- `phonedriver untether` stops at once when the Android phone is on mobile data only, and changes nothing on it.
- `phonedriver pair` and `phonedriver untether` are now tested on a physical phone (HONOR X8). On a Mac, both restart an adb server that macOS keeps off the local network.
- A phone connected in more than one way (cable, Wi-Fi, port 5555) is one line in `phonedriver devices`, so one phone no longer asks which device you mean.
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
