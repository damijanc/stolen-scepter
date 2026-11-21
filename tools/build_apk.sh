#!/bin/sh
# Build a sideloadable, debug-signed Android APK of the shell, with the game's
# own pack bundled as an APK asset (`assets/game.openbounty`).
#
#   sh tools/build_apk.sh            # -> target/android/stolen-scepter.apk
#   adb install -r target/android/stolen-scepter.apk
#
# Why not `cargo quad-apk`: it embeds Cargo 1.61, which cannot read this
# workspace (workspace inheritance, lockfile v4, edition-2024 dependencies).
# This script does the same steps with today's toolchain: build the shell as
# a cdylib (the `openbounty_android` example), then package miniquad's own
# Java activity around it with aapt2, javac, d8, zipalign and apksigner.
#
# Needs: `rustup target add aarch64-linux-android`, a JDK, and an Android SDK
# with platform 36, build-tools and an NDK. ANDROID_HOME and NDK_HOME default
# to the Android Studio locations.
set -eu
cd "$(dirname "$0")/.."

: "${ANDROID_HOME:=$HOME/Library/Android/sdk}"
[ -n "${NDK_HOME:-}" ] || NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* | sort | tail -1)"
PLATFORM="$ANDROID_HOME/platforms/android-36/android.jar"
BT="$(ls -d "$ANDROID_HOME"/build-tools/* | sort | tail -1)"
HOST_TAG="$(ls "$NDK_HOME/toolchains/llvm/prebuilt" | head -1)"
TC="$NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/bin"
PACKAGE="io.github.openbounty.stolenscepter"
LIB="openbounty_android"
OUT="target/android"
WORK="$OUT/work"

# 1. The native library. 16 KB page alignment: Android 15+ devices with
# 16 KB pages refuse a 4 KB-aligned library, and Play requires it.
export CC_aarch64_linux_android="$TC/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$TC/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TC/aarch64-linux-android26-clang"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384"
cargo build --release -p openbounty-shell --example "$LIB" --features android-apk \
  --target aarch64-linux-android

rm -rf "$WORK"
mkdir -p "$WORK/assets" "$WORK/lib/arm64-v8a" "$WORK/java" "$WORK/classes"
cp "target/aarch64-linux-android/release/examples/lib$LIB.so" "$WORK/lib/arm64-v8a/"

# 2. The pack, as the deflate-only ZIP `Pack::from_archive_bytes` reads.
python3 - "$WORK/assets/game.openbounty" <<'EOF'
import os, sys, zipfile
src, out = "packs/stolen-scepter", sys.argv[1]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(src):
        for name in sorted(files):
            if name != ".DS_Store":
                path = os.path.join(root, name)
                z.write(path, os.path.relpath(path, src))
EOF

# 3. miniquad's Java glue, taken from the exact miniquad version Cargo.lock
# pins, with its template names filled in and its plugin hooks emptied.
MQ="$(cargo metadata --format-version 1 --filter-platform aarch64-linux-android \
  | python3 -c 'import json,sys; print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"]=="miniquad").rsplit("/",1)[0])')"
mkdir -p "$WORK/java/$(echo "$PACKAGE" | tr . /)" "$WORK/java/quad_native"
sed -e "s/TARGET_PACKAGE_NAME/$PACKAGE/" -e "s/LIBRARY_NAME/$LIB/" -e 's#^ *//%.*$##' \
  "$MQ/java/MainActivity.java" > "$WORK/java/$(echo "$PACKAGE" | tr . /)/MainActivity.java"
cp "$MQ/java/QuadNative.java" "$WORK/java/quad_native/"

cat > "$WORK/AndroidManifest.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="$PACKAGE" android:versionCode="1" android:versionName="0.1.0">
    <uses-feature android:glEsVersion="0x00030000" android:required="true" />
    <application android:label="Stolen Scepter" android:hasCode="true"
        android:theme="@android:style/Theme.DeviceDefault.NoActionBar.Fullscreen">
        <activity android:name=".MainActivity" android:exported="true"
            android:screenOrientation="userLandscape"
            android:configChanges="orientation|keyboardHidden|screenSize|screenLayout|keyboard">
            <meta-data android:name="android.app.lib_name" android:value="$LIB" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
EOF

# 4. Package: manifest + assets, then dex, then the library.
"$BT/aapt2" link -o "$WORK/unsigned.apk" -I "$PLATFORM" \
  --manifest "$WORK/AndroidManifest.xml" -A "$WORK/assets" \
  --min-sdk-version 26 --target-sdk-version 36
javac -nowarn -source 8 -target 8 -bootclasspath "$PLATFORM" -classpath "$PLATFORM" \
  -d "$WORK/classes" $(find "$WORK/java" -name '*.java') 2>&1 | grep -v "^warning: \[options\]" || true
"$BT/d8" --min-api 26 --lib "$PLATFORM" --output "$WORK" $(find "$WORK/classes" -name '*.class')
(cd "$WORK" && zip -q unsigned.apk classes.dex && zip -q -0 unsigned.apk lib/arm64-v8a/lib$LIB.so)

# 5. Align (16 KB for the stored library) and sign with a local debug key.
"$BT/zipalign" -f -P 16 4 "$WORK/unsigned.apk" "$WORK/aligned.apk"
KEY="$OUT/debug.keystore"
[ -f "$KEY" ] || keytool -genkeypair -keystore "$KEY" -storepass android -keypass android \
  -alias debug -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Debug" >/dev/null 2>&1
"$BT/apksigner" sign --ks "$KEY" --ks-pass pass:android --out "$OUT/stolen-scepter.apk" "$WORK/aligned.apk"
ls -la "$OUT/stolen-scepter.apk"
