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

# Shell-level bootloop guard — metamount.sh is blocking, so a broken
# pipeline would hang boot indefinitely without this check.
COUNT=$(cat /data/adb/zeromount/.bootcount 2>/dev/null || echo 0)
if [ "$COUNT" -gt 0 ]; then
    echo "$LOG: bootloop guard (count=$COUNT), skipping pipeline" > /dev/kmsg 2>/dev/null
    ksud kernel notify-module-mounted 2>/dev/null
    exit 0
fi

. "$MODDIR/common.sh"

mkdir -p /data/adb/zeromount/flags
echo -n "" > /data/adb/zeromount/flags/zygisk_status
chmod 666 /data/adb/zeromount/flags/zygisk_status

if [ -n "$BIN" ] && [ -x "$BIN" ]; then
    HIDE_USB=$("$BIN" config get adb.hide_usb_debugging 2>/dev/null)
    if [ "$HIDE_USB" = "true" ] && [ -d "$MODDIR/.zygisk_stash" ]; then
        mv "$MODDIR/.zygisk_stash" "$MODDIR/zygisk"
        echo "$LOG: zygisk dir activated for USB hiding" > /dev/kmsg 2>/dev/null
    elif [ "$HIDE_USB" != "true" ] && [ -d "$MODDIR/zygisk" ]; then
        mv "$MODDIR/zygisk" "$MODDIR/.zygisk_stash"
        echo "$LOG: zygisk dir stashed (USB hiding disabled)" > /dev/kmsg 2>/dev/null
    fi
fi

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
        echo "$LOG: mount pipeline hung after 60s — forced termination" > /dev/kmsg 2>/dev/null
    fi
    echo "$LOG: mount pipeline exited (rc=$RET)" > /dev/kmsg 2>/dev/null

else
    echo "$LOG: binary not found (ABI=$ABI), skipping pipeline" > /dev/kmsg 2>/dev/null
fi

echo "$LOG: calling notify-module-mounted" > /dev/kmsg 2>/dev/null
ksud kernel notify-module-mounted 2>/dev/null
exit 0
