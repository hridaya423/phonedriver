#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
SDK=${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}
BT=$SDK/build-tools/${ANDROID_BUILD_TOOLS:-37.0.0}
PLATFORM=$SDK/platforms/${ANDROID_PLATFORM:-android-37.0}
AJ=$PLATFORM/android.jar
OUT=${OUT:-build}
rm -rf "$OUT" && mkdir -p "$OUT/classes" "$OUT/gen/com/genymobile/scrcpy"
cat > "$OUT/gen/com/genymobile/scrcpy/BuildConfig.java" <<JAVA
package com.genymobile.scrcpy;
public final class BuildConfig { public static final boolean DEBUG = false; public static final String VERSION_NAME = "phone-use"; }
JAVA
(cd aidl && "$BT/aidl" -o"../$OUT/gen" -I. -p "$PLATFORM/framework.aidl" android/view/IDisplayWindowListener.aidl)
javac -encoding UTF-8 -bootclasspath "$AJ" -cp "$BT/core-lambda-stubs.jar" -d "$OUT/classes" -source 1.8 -target 1.8 -Xlint:-options \
  -sourcepath "src:$OUT/gen" src/dev/phoneuse/agent/*.java
(cd "$OUT/classes" && "$BT/d8" --min-api 26 --release --classpath "$AJ" --output ../agent.zip $(find . -name '*.class'))
mv "$OUT/agent.zip" "$OUT/phone-use-agent.jar"
