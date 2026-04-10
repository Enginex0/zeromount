#!/system/bin/sh
# KSU/APatch metamodule mount hook (post-fs-data phase).
# Runs the full mount pipeline BEFORE notify-module-mounted so all
# overlays/bind mounts are established before Zygote starts.
MODDIR="${0%/*}"
LOG="zeromount"

# Single-instance lock -- KSU metamodule mode can double-fire.
# noclobber makes the redirect use O_CREAT|O_EXCL, which is atomic.
LOCKFILE="/dev/zeromount_metamount_lock"
( set -o noclobber; > "$LOCKFILE" ) 2>/dev/null || { ksud kernel notify-module-mounted 2>/dev/null; exit 0; }

echo "$LOG: metamount.sh entered (post-fs-data)" > /dev/kmsg 2>/dev/null

. "$MODDIR/common.sh"

mkdir -p /data/adb/zeromount/flags

# Reconcile external SUSFS module config changes made outside zeromount
EXTERNAL=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null || echo none)
if [ "$EXTERNAL" != "none" ] && [ -n "$ABI" ] && [ -x "$BIN" ]; then
    "$BIN" bridge reconcile "$EXTERNAL" 2>/dev/null
fi

if [ -n "$ABI" ] && [ -x "$BIN" ]; then
    echo "$LOG: starting mount pipeline (pre-zygote)" > /dev/kmsg 2>/dev/null
    timeout 60 "$BIN" mount
    RET=$?
    if [ "$RET" -eq 124 ]; then
        echo "$LOG: mount pipeline hung after 60s, forced termination" > /dev/kmsg 2>/dev/null
    fi
    echo "$LOG: mount pipeline exited (rc=$RET)" > /dev/kmsg 2>/dev/null

else
    echo "$LOG: binary not found (ABI=$ABI), skipping pipeline" > /dev/kmsg 2>/dev/null
fi

echo "$LOG: calling notify-module-mounted" > /dev/kmsg 2>/dev/null
ksud kernel notify-module-mounted 2>/dev/null
exit 0
