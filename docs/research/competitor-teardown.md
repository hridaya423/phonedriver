# phone-harness teardown

Source: https://github.com/phone-harness/phone-harness at commit `4bebff1` (v0.3.3), read on 2026-11-05.
Every claim below names the file and line it came from. Re-clone that commit to check any of them.

## What it is

A Python CLI. The agent pipes a Python script on stdin; `run.py` executes it with helper functions pre-imported.

- iPhone: captures the macOS iPhone Mirroring window with `screencapture`, writes a PNG to disk, runs Apple Vision OCR on the file, and posts synthetic Mac mouse and keyboard events into the window.
- Android: one `subprocess.run(["adb", ...])` per operation. `screencap -p` for pixels, `uiautomator dump` for the tree, `input tap|swipe|text|keyevent` for input.
- Cloud: an HTTPS op endpoint (`remote.py`) and a 1,931-line upsell CLI (`cloud.py`).

## Defects, worst first

### 1. Telemetry ships user content, on by default

`src/phone_harness/telemetry.py:262-268` sends `task` (the whole script, up to 20,000 chars), `output` (last 20,000 chars of stdout), and `steps` (every helper call with its arguments, `run.py:91`, truncated to 300 chars) to PostHog. Nothing is redacted. `type_text("hunter2")` sends the password. `print(screen())` sends whatever is on the phone. It is opt-out (`DO_NOT_TRACK=1` or config).

Our answer: no telemetry. See decision D9.

### 2. A blocklist posing as a sandbox

`run.py:391` `_host_reaching` strips modules, classes and `Path` objects from the script globals for Hermes and OpenClaw clients. Builtins remain, so `().__class__.__base__.__subclasses__()` reaches `os` in one line. The script still runs with full user privileges.

Our answer: never execute agent-written code. The agent calls typed operations. See decision D6.

### 3. iOS reads pixels from a video of the phone

`ios.py:1-23` documents it: no tree, no back, no app list, no current app. OCR sees text, not icons (`README.md` Limits). One phone per Mac. Unlocking the phone ends the session. About 750 lines of `ios.py` manage the Mirroring app's connect interstitials.

- `mirror.py:571-586` posts events at `kCGHIDEventTap`, which moves the user's real cursor.
- `background.py` uses private SkyLight symbols that can break on any macOS update (`ios.py:31-44` falls back when they do).
- `mirror.py:741-768` types by overwriting the user's Mac clipboard and leaves the text there on purpose.

Our answer: talk to the phone itself over Apple's device services. See decision D3.

### 4. Android uses the slowest path for every operation

- `android.py:430` taps with `adb shell input tap`. Each call spawns an `adb` client process on the host and an `app_process` JVM on the phone. Typical cost is 100 to 400 ms per tap.
- `android.py:380` captures with `adb exec-out screencap -p`, which PNG-encodes the full-resolution frame on the phone for every look.
- `android.py:677-711` reads the tree with `uiautomator dump`, which waits for the UI to go idle. It takes seconds and refuses any screen that keeps animating ("could not get idle state").
- `android.py:493` types with `input text`, which only handles ASCII. No emoji, CJK, or RTL text.
- No multi-touch anywhere (`README.md` Limits).

Our answer: a resident on-device agent with direct system-service calls. See decision D2.

### 5. Documented features that do not work

- `transport.py:140` promises two devices in one process. `android.py:202` writes the serial into the process-wide `os.environ["ANDROID_SERIAL"]`, so a second `Android(serial=...)` redirects the first.
- `mirror.py:548` and `background.py:133` capture to a fixed `/tmp/phone-harness/window.png`. Two concurrent runs overwrite each other's capture.

Our answer: per-device actors that own their state, unique artifact paths. See architecture "Daemon".

### 6. Waiting is guessing

- Fixed sleeps after navigation: `ios.py:117,121` (0.8 s), `ios.py:128-134` (0.9 s + 1.2 s for every app launch), `android.py:510-518`.
- `helpers.py:642` `wait_stable` takes a full PNG every 0.5 s and compares MD5 hashes. A blinking caret or a spinner defeats it; the 0.5 s floor taxes every action.

Our answer: settle detection from UI events and decoded-frame difference. See architecture "Settle".

### 7. Weight that is not product

- `cloud.py`, the largest file, sells the hosted service.
- `tests/test_pricing_copy.py` tests prices quoted in docs.
- README is mostly UTM-tagged banner images.

## What they got right (keep these ideas)

- An op vocabulary named for meaning, not mechanism (`transport.py:1-80`).
- Raising `Unsupported` instead of faking a capability (`transport.py:271`).
- Tagging each text box with `source: "tree" | "pixels"` (`transport.py:205-211`).
- A scroll must not fling: touch down, move, hold, then lift (`android.py:444-478`). Without the hold, lists coast past rows between looks.
- Typing by paste is exact; typing by keystrokes goes through autocorrect (`mirror.py:771-779`).

These are recorded in `docs/architecture.md` where we use them.
