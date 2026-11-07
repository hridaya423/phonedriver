#!/usr/bin/env bash
# One command from a checkout, on the machine that runs the emulator (default local adb server):
#   PH_PYTHON=/path/to/venv/bin/python bash bench/speed/reproduce.sh [emulator-5554]
# PH_PYTHON is a Python with `pip install phone-harness==0.3.3`. The testbed app is built first: see testbeds/android.
set -euo pipefail
cd "$(dirname "$0")/../.."
SERIAL="${1:-emulator-5554}"
OUT="${OUT:-bench/speed/out-$(date +%Y%m%d-%H%M%S)}"
: "${PH_PYTHON:?set PH_PYTHON to a Python that has phone-harness 0.3.3}"
unset ADB_SERVER_SOCKET
bash android-agent/build.sh
cargo build --release -p phone-use
APK=testbeds/android/app/build/outputs/apk/debug/app-debug.apk
EXE=target/release/phone-use
[ -f "$EXE.exe" ] && EXE="$EXE.exe"
"$PH_PYTHON" bench/speed/run_speed.py --phone-use "$EXE" --apk "$APK" --serial "$SERIAL" --out "$OUT" --ph-python "$PH_PYTHON"
python3 bench/speed/stats.py "$OUT" --md "$OUT/tables.md"
