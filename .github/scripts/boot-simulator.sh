#!/usr/bin/env bash
# Boots the newest iOS runtime's first iPhone. A device from a fresh image sets itself up for minutes after its first
# boot (widgets, health, push and the like grow its data from 1.2 GB to 1.9 GB), so the job keeps the data of a
# device that has run the replay and puts it back here before later boots. Writes $RUNNER_TEMP/udid when it has
# chosen and $RUNNER_TEMP/booted when it is done.
set -euo pipefail

kept="$RUNNER_TEMP/simulator"
udid=$(xcrun simctl list devices available -j | jq -r '
  .devices | to_entries
  | map(select(.key | test("SimRuntime\\.iOS-")))
  | sort_by(.key | capture("iOS-(?<v>[0-9-]+)$").v | split("-") | map(tonumber))
  | reverse | map(.value[] | select(.name | startswith("iPhone"))) | .[0].udid')
echo "$udid" > "$RUNNER_TEMP/udid"
device="$HOME/Library/Developer/CoreSimulator/Devices/$udid"

if [ -d "$kept/$udid/data" ]; then
  rm -rf "$device/data"
  cp -c -R "$kept/$udid/data" "$device/data"
  echo "restored the device's data: $(du -sh "$device/data" | cut -f1)"
fi
xcrun simctl bootstatus "$udid" -b
touch "$RUNNER_TEMP/booted"
