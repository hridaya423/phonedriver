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

- 2026-11-05, Task 0.1 workspace skeleton. `blaze test -p phone-use` failed first (stdout "" for `version`, no usage), then `test result: ok. 2 passed`. `blaze pin` wrote `[workspace.metadata.blaze] version = "0.20261030.1"`.
- 2026-11-05, Task 0.2 lints and CI. `cargo fmt --check && cargo clippy --all-targets -- -D warnings` → `Finished`, no warnings; `blaze test` all green. Clippy group needs `priority = -1` (`all = { level = "deny", priority = -1 }`). CI (`.github/workflows/ci.yml`) cannot run until a remote exists, which is a one-way door left to the user.
- 2026-11-05, Task 0.3 geometry. `blaze test -p phone-use-core geometry` failed to build first (types missing), then `5 passed` including the `device_round_trip` property test (256 cases).
- 2026-11-05, Task 0.4 element model and RawNode parsing. Tests failed to build first (`unresolved import`), then `blaze test -p phone-use-core` → `15 passed`. Fixture `crates/core/tests/fixtures/android/handmade_settings.json` (18 nodes, density 3, uses every wire key). Choices: `core::raw` normalizes each platform's wire node into neutral `label`/`value`/`hint` at parse time; role tables live in `core::element` as the plan says (architecture 4.2 updated to match). Surprise: `blaze test` built `#[derive(Serialize)]` on bitflags types without bitflags' `serde` feature, but `cargo clippy` failed on it; the feature is now explicit. Run `cargo clippy` after each task, not only blaze.
- 2026-11-05, Task 0.5 ref assignment. Refs belong only to kept nodes, so pruning (architecture 4.4 rules 1 to 4, including label merging) landed here as `core::prune` (`prune(&RawTree) -> Vec<Node>`), and `RefTable::assign(&RawTree, &[Node]) -> Vec<Ref>` takes its output; 0.6 renders from the same nodes. Tests failed to build first, then `BLAZE_TESTS=all blaze test -p phone-use-core` → `25 passed` (property test `refs_are_stable_unique_and_never_reused`: 256 cases over insert, delete, move, relabel, with a second table checking determinism). Mutation check: dropping the continuity condition makes `recycled_row_gets_a_new_ref` fail. Note: `blaze test` caches passing tests; use `BLAZE_TESTS=all` for evidence runs. iOS rows have no `V` flag in protocol section 3 yet; prune requires `V`, so the runner must emit it (Phase 5).
- 2026-11-05, Task 0.6 snapshot rendering. Tests failed to build first, then unit tests passed and the four goldens failed as new. Read all four `.snap.new` files (default, `--interactive`, `--within @e3`, `--max-lines 3`); they match architecture section 1's shape (`@e4 button "Network & internet" "Mobile, Wi‑Fi, hotspot"`, `    @e8 switch "Dark theme" on`, `… 6 more`), so accepted by renaming (cargo-insta is not installed). `BLAZE_TESTS=all blaze test -p phone-use-core` → `40 passed`; clippy clean. The plan's `--within @e2` became `@e3` because the fixture's list is `@e3` (heading `@e1`, search `@e2`). Text resolution returns the target ref plus its center; architecture 4.4 now lists role words and the second-string rule.
- 2026-11-05, Task 0.7 diff. Tests failed to build first. Then two failed because my test buttons reused one source id per slot, which ref continuity rightly reads as "relabeled in place"; fixed the test data to give each label its own node id, as a real new screen does. `BLAZE_TESTS=all blaze test -p phone-use-core diff` → `7 passed` (toggle `~ @e4 switch "Dark theme" off → on`, navigation → `Full`, `- @e1 @e2 @e3 @e4 @e5`, value `"Bo" → "Bob"`, `secure field changed` with no secret in the `Debug` output). Architecture 4.5 states the line shapes and the share formula.
