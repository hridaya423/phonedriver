# Tablets and large screens (task 7.3, iOS half)

Device: simulator `iPad Pro 13 phone-use`, type `iPad-Pro-13-inch-M5-16GB`, iOS 26.5, created on the Mac mini with `xcrun simctl create` (no new runtime). Screen 1032x1376 pt portrait, 1376x1032 pt landscape. The Android tablet half of 7.3 is not done here (the win-pc emulators were in use by another agent).

## Rotating the simulator

`simctl` has no rotate verb, and Xcode 27 ships no Simulator.app (DeviceHub.app opened with no window to drive over ssh). Rotation here came from a throwaway XCUITest bundle (`~/phone-use-work/spikes/rotate` on the mini, not in the repo) that sets `XCUIDevice.sharedDevice.orientation` and exits; the orientation stays after the test ends. The product has no rotate verb; this stays a test fixture. Upside-down did not take effect on the testbed (header stayed portrait), so it is not claimed.

## Gaps found and fixed

1. Header and swipes used the size from connect. After rotation `phone-use snapshot` printed `1032x1376pt · portrait` while the screenshot was 1376x1032. Fix: the screen comes from the application element's frame on each snapshot; the daemon asks the device (`Device::current_screen`, default `None` so Android keeps its `DisplayChanged` events). Test: `daemon::the_header_follows_a_device_that_reports_its_own_rotation`; map unit test `the_screen_is_the_application_frame`.
2. Taps landed in the wrong place in landscape. Testbed log with the device in landscape: `tap B5` at (519, 438) logged `touch began x 438 y 513`; with the fix `x 519 y 438`, `{"ev":"tap","id":"b5"}`. Checked in landscape left, landscape right and portrait: the logged touch equals the requested point and the right button logs (`B17` at 519,630; `B9` at 519,502; `B2` at 642,546). Fix in `ios-runner/Runner/Gesture.m`.
3. The testbed supported only portrait. `UISupportedInterfaceOrientations~ipad` now lists all four; the iPhone list is unchanged.

## Gaps recorded, not fixed

- `bench/scenarios/ios/testbed-list.yaml` assumes two swipes reach "Row 45", which holds on a 874 pt phone and not on a 1376 pt tablet (two swipes end at rows 56 to 83). It is a scenario assumption, not a product defect: `scroll down --until "Row 45"` reaches the row in one call on the iPad.
- Split View and Slide Over: not checked.
- Rotation is not a product verb or MCP tool; an agent cannot rotate a simulator through phone-use.
