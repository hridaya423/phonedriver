#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
SDK=/opt/homebrew/share/android-commandlinetools
BT=$SDK/build-tools/37.0.0
AJ=$SDK/platforms/android-37.0/android.jar
OUT=${OUT:-build}
rm -rf "$OUT" && mkdir -p "$OUT/classes" "$OUT/gen/com/genymobile/scrcpy"
cat > "$OUT/gen/com/genymobile/scrcpy/BuildConfig.java" <<JAVA
package com.genymobile.scrcpy;
public final class BuildConfig { public static final boolean DEBUG = false; public static final String VERSION_NAME = "spike"; }
JAVA
(cd aidl && "$BT/aidl" -o"../$OUT/gen" -I. android/content/IOnPrimaryClipChangedListener.aidl && "$BT/aidl" -o"../$OUT/gen" -I. -p "$SDK/platforms/android-37.0/framework.aidl" android/view/IDisplayWindowListener.aidl)
javac -encoding UTF-8 -bootclasspath "$AJ" -cp "$BT/core-lambda-stubs.jar" -d "$OUT/classes" -source 1.8 -target 1.8 -Xlint:-options \
  $(find src "$OUT/gen" -name '*.java') 
(cd "$OUT/classes" && "$BT/d8" --min-api 26 --classpath "$AJ" --output ../spike.zip $(find . -name '*.class'))
mv "$OUT/spike.zip" "$OUT/spike.jar"
ls -l "$OUT/spike.jar"
