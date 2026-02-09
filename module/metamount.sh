#!/system/bin/sh
MODDIR="${0%/*}"

case "$(uname -m)" in
    aarch64) ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64) ABI=x86_64 ;;
    i686|i386) ABI=x86 ;;
esac

BIN="$MODDIR/bin/${ABI}/zeromount"
[ -x "$BIN" ] || exit 1

# Bootloop protection: increment counter, reset on success
COUNTER_FILE="/data/adb/zeromount/.bootcount"
COUNT=$(cat "$COUNTER_FILE" 2>/dev/null || echo 0)
COUNT=$((COUNT + 1))
echo "$COUNT" > "$COUNTER_FILE"
[ "$COUNT" -gt 3 ] && { "$BIN" config restore; echo 0 > "$COUNTER_FILE"; }

"$BIN" mount && echo 0 > "$COUNTER_FILE"
