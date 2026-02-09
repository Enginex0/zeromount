#!/system/bin/sh
# Detect kernel capabilities within KSU's 10-second blocking timeout.
# KSU11: post-mount.sh and boot-completed.sh intentionally unused.
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64) ABI=x86_64 ;;
    i686|i386) ABI=x86 ;;
    *) exit 1 ;;
esac

BIN="$MODDIR/bin/${ABI}/zeromount"
[ -x "$BIN" ] || exit 1

# Must complete in <2 seconds (within 10s KSU timeout)
"$BIN" detect
