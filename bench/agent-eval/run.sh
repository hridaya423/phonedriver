#!/usr/bin/env bash
# Usage: bench/agent-eval/run.sh <tuning|heldout> <runs> <out-dir> [task]
# Needs PHONE_USE_BIN, PHONE_USE_BENCH, PHONE_USE_DEVICE (android:<serial>) and ADB_SERVER_SOCKET in the environment.
set -u
set_name=$1
runs=$2
out=$3
only=${4:-}
here=$(cd "$(dirname "$0")" && pwd)
tasks=$here/tasks.yaml
mkdir -p "$out"
workdir=$(mktemp -d)
limit=${EVAL_TIMEOUT_S:-600}
bench() { "$PHONE_USE_BENCH" eval "$@"; }

for task in $(bench list "$tasks" "$set_name"); do
  [ -n "$only" ] && [ "$task" != "$only" ] && continue
  for run in $(seq 1 "$runs"); do
    id=$task-$run
    home=$(mktemp -d)
    bench reset "$tasks" "$task" "$PHONE_USE_BIN" "$PHONE_USE_DEVICE" >"$out/$id.reset.txt" 2>&1 || echo "reset failed for $id: $(cat "$out/$id.reset.txt")" >&2
    sed -e "s#\${PHONE_USE_BIN}#$PHONE_USE_BIN#" -e "s#\${PHONE_USE_HOME}#$home#" \
        -e "s#\${PHONE_USE_DEVICE}#$PHONE_USE_DEVICE#" -e "s#\${ADB_SERVER_SOCKET}#$ADB_SERVER_SOCKET#" \
        "$here/mcp.json" >"$out/$id.mcp.json"
    prompt=$(bench prompt "$tasks" "$task")
    start=$(date +%s)
    (cd "$workdir" && claude -p "$prompt" --mcp-config "$out/$id.mcp.json" --strict-mcp-config \
        --allowedTools "mcp__phone-use__*" --tools "" --disable-slash-commands --setting-sources "" \
        --no-session-persistence --model claude-sonnet-5-5 --output-format stream-json --verbose \
        >"$out/$id.jsonl" 2>"$out/$id.stderr") &
    pid=$!
    ( sleep "$limit"; kill "$pid" 2>/dev/null ) &
    watchdog=$!
    wait "$pid" 2>/dev/null
    kill "$watchdog" 2>/dev/null
    wall=$(( $(date +%s) - start ))
    for p in $(lsof -t "$home/run/daemon.sock" 2>/dev/null); do kill "$p" 2>/dev/null; done
    sleep 2
    "$PHONE_USE_BENCH" stats "$out/$id.jsonl" | python3 -c 'import json,sys; print(json.load(sys.stdin)["answer"] or "")' >"$out/$id.answer.txt"
    if bench check "$tasks" "$task" "$PHONE_USE_BIN" "$PHONE_USE_DEVICE" "$out/$id.answer.txt" >"$out/$id.check.txt" 2>&1; then ok=true; else ok=false; fi
    bench cleanup "$tasks" "$task" "$PHONE_USE_BIN" "$PHONE_USE_DEVICE" >/dev/null 2>&1
    stats=$("$PHONE_USE_BENCH" stats "$out/$id.jsonl")
    python3 -c 'import json,sys; d=json.loads(sys.argv[1]); d.update(task=sys.argv[2], run=int(sys.argv[3]), success=sys.argv[4]=="true", wall_s=int(sys.argv[5])); d.pop("answer",None); print(json.dumps(d))' \
      "$stats" "$task" "$run" "$ok" "$wall" | tee -a "$out/results.jsonl"
  done
done
