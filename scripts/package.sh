#!/usr/bin/env bash
# Assemble ZeroMount module ZIP from CI artifacts or local build.
# Usage: ./scripts/package.sh [--version v2.0.0] [--out zeromount-v2.0.0.zip]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module"

VERSION="v2.0.0"
OUT_NAME=""
STAGING=""
BUILD=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --out)     OUT_NAME="$2"; shift 2 ;;
        --build)   BUILD=true; shift ;;
        *)         echo "Unknown arg: $1"; exit 1 ;;
    esac
done

[ -z "$OUT_NAME" ] && OUT_NAME="zeromount-${VERSION}.zip"

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

echo "==> Assembling ZeroMount $VERSION"

# -- Shell scripts --
SCRIPTS=(
    post-fs-data.sh
    metamount.sh
    service.sh
    uninstall.sh
    metainstall.sh
    metauninstall.sh
    customize.sh
    monitor.sh
)

for script in "${SCRIPTS[@]}"; do
    src="$MODULE_DIR/$script"
    if [ ! -f "$src" ]; then
        echo "FATAL: missing $script" >&2
        exit 1
    fi
    cp "$src" "$STAGING/$script"
done

# -- module.prop --
if [ ! -f "$MODULE_DIR/module.prop" ]; then
    echo "FATAL: missing module.prop" >&2
    exit 1
fi
cp "$MODULE_DIR/module.prop" "$STAGING/module.prop"

# Patch version into module.prop if --version was given
sed -i "s/^version=.*/version=${VERSION}/" "$STAGING/module.prop"
VERSION_CODE="${VERSION#v}"
VERSION_CODE="${VERSION_CODE//.}"
sed -i "s/^versionCode=.*/versionCode=${VERSION_CODE}/" "$STAGING/module.prop"

# -- Cross-compile Rust binaries (optional) --
declare -A ABI_TARGET=(
    [arm64-v8a]=aarch64-linux-android
    [armeabi-v7a]=armv7-linux-androideabi
    [x86_64]=x86_64-linux-android
    [x86]=i686-linux-android
)

if [ "$BUILD" = true ]; then
    NDK_BIN="/opt/android-ndk-r25b/toolchains/llvm/prebuilt/linux-x86_64/bin"
    if [ ! -d "$NDK_BIN" ]; then
        echo "FATAL: Android NDK not found at /opt/android-ndk-r25b" >&2
        exit 1
    fi

    # Use president's rustup if available, fall back to system cargo
    if [ -f "/home/president/.cargo/bin/cargo" ]; then
        export RUSTUP_HOME=/home/president/.rustup
        export CARGO_HOME=/home/president/.cargo
        CARGO="/home/president/.cargo/bin/cargo"
    else
        CARGO="cargo"
    fi
    export PATH="$NDK_BIN:$PATH"

    for abi in "${!ABI_TARGET[@]}"; do
        target="${ABI_TARGET[$abi]}"
        echo "==> Building $abi ($target)"
        "$CARGO" build --manifest-path "$PROJECT_ROOT/Cargo.toml" \
            --target "$target" --release 2>&1
        mkdir -p "$MODULE_DIR/bin/$abi"
        cp "$PROJECT_ROOT/target/$target/release/zeromount" "$MODULE_DIR/bin/$abi/zeromount"
    done
    echo "==> All targets built"
fi

# -- Collect binaries into staging --
declare -A ABI_MAP=(
    [arm64-v8a]=zm-arm64
    [armeabi-v7a]=zm-arm
    [x86_64]=zm-x86_64
    [x86]=zm-x86
)
FOUND_BINS=0

for abi in "${!ABI_MAP[@]}"; do
    old_name="${ABI_MAP[$abi]}"
    mkdir -p "$STAGING/bin/$abi"

    if [ -f "$MODULE_DIR/bin/$abi/zeromount" ]; then
        cp "$MODULE_DIR/bin/$abi/zeromount" "$STAGING/bin/$abi/zeromount"
        FOUND_BINS=$((FOUND_BINS + 1))
    elif [ -f "$PROJECT_ROOT/target/${ABI_TARGET[$abi]}/release/zeromount" ]; then
        cp "$PROJECT_ROOT/target/${ABI_TARGET[$abi]}/release/zeromount" "$STAGING/bin/$abi/zeromount"
        FOUND_BINS=$((FOUND_BINS + 1))
    elif [ -f "$PROJECT_ROOT/staging/$old_name/$old_name" ]; then
        cp "$PROJECT_ROOT/staging/$old_name/$old_name" "$STAGING/bin/$abi/zeromount"
        FOUND_BINS=$((FOUND_BINS + 1))
    fi

    if [ -f "$MODULE_DIR/bin/$abi/aapt" ]; then
        cp "$MODULE_DIR/bin/$abi/aapt" "$STAGING/bin/$abi/aapt"
    fi
done

if [ "$FOUND_BINS" -ne 4 ]; then
    echo "FATAL: found $FOUND_BINS/4 zeromount binaries (need all 4 ABIs)" >&2
    exit 1
fi

# -- WebUI --
WEBROOT_SRC=""
if [ -d "$MODULE_DIR/webroot" ]; then
    WEBROOT_SRC="$MODULE_DIR/webroot"
elif [ -d "$PROJECT_ROOT/staging/webroot" ]; then
    WEBROOT_SRC="$PROJECT_ROOT/staging/webroot"
fi

if [ -n "$WEBROOT_SRC" ]; then
    cp -r "$WEBROOT_SRC" "$STAGING/webroot"
else
    echo "FATAL: webroot/ not found" >&2
    exit 1
fi

# -- META-INF for recovery/KSU compatibility --
mkdir -p "$STAGING/META-INF/com/google/android"
cat > "$STAGING/META-INF/com/google/android/update-binary" << 'UPDATER'
#!/sbin/sh

OUTFD=/proc/self/fd/$2
ZIPFILE="$3"

ui_print() { echo -e "ui_print $1\nui_print" >> $OUTFD; }

# KernelSU and APatch handle installation natively.
# This update-binary provides TWRP recovery fallback.
MODPATH="${MODPATH:-/data/adb/modules/zeromount}"
mkdir -p "$MODPATH"
unzip -o "$ZIPFILE" -d "$MODPATH" >&2
chmod 755 "$MODPATH"/*.sh "$MODPATH"/bin/*/zeromount 2>/dev/null || true
ui_print "ZeroMount installed via recovery"
exit 0
UPDATER

echo "" > "$STAGING/META-INF/com/google/android/updater-script"

# -- Verify eliminated scripts are ABSENT --
ELIMINATED=(logging.sh susfs_integration.sh sync.sh zm-diag.sh)
for dead in "${ELIMINATED[@]}"; do
    if [ -f "$STAGING/$dead" ]; then
        echo "FATAL: eliminated script $dead found in staging!" >&2
        exit 1
    fi
done

# -- Build ZIP --
case "$OUT_NAME" in
    /*) OUT_PATH="$OUT_NAME" ;;
    *)  OUT_PATH="$PROJECT_ROOT/$OUT_NAME" ;;
esac
(cd "$STAGING" && zip -r9 "$OUT_PATH" .)

echo "==> Built: $OUT_PATH"
echo "==> Contents:"
unzip -l "$OUT_PATH" | tail -n +4 | head -n -2

echo ""
echo "==> Verification:"
echo "    Binaries: $FOUND_BINS/4 (bin/<abi>/zeromount)"
echo "    WebUI: $([ -d "$STAGING/webroot" ] && echo "present" || echo "MISSING")"
echo "    Eliminated scripts: verified absent"
