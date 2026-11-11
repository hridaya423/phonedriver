| op | tool | n | p50 ms | p95 ms |
|---|---|---|---|---|
| tap | phonedriver | 50 | 38.5 | 175.6 |
| tap | phone-harness | 50 | 162.8 | 434.9 |
| tap | adb | 50 | 170.5 | 205.1 |
| tap, paced | phonedriver | 50 | 28.7 | 42.2 |
| tap, paced | phone-harness | 50 | 220.4 | 248.1 |
| tap, paced | adb | 50 | 69.4 | 89.5 |
| screenshot | phonedriver | 50 | 9.1 | 11.1 |
| screenshot | phone-harness | 50 | 348.3 | 715.0 |
| screenshot | adb | 50 | 291.2 | 763.7 |
| tap, device side | phonedriver | 50 | 0.2 | 15.0 |
| tap, device side | phone-harness | 50 | 46.6 | 196.9 |
| tap, device side | adb | 50 | 45.6 | 70.5 |
| tap, then what changed | phonedriver | 50 | 514.0 | 612.0 |
| tap, then what changed | adb | 50 | 2477.2 | 5460.4 |
| tree, Settings | phonedriver | 50 | 51.0 | 146.3 |
| tree, Settings | phone-harness | 50 | 2393.8 | 3017.7 |
| tree, Settings | adb | 50 | 2215.7 | 3054.8 |
| tree, Settings | phonedriver, settled | 50 | 49.3 | 61.0 |
| tree, list | phonedriver | 50 | 12.7 | 20.0 |
| tree, list | phone-harness | 50 | 2362.5 | 2398.1 |
| tree, list | adb | 50 | 2163.7 | 2204.5 |
| tree, list | phonedriver, settled | 50 | 12.4 | 17.3 |
| type 100 characters | phonedriver | 50 | 158.3 | 426.6 |
| type 100 characters | phone-harness | 50 | 3928.7 | 10064.9 |
| type 100 characters | adb | 50 | 3638.4 | 9234.2 |
| type 100 characters | phonedriver, mixed Unicode | 50 | 160.6 | 384.7 |
| cold start, first screen read | phonedriver | 10 | 1176.0 | 3014.6 |
| cold start, first screen read | phone-harness | 10 | 2776.6 | 5095.1 |
| cold start, first screen read | adb | 10 | 2248.4 | 7443.5 |
| tree, Settings (first pass) | phonedriver | 50 | 51.5 | 85.4 |
| tree, Settings (first pass) | phone-harness | 50 | 2393.3 | 2642.1 |
| tree, Settings (first pass) | adb | 50 | 2198.2 | 2252.1 |
| tree, Settings (first pass) | phonedriver, settled | 50 | 47.5 | 68.8 |
| tree, list (first pass) | phonedriver | 50 | 11.3 | 17.3 |
| tree, list (first pass) | phone-harness | 50 | 2357.7 | 3245.0 |
| tree, list (first pass) | adb | 50 | 2189.0 | 3519.1 |
| tree, list (first pass) | phonedriver, settled | 50 | 12.4 | 124.4 |

| op | adb / phonedriver (p50) | 95% CI | phone-harness / phonedriver (p50) | 95% CI |
|---|---|---|---|---|
| tap | 4.4x | 3.0 to 5.2 | 4.2x | 3.0 to 5.1 |
| tap, paced | 2.4x | 2.3 to 2.6 | 7.7x | 7.2 to 8.3 |
| screenshot | 32.0x | 30.9 to 33.2 | 38.2x | 36.9 to 39.8 |
| tap, device side | 237.3x | -399.2 to 2295.2 | 242.8x | -406.1 to 2352.2 |
| tap, then what changed | 4.8x | 4.7 to 5.2 | not supported |  |
| tree, Settings | 43.5x | 41.1 to 48.0 | 47.0x | 44.3 to 51.7 |
| tree, list | 170.0x | 162.0 to 177.4 | 185.6x | 176.8 to 193.4 |
| type 100 characters | 23.0x | 19.7 to 25.1 | 24.8x | 21.2 to 26.9 |
| cold start, first screen read | 1.9x | 1.4 to 2.4 | 2.4x | 1.7 to 2.6 |
| tree, Settings (first pass) | 42.7x | 40.4 to 45.7 | 46.5x | 44.1 to 49.9 |
| tree, list (first pass) | 193.1x | 181.0 to 225.2 | 208.0x | 194.5 to 225.2 |
