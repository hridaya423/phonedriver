#!/usr/bin/env bash
# Usage: bench/agent-eval/run-jobs.sh <harness> <out-dir> <task>...
# One run per task through run-harness.sh. Needs the environment run-harness.sh lists plus EMU_ADB (see jobs/jobs.py).
set -u
[ -z "${ANTHROPIC_API_KEY:-}" ] || { echo "ANTHROPIC_API_KEY is set" >&2; exit 1; }
harness=$1; out=$2; shift 2
here=$(cd "$(dirname "$0")" && pwd)
for task in "$@"; do
  "$here/run-harness.sh" "$harness" "$task" "$out"
done
