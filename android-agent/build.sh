#!/usr/bin/env bash
set -euo pipefail
case ${BASH_SOURCE[0]} in */*) cd "${BASH_SOURCE[0]%/*}" ;; esac
DEFAULT_SDK=/opt/homebrew/share/android-commandlinetools
SDK=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$DEFAULT_SDK}}
BT_VERSION=${ANDROID_BUILD_TOOLS:-37.0.0}
PLATFORM_NAME=${ANDROID_PLATFORM:-android-37.0}
BT=$SDK/build-tools/$BT_VERSION
PLATFORM=$SDK/platforms/$PLATFORM_NAME
AJ=$PLATFORM/android.jar

problems=()
if [ ! -d "$SDK" ]; then
  problems+=("- no Android SDK at $SDK")
else
  if [ ! -d "$BT" ]; then
    found=$(ls "$SDK/build-tools" 2>/dev/null | tr '\n' ' ' | sed 's/ $//')
    problems+=("- build-tools $BT_VERSION not found under $SDK (found: ${found:-none})")
  else
    for tool in aidl d8; do
      [ -x "$BT/$tool" ] || problems+=("- $tool: not in $BT")
    done
    [ -f "$BT/core-lambda-stubs.jar" ] || problems+=("- core-lambda-stubs.jar: not in $BT")
  fi
  [ -f "$AJ" ] || problems+=("- android.jar: platform $PLATFORM_NAME not found at $PLATFORM")
fi
command -v javac >/dev/null 2>&1 || problems+=("- javac: no JDK on PATH; install JDK 17")

if [ ${#problems[@]} -gt 0 ]; then
  {
    echo "error: cannot build the Android agent:"
    printf '  %s\n' "${problems[@]}"
    if [ -n "${ANDROID_HOME:-}" ]; then
      echo "ANDROID_HOME=$ANDROID_HOME."
    else
      echo "ANDROID_HOME is unset, so the SDK defaults to ${ANDROID_SDK_ROOT:-$DEFAULT_SDK}."
    fi
    echo "Do one of these:"
    echo "  install the tools:   sdkmanager \"build-tools;$BT_VERSION\" \"platforms;$PLATFORM_NAME\" (and JDK 17)"
    echo "  use another SDK:     ANDROID_HOME=<sdk directory>"
    echo "  use other versions:  ANDROID_BUILD_TOOLS=<version> ANDROID_PLATFORM=<android-NN.N>"
    echo "  use a built jar:     PHONE_USE_AGENT_JAR=<path to phone-use-agent.jar> (none is published yet)"
    echo "  skip Android:        cargo build --release -p phone-use --no-default-features (iOS only)"
  } >&2
  exit 1
fi

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
