#!/usr/bin/env bash
# Boots the newest iOS runtime's first iPhone in the background of the job. A first boot sets the device up for
# about four minutes on a 3-core runner, so its data is kept in $RUNNER_TEMP/simulator for the cache and put back
# before later boots. Writes $RUNNER_TEMP/udid when it has chosen and $RUNNER_TEMP/booted when it is done.
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
  xcrun simctl bootstatus "$udid" -b
else
  xcrun simctl bootstatus "$udid" -b
  xcrun simctl shutdown "$udid"
  mkdir -p "$kept/$udid"
  cp -c -R "$device/data" "$kept/$udid/data"
  echo "kept the device's data: $(du -sh "$kept/$udid/data" | cut -f1)"
  xcrun simctl bootstatus "$udid" -b
fi
touch "$RUNNER_TEMP/booted"
