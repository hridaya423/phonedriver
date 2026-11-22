---
title: How to replay an agent's run in CI with GitHub Actions
seo: Replay an AI agent's mobile app run in GitHub Actions on an Android emulator or iOS Simulator
nav: Replay in CI
description: Save what an agent did on your app as a steps file, replay it with no model, and run the files on every commit with the phonedriver GitHub Action on an Android emulator or an iOS Simulator.
meta: Save an agent's run on your app as a steps file and replay it on every commit with the phonedriver GitHub Action, on an Android emulator or the iOS Simulator.
updated: 2026-11-21
date: 2026-11-17
---

An agent that finds its way through your app once has written a test, if you keep what it did. `phonedriver save` writes the session's steps to a file, `phonedriver replay` runs them again with no model, and the phonedriver GitHub Action replays the files on an emulator or a Simulator on every commit.

## Save a run

After the agent (or you, with the CLI) has walked a flow that works:

```sh
phonedriver save checkout.steps
```

`save` lists every step it wrote, one a line, so a step that was a wrong turn can be deleted from the file. Only calls that worked are kept: taps, typing, waits, launches, shell commands, settings and permissions. An element is kept by its text and role, not by its `@ref`, so the file works in a later session and on another device with the same app. Save once before the flow to a file you throw away, so the second file holds only the flow.

A `wait` step is what makes a replay check something: `phonedriver wait "Order placed"` after the last tap fails the replay if the text never shows.

## Replay it

```console
$ phonedriver replay checkout.steps
replayed 9 steps in 12.4 s
```

A replay stops at the first step that fails, at a step that changed the screen when it was saved and now changes nothing, or at a `shell` step whose command exited with another status than the step expects (0, unless `save` kept the status the command had then, or you set `"expect_exit"` in the step). It prints the steps that ran, the screen as it is, and where to go on from:

```
error: step 6 of 9 (tap "Pay now") failed: no element matches "Pay now"
hint: Do step 6 yourself, then replay the same file from step 7 for the 3 steps left.
```

## Logins and fresh data

Text typed with `--secret` is never written to the file. `save` writes the step with the name of an environment variable instead, and lists it:

```console
$ phonedriver save login.steps
saved 3 steps to login.steps
1. tap "Sign in"
2. type secret $PHONEDRIVER_SECRET_1 into "Password"
3. tap "Continue"
the secret text is not in the file: set PHONEDRIVER_SECRET_1 before replaying (rename a variable in the file if you like)
```

A replay reads that variable from the environment of the process that runs it and types it as a secret, so it is hidden in replies and traces like any `--secret` text. If the variable is not set, the replay stops at that step and the hint names it. A renamed variable must still begin `PHONEDRIVER_SECRET_`, so a steps file cannot make a replay type another variable. With MCP, put the variable in the MCP server's `env`, not in a tool argument.

A replay types a secret only into a password field or a one-time code field. A steps file is text an agent can edit, and a secret typed into a plain field shows on the screen, where a screenshot can read it. If your flow types a secret into a plain field, such as an API key in a settings form, set `PHONEDRIVER_REPLAY_SECRET_ANY_FIELD=1` where the replay runs: in the job's `env`, or the MCP server's.

`{{name}}` in a step's text is filled from `--var name=value` (MCP: `vars`), and `{{run}}` is a short id new in every replay, for data that must be fresh, such as `test+{{run}}@example.com`. A variable the file uses and nobody gave is refused before the first step, with every missing name listed. Variables are not secrets and may show in replies; use the environment variable for a secret. In a `shell` step a value is shell text: `--var app='com.x; pm clear com.other'` runs two commands. Quote the place in the file, `am start -n '{{app}}/.Main'`, and give only values you trust. `save` never writes variables: edit them into the file where a value should change.

Over MCP, `replay` with `save: true` writes only a `.steps` file inside the server's working directory, and it replaces a file there only if that file is already a steps file.

```console
$ PHONEDRIVER_SECRET_1=hunter2-test phonedriver replay signup.steps --var plan=pro
replayed 6 steps in 14.2 s (run 3fa91c07)
```

## Check what each screen shows

A replay checks that the taps land and the waited-for text shows. `--screens` also checks what each step left on the screen:

```console
$ phonedriver replay settings.steps --screens screens/settings
replayed 5 steps in 10.5 s
kept 5 screens in screens/settings; a replay with --screens screens/settings compares each step's screen with them
```

The first replay keeps a screenshot after each step. Later replays compare each step's screen with its screenshot and stop at the first step whose text changed, with the text that changed:

```
error: step 5 of 5 (wait "Name") failed: its screen is not the one in screens/settings/05.png
changed: 2 regions, 35.7% of the screen
~ "General" → "About" at 201,85
+ "Name" at 59,161
hint: If the new screen is right, delete screens/settings/05.png and replay again to keep it.
```

Pixels that change with no text in them, such as a pressed row or a scroll bar, do not stop a replay, and the status bar is left out: its clock, notification icons and battery change on their own. A screen that differs is looked at again until it stops moving, so an animation that had not finished does not fail the job. Keep the screens on the device type CI runs, since a screenshot of another size stops the replay.

## Run the files in GitHub Actions

The phonedriver repository is also an Action. It starts an Android emulator on a Linux runner or an iOS Simulator on a macOS runner, installs your build, replays each file and fails the job at the first run that stops:

```yaml
name: replay
on: [push, pull_request]
jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: ./gradlew assembleDebug
      - uses: hridaya423/phonedriver@v0.1.7
        with:
          app: app/build/outputs/apk/debug/app-debug.apk
          runs: e2e/*.steps
          record: true
          vars: |
            plan=pro
        env:
          PHONEDRIVER_SECRET_1: ${{ secrets.APP_PASSWORD }}
```

For iOS, use a macOS runner, `platform: ios` and a Simulator `.app` built with `xcodebuild -sdk iphonesimulator`. `device` picks the Simulator; the default is the first iPhone the runner has.

Each run's result is a row in the job summary. The reply of every run, and the screen and app logs of a run that stopped, go in an artifact; with `record: true` the artifact has a video of every run too.

| Input | Default | What it does |
|---|---|---|
| `runs` | | Steps files, one path or glob a line |
| `platform` | `android` | `android` (Linux runner) or `ios` (macOS runner) |
| `app` | | An `.apk` or a Simulator `.app` to install first |
| `api-level` | `35` | The Android system image |
| `device` | first iPhone | The Simulator to boot |
| `record` | `false` | `true` keeps a video of every run |
| `vars` | | Values for `{{name}}` in the files, one `name=value` a line |
| `screens` | | A folder of kept screens, a subfolder for each run named as its steps file. A run with no subfolder keeps its screens in the artifact, under `screens/`, for you to commit |
| `version` | `latest` | The phonedriver release to install |
| `artifact` | `phonedriver-<job>` | The artifact's name |

The outputs `passed` and `failed` count the runs.

## Limits

- Text that changes on its own, such as free space, a date or the keyboard's suggestions, stops a replay with `--screens`. Use it on flows whose screens stay the same from one replay to the next.
- A replay finds each element again by its text. A run saved on one language or screen size can miss on another; save the run on the device size and language CI uses.
- The Action runs the Android emulator only on Linux runners, which have hardware acceleration; GitHub's macOS runners cannot run it.
- A step that waited on something outside the app (an email, a push notification) has nothing to wait on in CI.
