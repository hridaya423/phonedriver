#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
SDK=/opt/homebrew/share/android-commandlinetools
BT=$SDK/build-tools/37.0.0
AJ=$SDK/platforms/android-37.0/android.jar
rm -rf build && mkdir -p build/classes
[ -f debug.keystore ] || keytool -genkeypair -keystore debug.keystore -storepass android -keypass android -alias k -dname "CN=spike" -keyalg RSA -keysize 2048 -validity 10000 >/dev/null 2>&1
$BT/aapt2 link --manifest AndroidManifest.xml -I $AJ -o build/base.apk --min-sdk-version 26 --target-sdk-version 34
javac -encoding UTF-8 -bootclasspath $AJ -source 1.8 -target 1.8 -Xlint:-options -d build/classes $(find src -name '*.java')
(cd build/classes && $BT/d8 --min-api 26 --classpath $AJ --output .. $(find . -name '*.class'))
cp build/base.apk build/app.apk
(cd build && zip -q app.apk classes.dex)
$BT/zipalign -f 4 build/app.apk build/aligned.apk
$BT/apksigner sign --ks debug.keystore --ks-pass pass:android --out build/spikeapp.apk build/aligned.apk
ls -l build/spikeapp.apk
