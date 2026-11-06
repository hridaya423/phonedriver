# Android tablet (task 7.3, Android half)

AVD `Pixel_Tablet_API_37` (device `pixel_tablet`, image `system-images;android-37.0;google_apis;x86_64`, 2560x1600 at 320 dpi, 3 GB RAM, `-gpu swangle_indirect`, port 5558). Its home is `D:\avd` on win-pc because C: had 5 GB free and the emulator wants a 7 GB userdata partition (`ANDROID_AVD_HOME=D:\avd`, data partition set to 3G). The header reads `1280x800pt · landscape`; the natural orientation is landscape, so `user_rotation 0` is landscape and `1` is portrait (the phone scenario `rotation-mid-task` assumes the reverse; `bench/scenarios/android-tablet/rotation-mid-task-tablet.yaml` is the tablet version).

## Results (one run each, 2026-11-06)

| Run | Result |
|---|---|
| Full suite, landscape | 20 of 21; the failure is `rotation-mid-task`, whose phone assumption is wrong here (the header said `800x1280pt · portrait` after `user_rotation 1`, which is correct) |
| `rotation-mid-task-tablet`, portrait then landscape, a testbed tap logged in each | pass |
| Portrait suite (17 scenarios: without Chrome, Clock and Files first-run flows and the phone rotation scenario) | 15 of 17; `testbed-busy` and `testbed-web-form` timed out with an empty tree for 8 s after `am start` |

The first portrait attempts failed while win-pc's adb server died (`early eof`) and while the freshly booted tablet's System UI was not ready (`registerUiTestAutomationService` NullPointerException); the numbers above are from a tablet that had been up for 90 s. The two portrait failures are not explained; both were the first read after a cold start of a screen on a 2560x1600 software-rendered display.

## Two panes

Settings on this API 37 image is single pane on the tablet (the list is full width, the detail replaces it); the plan's example does not happen here. Files is two-pane: the roots list on the left and the folder on the right. `snapshot` lists both (`Recent`, `Images`, `Videos`, `Audio`, `Documents`, `Downloads`, `sdk_gphone64_x86_64`, then `Images off` ... `Files in Downloads`, `No items`) and I compared it with a screenshot of the same screen.

## Gaps

- `screenshot` taken within a few seconds of `launch Files` returned the app's splash frame while the tree already showed the loaded screen (video frames arrive slowly from this display); a second screenshot 4 s later was right. Not fixed.
- Split-screen was not tested.
