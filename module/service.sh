#!/system/bin/sh
MODDIR="${0%/*}"

# Single-instance guard
LOCKFILE="/dev/zeromount_lock"
if [ -f "$LOCKFILE" ]; then
    exit 0
fi
echo $$ > "$LOCKFILE"
trap 'kill 0 2>/dev/null; rm -f "$LOCKFILE"' EXIT
trap 'exit 0' INT TERM

# Shell fast-fail on bootloop
COUNT=$(cat /data/adb/zeromount/.bootcount 2>/dev/null || echo 0)
[ "$COUNT" -ge 3 ] && { echo "zeromount: bootloop guard triggered (count=$COUNT), skipping pipeline" > /dev/kmsg 2>/dev/null; exit 0; }

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 1
[ -x "$BIN" ] || exit 1

"$BIN" mount --post-boot

# Reset bootloop counter only after the system actually finishes booting.
# Pipeline no longer resets it — catches post-pipeline deadlocks.
(
    trap 'exit 0' TERM INT
    i=0
    while [ "$i" -lt 180 ]; do
        [ "$(getprop sys.boot_completed)" = "1" ] && {
            rm -f /data/adb/zeromount/.bootcount
            exit 0
        }
        sleep 1
        i=$((i + 1))
    done
) &

# Deferred SUSFS — waits for sdcard decryption via inotify, then retries path hiding
"$BIN" mount --susfs-retry --wait &
