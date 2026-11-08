# Android over Wi-Fi (task 7.4)

No physical phone was available, so this is a wireless-debugging session on an emulator, not a measurement of a radio link.

## Setup that worked (`Pixel_Tablet_API_37`, Android 17, `emulator-5558` on win-pc)

1. Wireless debugging was switched on from Settings > Developer options with phonedriver itself; System UI asked "Allow wireless debugging on this network?" (SSID `AndroidWifi`), and `tap "Allow"` from a `snapshot` of that dialog accepted it. The screen then showed `IP address & Port 10.0.2.18:38221`.
2. "Pair device with pairing code" showed a six-digit code and `10.0.2.18:35457`.
3. The emulator's guest network (10.0.2.x) is not reachable from the host, so on win-pc: `adb -s emulator-5558 emu redir add tcp:36457:35457` and `... tcp:38221:38221`.
4. On win-pc: `adb pair 127.0.0.1:36457 <code>` (`Successfully paired`) and `adb connect 127.0.0.1:38221` (`connected`). `adb devices -l` listed `127.0.0.1:38221 device`, a TCP transport next to `emulator-5558`.

## Evidence

`ADB_SERVER_SOCKET=tcp:127.0.0.1:5038 PHONEDRIVER_TEST_DEVICE=android:127.0.0.1:38221 BLAZE_TESTS=all blaze test -p phonedriver --test e2e -- --ignored --test-threads=1`: `10 passed` (agent deploy, snapshot, tap on point and ref within 1 dp, Unicode type verified, secrets, secure screen, screenshot size, busy screen, fixture save), including installing the testbed through the TCP transport.

`phonedriver-bench --serial <id> --runs 20 --warmup 3`, one invocation each, same tablet, same adb server (reached over an SSH tunnel from the Mac), p50 / p95 in ms:

| op | emulator transport | wireless debugging transport |
|---|---|---|
| tap (point, request to delivered) | 144.5 / 239.6 | 174.1 / 355.2 |
| tap (ref) | 276.1 / 352.9 | 313.1 / 395.2 |
| screenshot (settled) | 637.5 / 836.3 | 691.9 / 861.3 |
| tap to settled diff | 667.7 / 875.5 | 617.7 / 898.1 |
| snapshot, Settings, no settle | 126.8 / 305.4 | 133.8 / 313.9 |
| snapshot, List tab | 56.2 / 135.1 | 55.3 / 144.4 |
| type 100 chars | 669.6 / 1045.0 | 611.1 / 1164.6 |

Both columns include the SSH tunnel and a 2560x1600 software-rendered display, so none of the budgets in architecture section 12 is met here (they were set for a USB phone) and the columns differ by less than the run-to-run spread except for tap (about 30 ms more). What this shows: phonedriver needs nothing beyond `adb pair` and `adb connect`, and the TCP transport costs little on top of the other. It does not show what a phone on a real Wi-Fi network costs; that needs a phone.
