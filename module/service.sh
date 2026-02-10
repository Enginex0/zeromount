#!/system/bin/sh
MODDIR="${0%/*}"

# Single-instance guard
LOCKFILE="/dev/zeromount_lock"
if [ -f "$LOCKFILE" ]; then
    exit 0
fi
echo $$ > "$LOCKFILE"
trap 'rm -f "$LOCKFILE"' EXIT INT TERM

# Shell fast-fail on bootloop
COUNT=$(cat /data/adb/zeromount/.bootcount 2>/dev/null || echo 0)
[ "$COUNT" -ge 3 ] && { echo "zeromount: bootloop guard triggered (count=$COUNT), skipping pipeline" > /dev/kmsg 2>/dev/null; exit 0; }

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 1
[ -x "$BIN" ] || exit 1

"$BIN" mount --post-boot
