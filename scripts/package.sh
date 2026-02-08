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

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --out)     OUT_NAME="$2"; shift 2 ;;
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

# -- Rust binaries --
BINARIES=(zm-arm64 zm-arm zm-x86_64 zm-x86)
FOUND_BINS=0

for bin in "${BINARIES[@]}"; do
    # Check module/ first (local build), then staging/ (CI artifacts)
    if [ -f "$MODULE_DIR/$bin" ]; then
        cp "$MODULE_DIR/$bin" "$STAGING/$bin"
        FOUND_BINS=$((FOUND_BINS + 1))
    elif [ -f "$PROJECT_ROOT/staging/$bin/$bin" ]; then
        cp "$PROJECT_ROOT/staging/$bin/$bin" "$STAGING/$bin"
        FOUND_BINS=$((FOUND_BINS + 1))
    fi
done

if [ "$FOUND_BINS" -eq 0 ]; then
    echo "WARNING: no binaries found — ZIP will lack executables" >&2
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
    echo "WARNING: webroot/ not found — ZIP will lack WebUI" >&2
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
chmod 755 "$MODPATH"/*.sh "$MODPATH"/zm-* 2>/dev/null || true
ui_print "ZeroMount installed via recovery"
exit 0
UPDATER

echo "" > "$STAGING/META-INF/com/google/android/updater-script"

# -- Verify eliminated scripts are ABSENT --
ELIMINATED=(logging.sh susfs_integration.sh monitor.sh sync.sh zm-diag.sh)
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
echo "    Binaries: $FOUND_BINS/4"
echo "    WebUI: $([ -d "$STAGING/webroot" ] && echo "present" || echo "MISSING")"
echo "    Eliminated scripts: verified absent"
