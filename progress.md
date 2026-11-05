# Progress

## Status

Planning complete on 2026-11-05. No code yet. Next: Phase 0, Task 0.1 (`docs/plans/2026-11-05-01-phase-0-foundation.md`).

## Done

- 2026-11-05: Research and plan written.
  - Competitor read at source: `docs/research/competitor-teardown.md` (phone-harness `4bebff1`).
  - Prior art read at source: `docs/research/prior-art.md` (scrcpy `a60891a`, WebDriverAgent `3d6a4e0`, go-ios `6910f3b`, idevice `8970209`).
  - Design: `docs/architecture.md`, `docs/decisions.md`, `docs/protocol.md`, `docs/GLOSSARY.md`.
  - Plans: `docs/plans/2026-11-05-00-master-plan.md` and phases 0 to 8.
- 2026-11-05: Independent adversarial review of the plan (Claude Sonnet 5.5 subagent, no shared context) returned 22 findings. Two were verified at source before acting: scrcpy sets `KEY_REPEAT_PREVIOUS_FRAME_AFTER` (`SurfaceEncoder.java:31,322`), and go-ios (`offer.go:35-38`) and idevice (`negotiation.rs:166-186`) label RTP payload types 100/123 with opposite codecs. Changes applied: sessions keyed by name rather than connection; ref continuity for recycled rows; label merging for clickable rows; richer `RawNode`; two-phase settle (first change, then quiet) by area in points, with `settled: false` as a value; encoder settings from scrcpy; agent socket token; correct `pkill` pattern and shell-stream ownership; no-suppress accessibility flag; secrets off argv; codec spike and depacketizer task; iOS reachability stated; held-out M3 gate; budgets measured co-located on win-pc; cuts (Wi-Fi pairing client, `ocrs`, `--display`, `use` verb, adb crate evaluation).

## Facts observed during planning (verify again before relying on them)

- MacBook: rustc 1.99.1, blaze installed, Xcode 27.0 (27A266a), OpenJDK 17.0.20.1, d8 9.3.16, Android platforms `android-34`, `android-36`, `android-37.0` and build-tools up to 37.0.0 under `/opt/homebrew/share/android-commandlinetools`. `adb devices` showed no Android device locally.
- `xcrun devicectl list devices`: "Mamata's iPhone" iPhone 14 Plus iOS 26.5 paired and available; "Mukesh's iPhone" and an iPad Pro 13-inch (M5) unavailable. Do not touch without the user's permission.
- Name availability on 2026-11-05: `phone-use` free on crates.io and PyPI, taken on npm; `github.com/phone-use` taken.

## Open questions for the user

See the master plan, "Open questions for the user". None block Phases 0 to 4.

## Evidence log

(Append one entry per task: date, task, command, observed output.)
