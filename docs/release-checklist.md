# Release checklist

Task 8.3. This list is prepared and not executed. Nothing here has been published, tagged, pushed, or reserved. Each item ends with a recommendation and the exact decision you need to make. Facts come from the files named in each item. Anything marked "not verified" was not run.

## State on 2026-11-06

- Local packaging works for macOS arm64 (`dist-workspace.toml`, `progress.md` task 8.2). The other four targets are configured and not built.
- There is no LICENSE file, no git remote, and no CI.
- Phase 6 (physical iOS devices) is stopped, so the first release has no iOS device support. The README already says "iOS device: Not built yet".
- Phase 8 task 8.1 (AndroidWorld) has no result. Release notes can use only the benchmarks that exist.

## 1. License (D13)

Facts. `NOTICE` credits scrcpy, which is Apache-2.0, as the source of the code we adapted. `idevice` is MIT. `openh264` (the Rust crate) is BSD-2-Clause.

Decided 2026-11-06: AGPL-3.0-only (replaced MIT the same day, before any release). `LICENSE` exists, every crate sets `license = "AGPL-3.0-only"`, and `dist-workspace.toml` should add `LICENSE` to `include` next to `NOTICE`.

### 1a. The H.264 patent question

Facts. The `openh264` crate compiles Cisco's OpenH264 source. Cisco's patent license covers only the binaries that Cisco builds and distributes. A binary that we build from source does not have that license. `phone-use` decodes H.264 on every OS (architecture section 8), so every release binary contains the decoder.

Options.
- A. Ship the source build and accept the patent exposure. Most open-source projects do this. It is a legal risk, and I cannot judge it.
- B. Load Cisco's prebuilt binary at run time. The crate supports this through its `libloading` feature. The user's machine then downloads the binary from Cisco, and `phone-use` sends data to a third party (see D9 and AGENTS.md: the only allowed destinations are the device and Apple's DDI server). This needs your approval and a change to D9.
- C. Use the operating system decoder: VideoToolbox on macOS. It covers macOS only. Linux and Windows need another answer.

Recommendation. A for the first release, with a line in the README that says the binary contains the OpenH264 source build and has no Cisco patent license. Ask a lawyer before any commercial distribution. Choose B only if you want the patent license more than you want zero outbound traffic.

Decision needed. Write "A", "B", or "C". Say whether you want a lawyer to look first.

## 2. How the Android agent jar ships with `cargo install`

Facts. `crates/android/build.rs` builds `android-agent/` with the Android SDK, `d8`, and JDK 17, then embeds the jar. A crates.io build machine has none of these. `cargo install phone-use` therefore fails today unless `PHONE_USE_AGENT_JAR` points to a jar. The jar is 42,068 bytes. The packaged macOS binary contains it verbatim (verified in task 8.2).

Options.
- A. Commit the built jar and have `build.rs` use it when the SDK is absent. Add a reproducibility check.
- B. Publish only prebuilt binaries and no crate.

Recommendation. A. It keeps `cargo install` working and keeps the release auditable.

The reproducibility check, not yet written and not verified to pass:
1. Pin the JDK (17.x), build-tools (37.0.0), and platform (`android-37.0`) versions in `android-agent/build.sh`.
2. In CI, run `build.sh` into a clean directory.
3. Compare `sha256` of the new jar with the committed jar. Fail if they differ.
4. If they differ on the first run, the cause is probably timestamps in the zip. Fix it by building the jar with fixed file times and a sorted file order, and then repeat.
5. Also fail the release if a file under `android-agent/src` is newer than the committed jar's commit.

Decision needed. Write "A" or "B". If A, tell me whether the check may run on your machine only until you pick a CI host.

Also needed with A. The `phone-use` crate depends on `phone-use-core`, `phone-use-android`, `phone-use-ios`, and `phone-use-video`. Each of the four must be published first, or merged into one crate. Recommendation: publish all five under the `phone-use-` prefix and check that every name is free (section 4).

## 3. Repository owner and name

Facts. `github.com/phone-use` is taken. There is no remote. The `dist` config has no hosting set.

Options.
- A. Your own account: `github.com/<you>/phone-use`.
- B. A new organization, for example `github.com/phone-use-dev`. This costs nothing and can hold more repositories later.

Recommendation. A for the first release. Moving a repository to an organization later keeps the old links working through GitHub redirects.

Decision needed. Give me the exact `owner/name`. After that I set `repository` in `Cargo.toml`, enable `dist` installers and the GitHub hosting, and generate the release workflow. I will not create the repository or push.

## 4. Package names

Facts, checked on 2026-11-05 and not rechecked since: `phone-use` is free on crates.io and on PyPI, and taken on npm. Names for the other four crates were not checked.

Recommendation.
- crates.io: `phone-use`, plus `phone-use-core`, `phone-use-android`, `phone-use-ios`, `phone-use-video`. Check all five on the day you publish. A crates.io name cannot be removed or renamed after publishing.
- PyPI: reserve `phone-use` only when a Python wrapper exists. Today there is none, so do nothing.
- npm: use a scope, for example `@<owner>/phone-use`. Same rule: publish only when a wrapper exists.

Decision needed. Write "crates.io only" or "crates.io plus PyPI and npm". Name the npm scope if you want one.

## 5. Platforms

Facts. Only macOS arm64 was built. The Linux and Windows targets have no recorded compile. The iOS Simulator tier and Vision OCR work on macOS only. Android works through adb on every host in principle, and was driven from Windows in the M1 and M2 benchmarks.

Recommendation. Release macOS arm64 and macOS x86_64 first. Add Linux and Windows after a CI run compiles them and the Android scenario suite passes on each. Say so in the README.

Decision needed. Write "macOS only first" or "all five targets in 0.1.0".

## 6. Release notes

Draw only from `docs/bench/*`. Do not state a number that has no file behind it.

- M1 (`docs/bench/m1.md`, Android emulator API 37 on Windows): phone-use is faster than phone-harness 0.3.3 on every operation phone-harness supports, by 3x to 150x at p50. Three of five budgets met. The tap budget (15 ms) and the type-100-characters budget (150 ms) are missed: tap p50 was 29.7 and 36.5 ms, and typing took 253 and 288 ms (see D14).
- M2 (`docs/bench/m2.md`): the 20-scenario suite passed on API 26 three times in a row (60 of 60). The API 37 gate is open: two clean runs, then the emulator stopped answering at `testbed-web-form` on three cold boots.
- M4 (`docs/bench/m4.md`, iOS 26.5 Simulator, not iOS 27): 12 of 12 scenarios, three runs in a row. Four of five budgets met. Tap p50 was 357 ms against an 80 ms budget (D15). The runner's snapshot is about 2x faster than fully tuned WebDriverAgent. One flake in `settings-dark-appearance` has no known cause.
- Not measured and not claimed: any physical Android phone, any physical iPhone or iPad, the AndroidWorld success-rate comparison, Android tablets.
- Known gaps: the list in architecture section 14.

Recommendation. Publish the misses next to the wins, in the same section. The 8.1 plan says to publish whatever the numbers say.

Decision needed. Tell me whether to hold the release until task 8.1 has numbers, or to release without a "same model, higher success" claim. Recommendation: release without that claim.

## 7. Before the push, in order

1. Decisions 1 to 5 answered.
2. Add `LICENSE`, `repository`, `license`, and `description` fields. Run `cargo package --list` for each crate and read the file list (no `.audit/`, no benchmark data you do not want public).
3. Add the reproducibility check (section 2) and run it.
4. Build all configured targets with CI or hosts you approve. Run `phone-use version`, `doctor`, `skill` from a clean directory on each.
5. Run the packaged binary against an emulator: `phone-use devices`, then one `snapshot`. This was not run for the macOS arm64 artifact because the emulators were in use.
6. Read the README install section again. It still says "phone-use is not published".
7. You create the remote, push, tag, and publish. I do none of these.
