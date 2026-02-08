#!/system/bin/sh
# Detect kernel capabilities within KSU's 10-second blocking timeout.
# KSU11: post-mount.sh and boot-completed.sh intentionally unused.
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ARCH=arm64 ;;
    armv7*|armv8l) ARCH=arm ;;
    x86_64) ARCH=x86_64 ;;
    i686|i386) ARCH=x86 ;;
esac

BIN="$MODDIR/zm-${ARCH}"
[ -x "$BIN" ] || exit 1

# Must complete in <2 seconds (within 10s KSU timeout)
"$BIN" detect
