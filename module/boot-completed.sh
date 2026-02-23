#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

rm -f /data/adb/zeromount/.bootcount

EXTERNAL_SUSFS=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null)

# kernel_umount via ksud — only when no external module handles it
if [ "$EXTERNAL_SUSFS" = "none" ] || [ -z "$EXTERNAL_SUSFS" ]; then
    if [ "$("$BIN" config get brene.kernel_umount 2>/dev/null)" = "true" ]; then
        KSUD=""
        [ -x /data/adb/ksu/bin/ksud ] && KSUD=/data/adb/ksu/bin/ksud
        [ -z "$KSUD" ] && [ -x /data/adb/ap/bin/ksud ] && KSUD=/data/adb/ap/bin/ksud
        if [ -n "$KSUD" ]; then
            "$KSUD" feature set kernel_umount 1 2>/dev/null && \
                "$KSUD" feature save 2>/dev/null
            echo "$LOG: kernel_umount enabled via ksud" > /dev/kmsg 2>/dev/null
        fi
    fi
else
    echo "$LOG: kernel_umount deferred to external module ($EXTERNAL_SUSFS)" > /dev/kmsg 2>/dev/null
fi

# Emoji and vold-app-data need pm (package manager), only available post-boot
"$BIN" emoji apply-apps 2>/dev/null || true
"$BIN" vold-app-data 2>/dev/null || true

# Deferred SUSFS path hiding (Rust handles deference internally)
if [ "$("$BIN" config get brene.auto_hide_sdcard_data 2>/dev/null)" != "true" ]; then
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
