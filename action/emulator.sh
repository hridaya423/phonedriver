#!/usr/bin/env bash
set -euo pipefail

adb="$ANDROID_HOME/platform-tools/adb"
log="$RUNNER_TEMP/emulator.log"
pid="$RUNNER_TEMP/emulator.pid"
started="$RUNNER_TEMP/emulator.started"
failed="$RUNNER_TEMP/emulator.failed"

booted() {
  [ "$("$adb" -s emulator-5554 shell getprop sys.boot_completed 2> /dev/null | tr -d '\r')" = 1 ] &&
    "$adb" -s emulator-5554 shell pm path android > /dev/null 2>&1
}

case "${1:-}" in
  start)
    if [ "$RUNNER_OS" != Linux ]; then
      echo "::error::the Android emulator needs a Linux runner, such as ubuntu-latest"
      exit 1
    fi
    if [ -f "$started" ]; then
      exit 0
    fi
    touch "$started"
    trap 'touch "$failed"' ERR
    echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' | sudo tee /etc/udev/rules.d/99-kvm4all.rules > /dev/null
    sudo udevadm control --reload-rules
    sudo udevadm trigger --name-match=kvm
    tools="$ANDROID_HOME/cmdline-tools/latest/bin"
    image="system-images;android-$API;google_apis;x86_64"
    export ANDROID_AVD_HOME="$RUNNER_TEMP/avd"
    mkdir -p "$ANDROID_AVD_HOME"
    yes 2> /dev/null | "$tools/sdkmanager" --licenses > /dev/null || true
    "$tools/sdkmanager" --install emulator platform-tools "$image" > /dev/null
    echo no | "$tools/avdmanager" create avd -n phonedriver -k "$image" -d pixel_6 --force > /dev/null
    if [ ! -f "$ANDROID_AVD_HOME/phonedriver.ini" ]; then
      echo "::error::avdmanager made no virtual device from $image"
      touch "$failed"
      exit 1
    fi
    nohup "$ANDROID_HOME/emulator/emulator" -avd phonedriver -port 5554 -no-window -no-audio -no-boot-anim \
      -no-snapshot -gpu swiftshader_indirect < /dev/null > "$log" 2>&1 &
    echo $! > "$pid"
    ;;
  wait)
    for _ in $(seq 300); do
      booted && break
      if [ -f "$failed" ] || [ ! -f "$started" ]; then
        echo "::error::the emulator could not be set up; see the start step's output"
        exit 1
      fi
      if [ -f "$pid" ] && ! kill -0 "$(cat "$pid")" 2> /dev/null; then
        echo "::error::the emulator stopped before it booted; its log:"
        tail -40 "$log"
        exit 1
      fi
      sleep 2
    done
    if ! booted; then
      echo "::error::the emulator did not boot in 10 minutes; its log:"
      tail -40 "$log"
      exit 1
    fi
    echo "PHONEDRIVER_DEVICE=android:emulator-5554" >> "$GITHUB_ENV"
    ;;
  *)
    echo "usage: emulator.sh start|wait" >&2
    exit 2
    ;;
esac
