#!/usr/bin/env bash
# Usage: bench/agent-eval/run-harness.sh <phone-use|mobile-mcp|phone-harness|adb> <task> <out-dir>
# Needs PHONE_USE_BIN, PHONE_USE_BENCH, ADB_SERVER_SOCKET, PH_BIN_DIR (phone-harness venv bin), MOBILE_MCP_VERSION; target serial in SERIAL.
set -u
harness=$1; task=$2; out=$3
serial=${SERIAL:-emulator-5558}
here=$(cd "$(dirname "$0")" && pwd)
tasks=$here/tasks.yaml
mkdir -p "$out"; out=$(cd "$out" && pwd)
bench() { "$PHONE_USE_BENCH" eval "$@"; }
dev=android:$serial
adb -s $serial shell "pkill -f mobilecli" >/dev/null 2>&1; sleep 10
id=$harness-$task
home=$(mktemp -d); workdir=$(mktemp -d)
bench reset "$tasks" "$task" "$PHONE_USE_BIN" "$dev" >"$out/$id.reset.txt" 2>&1 || echo "reset failed: $(cat "$out/$id.reset.txt")" >&2
prompt=$(bench prompt "$tasks" "$task")
sys="The target device is the Android emulator with adb serial $serial. Use only that device."
args=(--strict-mcp-config --disable-slash-commands --setting-sources "" --no-session-persistence --model claude-sonnet-5-5 --output-format stream-json --verbose --max-turns 40)
env_extra=()
case $harness in
  phone-use)
    cat >"$out/$id.mcp.json" <<J
{"mcpServers":{"phone-use":{"command":"$PHONE_USE_BIN","args":["mcp"],"env":{"PHONE_USE_HOME":"$home","PHONE_USE_DEVICE":"$dev","ADB_SERVER_SOCKET":"$ADB_SERVER_SOCKET"}}}}
J
    args+=(--mcp-config "$out/$id.mcp.json" --allowedTools "mcp__phone-use__*" --tools "");;
  mobile-mcp)
    sys="The target device is the Android emulator named Pixel_Tablet_API_37 (adb serial $serial); mobile-mcp may list it as Pixel_Tablet_API_37 or emulator-5558; use only that device."
    cat >"$out/$id.mcp.json" <<J
{"mcpServers":{"mobile-mcp":{"command":"npx","args":["-y","@mobilenext/mobile-mcp@$MOBILE_MCP_VERSION"],"env":{"ANDROID_SERIAL":"$serial","ADB_SERVER_SOCKET":"$ADB_SERVER_SOCKET"}}}}
J
    tools=""; allowed="mcp__mobile-mcp__*"
    if [ "${ALLOW_ADB:-}" = 1 ]; then tools=Bash; allowed="$allowed Bash(adb:*)"; sys="$sys You may also run adb through Bash (adb -s $serial ...), for example adb shell, logcat -d and dumpsys."; fi
    args+=(--mcp-config "$out/$id.mcp.json" --tools "$tools" --allowedTools $allowed);;
  phone-harness)
    sys="$sys You control it through the phone-harness command in Bash: pipe a Python script to it, e.g. phone-harness <<'PY' then print(screen_info()) then PY. Helpers include screen(), tap_text(), type_text(), open_app()."
    export PATH="$PH_BIN_DIR:$PATH" ANDROID_SERIAL=$serial DO_NOT_TRACK=1 PHONE_HARNESS_TELEMETRY=0 PHONE_HARNESS_NO_UPDATE_CHECK=1 PHONE_HARNESS_PLATFORM=android
    allowed="Bash(phone-harness:*)"
    if [ "${ALLOW_ADB:-}" = 1 ]; then allowed="$allowed Bash(adb:*)"; sys="$sys You may also run adb through Bash (adb -s $serial ...), for example adb shell, logcat -d and dumpsys."; fi
    args+=(--tools Bash --allowedTools $allowed);;
  adb)
    sys="$sys You control it with adb through Bash (adb -s $serial ...). adb shell uiautomator dump /dev/stdout, input tap|swipe|text|keyevent, am start and screencap -p exist."
    args+=(--tools Bash --allowedTools "Bash(adb:*)");;
esac
start=$(date +%s)
(cd "$workdir" && claude -p "$prompt" --append-system-prompt "$sys" "${args[@]}" >"$out/$id.jsonl" 2>"$out/$id.stderr") &
pid=$!
( sleep 600; kill "$pid" 2>/dev/null ) &
wd=$!
wait "$pid" 2>/dev/null
pkill -f "mobile-mcp@$MOBILE_MCP_VERSION" 2>/dev/null
adb -s $serial shell "pkill -f mobilecli" >/dev/null 2>&1
kill "$wd" 2>/dev/null
wall=$(( $(date +%s) - start ))
for p in $(lsof -t "$home/run/daemon.sock" 2>/dev/null); do kill "$p" 2>/dev/null; done
"$PHONE_USE_BENCH" stats "$out/$id.jsonl" | python3 -c 'import json,sys; print(json.load(sys.stdin)["answer"] or "")' >"$out/$id.answer.txt"
if bench check "$tasks" "$task" "$PHONE_USE_BIN" "$dev" "$out/$id.answer.txt" "$out/$id.jsonl" >"$out/$id.check.txt" 2>&1; then ok=true; else ok=false; fi
bench cleanup "$tasks" "$task" "$PHONE_USE_BIN" "$dev" >/dev/null 2>&1
stats=$("$PHONE_USE_BENCH" stats "$out/$id.jsonl")
python3 -c 'import json,sys; d=json.loads(sys.argv[1]); d.update(harness=sys.argv[2], task=sys.argv[3], success=sys.argv[4]=="true", wall_s=int(sys.argv[5])); d.pop("answer",None); print(json.dumps(d))' "$stats" "$harness" "$task" "$ok" "$wall" | tee -a "$out/results.jsonl"
