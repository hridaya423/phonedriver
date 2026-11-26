# Contributing to phonedriver

Bug reports, test results from phones I don't own, and pull requests are all welcome. This page says how to build phonedriver, what a change needs before I merge it, and where each kind of report goes.

## Where things go

- **A bug or a device that does not work:** [open an issue](https://github.com/hridaya423/phonedriver/issues/new/choose). The form asks for `phonedriver doctor`'s output, which answers most of my first questions.
- **A security problem:** report it in private, as [SECURITY.md](SECURITY.md) says. Do not open a public issue.
- **A question, or something your agent did on a phone:** [Discussions](https://github.com/hridaya423/phonedriver/discussions).
- **An idea:** an issue is fine. Say what you were doing when you wanted it. A proposal that comes with a measurement, as [#41](https://github.com/hridaya423/phonedriver/issues/41) and [#52](https://github.com/hridaya423/phonedriver/issues/52) did, gets built faster.

## Build it

You need Rust (stable). The Android helper is a small Java program that the build compiles, so the default build also needs JDK 17 and the Android SDK's build-tools 37.0.0 and platform `android-37.0`:

```sh
sdkmanager "build-tools;37.0.0" "platforms;android-37.0"
cargo build -p phonedriver
```

When a tool is missing the build stops and lists what it looked for. Two ways around the SDK:

```sh
ANDROID_HOME=<sdk directory> cargo build -p phonedriver      # an SDK somewhere else
cargo build -p phonedriver --no-default-features             # iOS only, no Android helper
```

The iOS runner (`ios-runner/`) is built by Xcode on a Mac the first time phonedriver connects to a Simulator or an iPhone.

## Check it

These are the commands CI runs:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --no-fail-fast
```

The tests need no phone: they run the real daemon against a fake device (`crates/core/src/testing.rs`). Tests that need an emulator or a Simulator are marked `#[ignore]` and say which device they want.

## What a pull request needs

1. **A test that failed before your change.** Write the test first, run it, and paste the failure in the pull request. Then the fix, then the passing run.
2. **Proof on a device, when the change touches one.** A fake proves the logic; it does not prove what a phone does. Run the change on an emulator, a Simulator or a phone and say which, with the command and what it printed. The testbed apps in `testbeds/` log every touch they receive, so "the tap landed" can be read from the app, not from phonedriver's own reply.
3. **The limits.** Say what you did not test. "Not run on iOS" is a fine sentence to write.

Keep to the code's habits: no comments except public API docs and outside constraints the code cannot express, no trait with one implementation, and no retry or fallback without the failure that made you add it written beside it.

A change to a design choice also changes `docs/decisions.md`: add an entry or amend the one you are changing, with the evidence. `docs/architecture.md` describes how the pieces fit.

## Three rules that do not bend

- **No telemetry.** phonedriver connects to the device, and to nothing else unless the person asked (the full list is in [SECURITY.md](SECURITY.md)). A test runs the whole tool in a sandbox that blocks every other connection.
- **Secrets stay secret.** Text given with `--secret` must never reach a log, a trace, an error or test output.
- **Other people's phones.** Do not put a serial number, a phone number, a Wi-Fi name or a screenshot of someone's personal screen in an issue, a test or a commit.

## Licence

phonedriver is AGPL-3.0-only. By opening a pull request you agree that your contribution is under that licence.
