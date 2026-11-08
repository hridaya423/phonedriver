#!/usr/bin/env bash
set -uo pipefail

usage() {
  cat <<'EOF'
webview-hang.sh: one trial of the API 37 WebView hang experiment (docs/bench/history/m2.md).

Usage: bench/experiments/webview-hang.sh <arm> <trial-name> [iterations]

Runs on win-pc in MSYS bash, next to the emulator. Each trial cold-boots the AVD,
waits until the launcher has focus and the 1-minute load is under 2 (closing and counting System UI ANR dialogs), starts the captures, then runs <iterations> rounds
of the arm and checks after each round that `adb shell` still answers.

Arms:
  none     open the testbed Web tab, type and tap with `adb shell input` only (no UiAutomation client)
  dump     as none, plus `uiautomator dump` after the page loads (the platform's own UiAutomation client)
  agent    as none, plus three `phonedriver snapshot` reads (our agent: tree and video)
  webform  the scenario testbed-web-form through phonedriver-bench
  chrome   the scenarios in bench/scenarios/android-unstable (chrome-open-page) through phonedriver-bench
  suite    the whole Android scenario suite through phonedriver-bench (one run per iteration)

Environment (defaults in brackets):
  AVD [Pixel_API_37]  SERIAL [emulator-5554]
  EMU_FLAGS [-no-snapshot -no-audio -no-boot-anim -no-window -gpu swangle_indirect]
  PHONEDRIVER [/d/pu-m2/target/release/phonedriver.exe]  BENCH [/d/pu-m2/target/release/phonedriver-bench.exe]
  REPO [/d/pu-m2/src]  OUT [/d/pu-m2/trials]  HANG_WAIT_S [300]  BOOT=none reuses the running emulator
  READY [calm]: early starts the arm as soon as sys.boot_completed is 1, as the sessions that saw the hangs did

Writes <OUT>/<trial-name>/{emulator.out,emulator.err,logcat.txt,guest.txt,host.txt,arm.txt,anr/,dropbox/}
and appends one line to <OUT>/trials.tsv:
  start  trial  arm  emu_flags  boot_s  rounds_ok  rounds  outcome  seconds  boot_anrs  vulkan  crashpad_crashes
vulkan is the host Vulkan driver the emulator chose; crashpad_crashes counts crashes of the emulator's crash
reporter (Application log event 1000) since the trial started: each one is a crash of qemu that left it suspended.
Outcome is ok, hang (adb shell silent for HANG_WAIT_S, emulator process alive) or died (process gone).
A bench round is stopped once adb shell has been silent for a minute.
EOF
}

[ $# -ge 2 ] || { usage; exit 2; }
ARM=$1
TRIAL=$2
ROUNDS=${3:-10}
AVD=${AVD:-Pixel_API_37}
SERIAL=${SERIAL:-emulator-5554}
EMU_FLAGS=${EMU_FLAGS:--no-snapshot -no-audio -no-boot-anim -no-window -gpu swangle_indirect}
PHONEDRIVER=${PHONEDRIVER:-/d/pu-m2/target/release/phonedriver.exe}
BENCH=${BENCH:-/d/pu-m2/target/release/phonedriver-bench.exe}
REPO=${REPO:-/d/pu-m2/src}
OUT=${OUT:-/d/pu-m2/trials}
HANG_WAIT_S=${HANG_WAIT_S:-300}
SDK=${SDK:-/d/Android/Sdk}
export MSYS_NO_PATHCONV=1
export PATH=$SDK/platform-tools:$SDK/emulator:$PATH
TB=dev.phonedriver.testbed
DIR=$OUT/$TRIAL
mkdir -p "$DIR"
WINDIR_OUT=$(cygpath -w "$DIR")
export PHONEDRIVER_HOME
PHONEDRIVER_HOME=$(cygpath -w "$DIR/home")

adbs() { timeout 60 adb -s "$SERIAL" "$@"; }
alive() { [ "$(timeout 20 adb -s "$SERIAL" shell echo ok 2>/dev/null | tr -d '\r')" = ok ]; }
our_qemu() {
  powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name like 'qemu-system%'\" | Where-Object { \$_.CommandLine -match ' -avd $AVD ' } | ForEach-Object { \$_.ProcessId }" | tr -d '\r'
}
qemu_running() { [ -n "$(our_qemu)" ]; }
qemu_cpu() {
  local pid
  pid=$(our_qemu)
  [ -n "$pid" ] && powershell -NoProfile -Command "(Get-Process -Id $pid).TotalProcessorTime.TotalSeconds" | tr -d '\r'
}
crashpad_crashes_since() {
  powershell -NoProfile -Command "@(Get-WinEvent -FilterHashtable @{LogName='Application'; Id=1000; StartTime=[datetime]'$1'} -ErrorAction SilentlyContinue | Where-Object { \$_.Message -match 'crashpad_handler.exe' }).Count" | tr -d '\r'
}
log() { echo "$(date +%H:%M:%S) $*" | tee -a "$DIR/arm.txt"; }

stop_emulator() {
  adbs emu kill >/dev/null 2>&1
  for _ in $(seq 30); do qemu_running || return 0; sleep 2; done
  local pid
  for pid in $(our_qemu); do taskkill /F /PID "$pid" >/dev/null 2>&1; done
  sleep 3
}

boot() {
  stop_emulator
  local args
  args=$(printf "'%s'," -avd "$AVD" -port "${SERIAL#emulator-}" $EMU_FLAGS)
  powershell -NoProfile -Command "Start-Process -FilePath '$(cygpath -w "$SDK/emulator/emulator.exe")' \
    -ArgumentList ${args%,} -WindowStyle Hidden \
    -RedirectStandardOutput '$WINDIR_OUT\\emulator.out' -RedirectStandardError '$WINDIR_OUT\\emulator.err'"
  local t0=$SECONDS
  timeout 300 adb -s "$SERIAL" wait-for-device
  until [ "$(timeout 10 adb -s "$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; do
    [ $((SECONDS - t0)) -gt 600 ] && return 1
    sleep 3
  done
  local calm=0 focus load
  [ "${READY:-calm}" = early ] && calm=2
  while [ $calm -lt 2 ]; do
    [ $((SECONDS - t0)) -gt 900 ] && return 1
    sleep 10
    focus=$(timeout 15 adb -s "$SERIAL" shell dumpsys window 2>/dev/null | grep mCurrentFocus)
    load=$(timeout 15 adb -s "$SERIAL" shell cat /proc/loadavg 2>/dev/null | cut -d' ' -f1)
    if echo "$focus" | grep -q "Not Responding"; then
      log "boot: ANR dialog in focus at $((SECONDS - t0))s: ${focus##*u0 }"
      BOOT_ANRS=$((BOOT_ANRS + 1))
      adbs shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1
      calm=0
    elif echo "$focus" | grep -q "launcher" && awk "BEGIN{exit !(${load:-99} < 2.0)}"; then
      calm=$((calm + 1))
    else
      calm=0
    fi
  done
  BOOT_S=$((SECONDS - t0))
}

start_captures() {
  adbs logcat -b all -c >/dev/null 2>&1
  adb -s "$SERIAL" logcat -b all -v threadtime > "$DIR/logcat.txt" 2>&1 &
  LOGCAT_PID=$!
  (
    while true; do
      echo "== $(date +%H:%M:%S)"
      timeout 15 adb -s "$SERIAL" shell 'cat /proc/pressure/cpu /proc/pressure/memory /proc/pressure/io; head -3 /proc/meminfo; top -b -n 1 -m 8 | tail -9' 2>&1
      sleep 10
    done
  ) > "$DIR/guest.txt" 2>&1 &
  GUEST_PID=$!
  (
    while true; do
      echo "== $(date +%H:%M:%S)"
      powershell -NoProfile -Command "Get-Process qemu-system* -ErrorAction SilentlyContinue | Select-Object Id,CPU,@{n='WS_MB';e={[int](\$_.WorkingSet64/1MB)}},Threads | Format-Table -HideTableHeaders | Out-String -Width 200; \
        (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory; (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average" 2>&1 | tr -s '\r\n' '\n'
      sleep 10
    done
  ) > "$DIR/host.txt" 2>&1 &
  HOST_PID=$!
}

stop_captures() {
  kill "$LOGCAT_PID" "$GUEST_PID" "$HOST_PID" 2>/dev/null
}

collect() {
  if alive; then
    adbs root >/dev/null 2>&1
    sleep 3
    timeout 60 adb -s "$SERIAL" wait-for-device
    adbs shell dumpsys window windows > "$DIR/dumpsys-window.txt" 2>&1
    adbs pull /data/anr "$DIR/anr" >/dev/null 2>&1
    adbs pull /data/system/dropbox "$DIR/dropbox" >/dev/null 2>&1
    adbs pull /data/tombstones "$DIR/tombstones" >/dev/null 2>&1
  fi
}

web_round() {
  adbs shell am force-stop $TB
  adbs shell am start -n $TB/.MainActivity --es tab web >/dev/null
  sleep 6
  case $ARM in
    dump) timeout 60 adb -s "$SERIAL" shell uiautomator dump /data/local/tmp/d.xml >> "$DIR/arm.txt" 2>&1 ;;
    agent) for _ in 1 2 3; do timeout 60 "$PHONEDRIVER" --device "android:$SERIAL" snapshot >> "$DIR/arm.txt" 2>&1; done ;;
  esac
  adbs shell input tap 394 508
  sleep 1
  adbs shell input text Ada
  adbs shell input tap 172 565
  adbs shell input tap 81 623
  sleep 2
  adbs shell input keyevent BACK
  adbs shell am force-stop $TB
}

bench_round() {
  local only=()
  [ "$ARM" = webform ] && only=(--only testbed-web-form)
  [ "$ARM" = chrome ] && only=(--dir bench/scenarios/android-unstable)
  (cd "$REPO" && exec timeout 1800 "$BENCH" scenario --phonedriver "$(cygpath -w "$PHONEDRIVER")" --device "android:$SERIAL" "${only[@]}") >> "$DIR/arm.txt" 2>&1 &
  local bench=$! silent=0
  while kill -0 $bench 2>/dev/null; do
    sleep 20
    if alive; then silent=0; else silent=$((silent + 1)); fi
    if [ $silent -ge 3 ]; then
      log "adb shell silent for a minute during the round, stopping the bench"
      taskkill /F /IM phonedriver-bench.exe >/dev/null 2>&1
      taskkill /F /IM phonedriver.exe >/dev/null 2>&1
      break
    fi
  done
  wait $bench 2>/dev/null
}

case $ARM in none|dump|agent|webform|suite|chrome) ;; *) usage; exit 2 ;; esac
START=$(date +%Y-%m-%dT%H:%M:%S)
BOOT_S=0
BOOT_ANRS=0
if [ "${BOOT:-cold}" != none ]; then
  log "booting $AVD with $EMU_FLAGS, ready ${READY:-calm}"
  if ! boot; then
    log "boot did not finish"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$START" "$TRIAL" "$ARM" "$EMU_FLAGS" - 0 "$ROUNDS" boot_failed 0 >> "$OUT/trials.tsv"
    exit 1
  fi
  log "booted in ${BOOT_S}s"
fi
start_captures
T0=$SECONDS
OK=0
OUTCOME=ok
for r in $(seq "$ROUNDS"); do
  log "round $r"
  case $ARM in
    none|dump|agent) web_round ;;
    webform|suite|chrome) bench_round ;;
  esac
  if alive; then
    OK=$r
    continue
  fi
  log "adb shell silent after round $r"
  waited=0
  until alive || [ $waited -ge "$HANG_WAIT_S" ]; do sleep 15; waited=$((waited + 15)); done
  if alive; then
    log "adb shell answered again after ${waited}s"
    OK=$r
    continue
  fi
  if qemu_running; then
    OUTCOME=hang
    c1=$(qemu_cpu); sleep 10; c2=$(qemu_cpu)
    log "emulator process CPU seconds $c1 then $c2 ten seconds later"
  else
    OUTCOME=died
  fi
  log "$OUTCOME after round $r"
  break
done
[ "$ARM" = agent ] && "$PHONEDRIVER" --device "android:$SERIAL" release >/dev/null 2>&1
collect
stop_captures
VULKAN=$(grep -o -m1 "vulkan_mode_selected:[a-z_]*" "$DIR/emulator.out" 2>/dev/null)
CRASHPAD=$(crashpad_crashes_since "${START/T/ }")
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$START" "$TRIAL" "$ARM" "$EMU_FLAGS" "$BOOT_S" "$OK" "$ROUNDS" "$OUTCOME" $((SECONDS - T0)) "$BOOT_ANRS" "${VULKAN#*:}" "$CRASHPAD" >> "$OUT/trials.tsv"
log "trial $TRIAL: $OUTCOME, $OK of $ROUNDS rounds"
