# phonedriver: rules for agents working in this repo

phonedriver lets AI agents see and operate Android and iOS devices. The design is in `docs/`. The work is in `docs/plans/`.

## Start here

1. Read `progress.md`. It says where the last session stopped.
2. Read `docs/plans/2026-11-05-00-master-plan.md`, then the current phase's plan.
3. Read `docs/architecture.md` sections the current task names. Read `docs/decisions.md` before changing any design choice.
4. Execute with the `executing-plans` skill, task by task.

## Rules

- **Plans are the contract.** If reality disagrees with a plan (a spike result, an API that does not exist), update the plan, `docs/architecture.md`, and `docs/decisions.md` in the same commit, and say why in `progress.md`. Do not silently diverge.
- **Test first.** Each task starts with a failing test. Device behavior is proven against the testbed apps' event logs, not against our own replies.
- **Evidence.** After each task, append to `progress.md`: what changed, the command you ran, and the output that proved it. Look at every screenshot and rendered snapshot you claim is right.
- **Build with blaze.** `blaze build`, `blaze test`, `blaze check`. Use `cargo` only for `add`, `update`, `fmt`, `clippy`, and what blaze does not cover.
- **No comments** except license headers, public API docs, and external constraints the code cannot express (for example "dtuhidd drops HID events unless a media stream is active").
- **No speculative code.** No traits with one implementation (the `Device` trait has two plus a fake), no retries or fallbacks without an observed failure written next to them.
- **Spikes are throwaway.** They live in `spikes/` and are deleted at the end of their phase. Their results live in `docs/spikes/`.
- **No telemetry, ever** (decision D9). Never add code that sends data anywhere except the device and, for DDI mounting, Apple.
- **Secrets.** Text passed with `--secret` must never reach logs, traces, errors, or test output.

## Machines

- **MacBook** (where this repo lives): Rust, Xcode 27, Android build-tools 37 at `/opt/homebrew/share/android-commandlinetools`, `d8`, `adb`, JDK 17. Never drive GUIs on its screen.
- **win-pc**: Android emulators (`Pixel_API_37`; create `Pixel_API_26` and `Pixel_Tablet_API_37`), SDK at `D:\Android\Sdk`. Reach its adb server with `ssh -N -L 5038:127.0.0.1:5037 win-pc` and `ADB_SERVER_SOCKET=tcp:127.0.0.1:5038`.
- **Mac mini**: iOS simulators and anything with a GUI. Shell and files through the cua Space `mini`; its screen through the `mini-desktop` MCP.
- **Physical phones and tablets** belong to people. Ask the user before touching any of them.

## Stop and ask (one-way doors)

Driving or installing on a physical device; updating a device's OS; downloading DDI images without Xcode; creating a remote, pushing, or publishing; choosing the license. Ask one question at a time with a recommendation.

## Commits

One local commit per task, if the user approved commits for the session (ask once at the start). Never push.
