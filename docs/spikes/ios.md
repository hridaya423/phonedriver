# iOS spikes

## I4: WebDriverAgent baseline and runner snapshot technique (2026-11-05)

**Setup.** Mac mini, Xcode 27.0 (27A266a), iOS 27.0 simulator SDK, iPhone 17 simulator running iOS 26.5 (the only simulator runtime on the mini). WebDriverAgent `3d6a4e0` built with `xcodebuild -scheme WebDriverAgentRunner test` (WDA 16.15.3). Client on the same Mac over loopback. Screens: testbed List tab (`-tab list`), testbed Taps tab (`-tab taps`), Settings root (`com.apple.Preferences`). Each row: 2 warm-up calls, then 20 timed calls; p50 and p95 in milliseconds. Spike code was `spikes/ios-runner-snapshot/` (`time_wda.py` and an Objective-C XCTest), deleted at the end of the phase.

### WDA `GET /session/:id/source` (HTTP round trip, client side)

| Screen | Settings | p50 | p95 | Elements | Tree depth |
|---|---|---|---|---|---|
| List | default (`snapshotMaxDepth` 50) | 380.7 | 388.0 | 150 | 12 |
| List | `snapshotMaxDepth` 30 | 380.7 | 383.9 | 150 | 12 |
| List | depth 30, `waitForIdleTimeout` 0, `animationCoolOffTimeout` 0, `excluded_attributes=visible,accessible,hittable` | **45.0** | 45.4 | 150 | 12 |
| List | same, `format=json` | 193.7 | 198.7 | 150 | |
| Taps | default | 109.2 | 110.2 | 59 | 8 |
| Taps | depth 30 | 109.1 | 109.6 | 59 | 8 |
| Taps | depth 30, idle waits off, excluded attributes | **26.5** | 27.3 | 59 | 8 |
| Taps | same, `format=json` | 83.7 | 85.3 | 59 | |
| Settings | default | 476.7 | 482.6 | 168 | 21 |
| Settings | depth 30 | 477.0 | 481.5 | 168 | 21 |
| Settings | depth 30, idle waits off, excluded attributes | **49.9** | 50.2 | 168 | 21 |
| Settings | same, `format=json` | 218.4 | 220.5 | 168 | |

An earlier pass (same build, 20 runs) also tried smaller depths: `snapshotMaxDepth` 15 cuts Settings to 25 of 168 elements (72 ms) and 10 cuts the List tab to 122 of 150 elements, so depth below about 21 loses real content on stock apps. Depth 30 is the smallest round value that keeps every tree complete; it does not change the time when the tree is shallower than 30.

What dominates WDA's time is the per-element `visible`, `accessible` and `hittable` attributes, not the depth: excluding them cuts 380 ms to 45 ms on the List tab. The JSON format is slower than XML because it computes more per element.

### In-process snapshot inside an XCTest (no transport)

`XCAXClient_iOS` from `[XCUIDevice.sharedDevice accessibilityInterface]`, element = first of `activeApplications`, `requestSnapshotForElement:attributes:parameters:` with `defaultParameters` plus `maxDepth`. The reduced attribute list comes from `+[XCElementSnapshot axAttributesForElementSnapshotKeyPaths:isMacOS:]` with `label, value, identifier, frame, elementType, enabled, selected, hasFocus, placeholderValue`, which returns an `NSSet` of 12 AX names (pass `allObjects`; an `NSSet` raises `array argument is not an NSArray`). Enabled and selected travel inside `XC_kAXXCAttributeTraits`. The standard list (`sanitizedElementSnapshotHierarchyAttributesForAttributes:nil`) is 22 names and is already in AX form. Defaults on this Xcode: `maxDepth`, `maxChildren`, `maxArrayCount` all `INT_MAX`.

| Screen | Variant | p50 | p95 | Nodes | Depth |
|---|---|---|---|---|---|
| List | public `-[XCUIApplication snapshotWithError:]` | 34.2 | 34.8 | 150 | 12 |
| List | AX, standard attributes, depth 30 | 24.9 | 25.0 | 150 | 12 |
| List | AX, reduced attributes, depth 30 | 20.7 | 21.0 | 150 | 12 |
| List | AX reduced + flatten to `RawNode` + JSON | **21.2** | 21.5 | 150 | 12 |
| Taps | public `snapshotWithError:` | 19.0 | 19.4 | 59 | 8 |
| Taps | AX standard, depth 30 | 13.5 | 13.7 | 59 | 8 |
| Taps | AX reduced, depth 30 | 11.8 | 12.4 | 59 | 8 |
| Taps | AX reduced + flatten + JSON | **12.2** | 13.9 | 59 | 8 |
| Settings | public `snapshotWithError:` | 37.6 | 38.0 | 168 | 21 |
| Settings | AX standard, depth 30 | 28.2 | 28.6 | 168 | 21 |
| Settings | AX reduced, depth 30 | 23.0 | 23.9 | 168 | 21 |
| Settings | AX reduced + flatten + JSON | **23.7** | 23.9 | 168 | 21 |

Flattening to `RawNode` and encoding JSON costs under 1 ms at these sizes.

### What this sets for D4

Two readings of "WDA with `snapshotMaxDepth` tuned":
- Depth only (the literal D4 wording): 381 / 109 / 477 ms. The technique beats it by 9x to 20x.
- Every WDA knob a careful user would turn (depth 30, idle waits off, `excluded_attributes`): 45.0 / 26.5 / 49.9 ms. Half of that is 22.5 / 13.3 / 25.0 ms. The in-process technique (21.2 / 12.2 / 23.7 ms) is just under half before any transport cost, so the runner can meet 2x only if its transport and request handling add about 1 ms.

We hold the runner to the stricter bar. Task 5.3 measures it end to end over TCP.
