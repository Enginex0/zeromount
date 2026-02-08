#!/system/bin/sh
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ARCH=arm64 ;;
    armv7*|armv8l) ARCH=arm ;;
    x86_64) ARCH=x86_64 ;;
    i686|i386) ARCH=x86 ;;
esac

BIN="$MODDIR/zm-${ARCH}"
[ -x "$BIN" ] || exit 1

# Bootloop protection: increment counter, reset on success
COUNTER_FILE="/data/adb/zeromount/.bootcount"
COUNT=$(cat "$COUNTER_FILE" 2>/dev/null || echo 0)
COUNT=$((COUNT + 1))
echo "$COUNT" > "$COUNTER_FILE"
[ "$COUNT" -gt 3 ] && { "$BIN" config restore; echo 0 > "$COUNTER_FILE"; }

"$BIN" mount && echo 0 > "$COUNTER_FILE"
