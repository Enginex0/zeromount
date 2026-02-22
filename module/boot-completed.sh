#!/system/bin/sh
MODDIR="${0%/*}"
LOG="zeromount"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

# Bootcount reset — boot completed successfully
rm -f /data/adb/zeromount/.bootcount

# Deferred SUSFS gated by auto_hide_sdcard_data toggle
if [ "$("$BIN" config get brene.auto_hide_sdcard_data 2>/dev/null)" != "true" ]; then
    exit 0
fi

# Skip if susfs4ksu userspace module handles deferred SUSFS
# 3-phase: module dir exists + not disabled/removed + binary executable
susfs4ksu_active() {
    for id in susfs4ksu susfs4ksu_next; do
        dir="/data/adb/modules/$id"
        [ -d "$dir" ] && [ ! -f "$dir/disable" ] && [ ! -f "$dir/remove" ] && return 0
    done
    return 1
}
susfs_binary_found() {
    for p in /data/adb/ksu/bin/ksu_susfs /data/adb/ap/bin/ksu_susfs /data/adb/ksu/bin/susfs; do
        [ -x "$p" ] && return 0
    done
    return 1
}
if susfs4ksu_active && susfs_binary_found; then
    echo "$LOG: susfs4ksu module active with binary, skipping deferred SUSFS" > /dev/kmsg 2>/dev/null
    exit 0
fi

# Wait for FUSE sdcard (matches official susfs4ksu approach)
i=0
until [ -d "/sdcard/Android/data" ]; do
    sleep 1
    i=$((i + 1))
    [ "$i" -ge 120 ] && {
        echo "$LOG: sdcard not ready after 120s, skipping deferred SUSFS" > /dev/kmsg 2>/dev/null
        exit 0
    }
done

"$BIN" mount --susfs-retry
