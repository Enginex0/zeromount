#!/bin/bash
set -e

NDK=/opt/android-ndk-r29
BUILD_TOOLS=~/Android/Sdk/build-tools/34.0.0
ANDROID_JAR=~/Android/Sdk/platforms/android-34/android.jar
WORK=/tmp/adbdetect_build
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Cleaning build dir"
rm -rf "$WORK"
mkdir -p "$WORK"/{classes,lib/arm64-v8a,apk,res/values}

echo "==> Building JNI .so"
"$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang" \
    -shared -fPIC -O2 \
    "$SCRIPT_DIR/jni/adbdetect.c" \
    -o "$WORK/lib/arm64-v8a/libadbdetect.so" \
    -llog -lc

echo "==> Compiling Java"
javac -source 8 -target 8 \
    -cp "$ANDROID_JAR" \
    -d "$WORK/classes" \
    "$SCRIPT_DIR/src/com/test/adbdetect/DetectorCheck.java"

echo "==> Dexing"
"$BUILD_TOOLS/d8" --release \
    --output "$WORK/apk/" \
    "$WORK/classes/com/test/adbdetect/DetectorCheck.class" \
    --lib "$ANDROID_JAR"

echo "==> Packaging resources"
cat > "$WORK/res/values/strings.xml" <<'EOF'
<resources><string name="app_name">ADBDetect</string></resources>
EOF
"$BUILD_TOOLS/aapt2" compile --dir "$WORK/res/" -o "$WORK/compiled.zip"
"$BUILD_TOOLS/aapt2" link "$WORK/compiled.zip" \
    -I "$ANDROID_JAR" \
    --manifest "$SCRIPT_DIR/AndroidManifest.xml" \
    -o "$WORK/apk/adbdetect_base.apk"

echo "==> Adding dex and native lib"
cd "$WORK/apk"
cp classes.dex ./
zip -j adbdetect_base.apk classes.dex
mkdir -p lib/arm64-v8a
cp "$WORK/lib/arm64-v8a/libadbdetect.so" lib/arm64-v8a/
zip -r adbdetect_base.apk lib/

echo "==> Aligning and signing"
"$BUILD_TOOLS/zipalign" -f 4 adbdetect_base.apk adbdetect_aligned.apk
"$BUILD_TOOLS/apksigner" sign \
    --ks ~/.android/debug.keystore --ks-pass pass:android \
    --out adbdetect_signed.apk adbdetect_aligned.apk

echo ""
echo "==> Output: $WORK/apk/adbdetect_signed.apk"
echo ""
echo "Install:"
echo "  adb install -r $WORK/apk/adbdetect_signed.apk"
echo "Launch:"
echo "  adb shell am start -n com.test.adbdetect/.DetectorCheck"
echo "Logcat:"
echo "  adb logcat -s ADBDetect"
