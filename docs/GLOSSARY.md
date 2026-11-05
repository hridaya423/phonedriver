# Glossary

Use these words in code, docs and replies. One meaning each.

- **Agent**: the AI using phone-use. Not the on-device process.
- **Device agent**: the Java process (`phone-use-agent.jar`) running on an Android device via `app_process`.
- **Runner**: PhoneUseRunner, the XCTest bundle on an iOS device or simulator.
- **Daemon**: the long-lived `phone-use daemon` process on the host.
- **Device**: one phone, tablet, emulator or simulator, addressed by a device id like `android:emulator-5554`.
- **Tier**: how an iOS device is driven: CoreDevice, Runner, or Simulator.
- **Lease**: a client's exclusive claim on a device.
- **Session**: the daemon's state for one lease: ref table, last tree, settle detector.
- **Element**: one node of the pruned UI, with a ref.
- **Ref**: `@eN`, a session-stable name for an element.
- **Snapshot**: the rendered list of elements on screen.
- **Observation**: a snapshot or a diff, plus the header.
- **Diff**: `+`, `-`, `~` lines describing what changed since the last observation.
- **Settle**: the UI stopped changing meaningfully; computed by the settle detector.
- **Target**: what an action points at: a ref, a point, or text.
- **Point**: a logical-point coordinate (iOS points, Android dp).
- **FrameStore**: the latest decoded frame, its thumbnail, and its sequence number.
- **Testbed**: our own app on each platform whose UI and event log are the ground truth for tests.
- **NeedsUser**: an error meaning only a human can continue (unlock, trust, enable a setting).
