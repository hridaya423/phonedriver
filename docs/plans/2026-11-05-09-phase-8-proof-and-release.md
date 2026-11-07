# Phase 8: Proof and release

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans`. This phase ends at a one-way door: stop before publishing anything.

**Goal:** Show with a public benchmark that phonedriver makes the same model succeed more often, faster, and cheaper than phone-harness, then prepare a release for the user to approve.

**Tech Stack:** AndroidWorld, `cargo-dist`.

---

### Task 8.1: AndroidWorld adapter (dropped)

**Dropped by the user on 2026-11-06:** "we dont care about doing androidworld. let's just make it the best harness." The top AndroidWorld scores measure whole agent systems, not the tool layer, and a fair run costs paid model calls. Proof of quality comes instead from the M1 to M5 benchmarks, the adb baseline, the agent-experience audit and its replays. The original task text stays below for the record.

**Files:** Create `bench/androidworld/` with a README and an adapter.

AndroidWorld (https://github.com/google-research/android_world) runs 116 tasks on an Android emulator and checks success programmatically. Read its agent interface first and record its version and commit in the README.

Check for conflicts before writing the adapter: AndroidWorld installs its own accessibility forwarding app and talks to the emulator console over gRPC, and it may expect a specific AVD. Our agent's UiAutomation connection and its accessibility service must coexist; if they cannot, run our agent with the forwarder disabled and record that the comparison changed AndroidWorld's default setup.

**Stop for approval before the full run.** 3 runs × 116 tasks × 2 harnesses is about 700 agent episodes. Estimate tokens per episode from a 10-task pilot, multiply out, and give the user the expected API cost and wall time. Run the full suite only after a yes. Write two thin agents for it, identical in prompt and model (`claude-sonnet-5-5`), differing only in the device tool layer:
- **phonedriver**: the MCP tools from Phase 4.
- **phone-harness**: its documented helpers, with telemetry off (`DO_NOT_TRACK=1`, `PHONE_HARNESS_TELEMETRY=0`).

Run the full suite three times per harness on `Pixel_API_37` (or the AVD AndroidWorld requires; record any difference). Report success rate with a 95% confidence interval (Wilson), median steps, tokens, and wall time per task in `docs/bench/androidworld.md`. Read the failure transcripts of both and classify each failure: harness (wrong tap, missed element, timeout) vs model (wrong plan). Fix harness-caused failures in phonedriver and rerun.

The claim we want to make is "same model, higher success, fewer tokens, less time." Publish whatever the numbers say, including any category where phone-harness wins.

### Task 8.2: Packaging

`cargo-dist` for macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64). The binary embeds the Android agent jar. The iOS runner ships as source plus a build command (signing is per user). The release workflow builds the agent jar in a step (`.github/workflows/release-build-setup.yml`) and passes it as `PHONEDRIVER_AGENT_JAR`; the runner sources are compiled into the binary. Installers: shell, PowerShell, Homebrew (local half done 2026-11-06, see `progress.md`). Test each artifact on a clean machine or VM: install, `phonedriver doctor`, Android e2e against the emulator.

### Task 8.3: Release checklist (stop here)

Prepare, do not execute:
- License (decision D13): ask the user. Also the `openh264` patent question in D13.
- How the Android agent jar ships with `cargo install`: crates.io builds have no Android SDK, so either commit the built jar with a CI check that rebuilding it reproduces the same bytes, or publish only prebuilt binaries. Recommend the committed jar.
- GitHub repository name and owner: ask the user. `github.com/phone-use` is taken; options are the user's account or a new org.
- crates.io `phone-use` (free at research time), PyPI `phone-use` (free), npm (taken; use a scope).
- Release notes from `docs/bench/*`.

Present the checklist to the user with a recommendation for each item and wait.
