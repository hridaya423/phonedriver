# Changelog

Every release of phonedriver, newest first. Binaries and checksums for each version are on [GitHub Releases](https://github.com/hridaya423/phonedriver/releases).

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
