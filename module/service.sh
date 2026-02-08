#!/system/bin/sh
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ARCH=arm64 ;;
    armv7*|armv8l) ARCH=arm ;;
    x86_64) ARCH=x86_64 ;;
    i686|i386) ARCH=x86 ;;
esac

BIN="$MODDIR/zm-${ARCH}"
[ -x "$BIN" ] || exit 0

# Post-boot tasks: UID blocking, WebUI symlink, module watcher
"$BIN" mount --post-boot
