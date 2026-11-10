| op | tool | n | p50 ms | p95 ms |
|---|---|---|---|---|
| tap | phonedriver | 50 | 47.5 | 201.2 |
| tap | phone-harness | 50 | 143.5 | 381.9 |
| tap | adb | 50 | 135.3 | 191.1 |
| tap, paced | phonedriver | 50 | 37.7 | 180.9 |
| tap, paced | phone-harness | 50 | 279.2 | 898.7 |
| tap, paced | adb | 50 | 99.7 | 467.6 |
| screenshot | phonedriver | 50 | 10.6 | 32.8 |
| screenshot | phone-harness | 50 | 553.4 | 1412.8 |
| screenshot | adb | 50 | 477.4 | 1098.0 |
| tap, device side | phonedriver | 50 | 5.7 | 48.0 |
| tap, device side | phone-harness | 50 | 79.1 | 496.7 |
| tap, device side | adb | 50 | 66.9 | 122.0 |
| tap, then what changed | phonedriver | 50 | 627.6 | 1126.8 |
| tap, then what changed | adb | 50 | 3199.8 | 5625.3 |
| tree, Settings | phonedriver | 50 | 98.2 | 182.3 |
| tree, Settings | phone-harness | 50 | 2999.7 | 5615.9 |
| tree, Settings | adb | 50 | 2912.5 | 6824.2 |
| tree, Settings | phonedriver, settled | 50 | 100.5 | 921.5 |
| tree, list | phonedriver | 50 | 25.5 | 84.2 |
| tree, list | phone-harness | 50 | 2889.7 | 5668.4 |
| tree, list | adb | 50 | 2461.0 | 3512.2 |
| tree, list | phonedriver, settled | 50 | 25.9 | 119.8 |
| type 100 characters | phonedriver | 50 | 361.6 | 1114.3 |
| type 100 characters | phone-harness | 50 | 9720.1 | 13061.3 |
| type 100 characters | adb | 50 | 9419.6 | 13029.4 |
| type 100 characters | phonedriver, mixed Unicode | 50 | 387.5 | 977.9 |
| cold start, first screen read | phonedriver | 10 | 2378.0 | 8731.8 |
| cold start, first screen read | phone-harness | 10 | 3261.9 | 4165.4 |
| cold start, first screen read | adb | 10 | 2693.5 | 3706.5 |

| op | adb / phonedriver (p50) | 95% CI | phone-harness / phonedriver (p50) | 95% CI |
|---|---|---|---|---|
| tap | 2.8x | 2.2 to 3.6 | 3.0x | 2.3 to 3.7 |
| tap, paced | 2.6x | 1.9 to 3.4 | 7.4x | 5.7 to 8.9 |
| screenshot | 44.8x | 32.4 to 49.4 | 52.0x | 39.8 to 60.7 |
| tap, device side | 11.7x | 3.9 to 34.7 | 13.8x | 4.8 to 46.7 |
| tap, then what changed | 5.1x | 4.7 to 5.6 | not supported |  |
| tree, Settings | 29.7x | 25.3 to 32.6 | 30.6x | 27.0 to 35.1 |
| tree, list | 96.4x | 88.9 to 120.4 | 113.2x | 101.1 to 139.4 |
| type 100 characters | 26.0x | 21.9 to 32.6 | 26.9x | 23.7 to 33.9 |
| cold start, first screen read | 1.1x | 0.9 to 1.2 | 1.4x | 1.1 to 1.5 |
