#!/system/bin/sh
# KSU/APatch metamodule mount hook (post-fs-data phase).
# Runs the full mount pipeline BEFORE notify-module-mounted so all
# overlays/bind mounts are established before Zygote starts.
MODDIR="${0%/*}"
LOG="zeromount"

# Single-instance lock -- KSU metamodule mode can double-fire
LOCKFILE="/dev/zeromount_metamount_lock"
[ -f "$LOCKFILE" ] && { ksud kernel notify-module-mounted 2>/dev/null; exit 0; }
touch "$LOCKFILE"

echo "$LOG: metamount.sh entered (post-fs-data)" > /dev/kmsg 2>/dev/null

# Shell-level bootloop guard — metamount.sh is blocking, so a broken
# pipeline would hang boot indefinitely without this check.
COUNT=$(cat /data/adb/zeromount/.bootcount 2>/dev/null || echo 0)
if [ "$COUNT" -gt 0 ]; then
    echo "$LOG: bootloop guard (count=$COUNT), skipping pipeline" > /dev/kmsg 2>/dev/null
    ksud kernel notify-module-mounted 2>/dev/null
    exit 0
fi

. "$MODDIR/common.sh"

if [ -n "$ABI" ] && [ -x "$BIN" ]; then
    echo "$LOG: starting mount pipeline (pre-zygote)" > /dev/kmsg 2>/dev/null
    timeout 60 "$BIN" mount
    RET=$?
    if [ "$RET" -eq 124 ]; then
        echo "$LOG: mount pipeline hung after 60s — forced termination" > /dev/kmsg 2>/dev/null
    fi
    echo "$LOG: mount pipeline exited (rc=$RET)" > /dev/kmsg 2>/dev/null

else
    echo "$LOG: binary not found (ABI=$ABI), skipping pipeline" > /dev/kmsg 2>/dev/null
fi

echo "$LOG: calling notify-module-mounted" > /dev/kmsg 2>/dev/null
ksud kernel notify-module-mounted 2>/dev/null
exit 0
