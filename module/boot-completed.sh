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
            echo "zeromount: kernel_umount enabled via ksud" > /dev/kmsg 2>/dev/null
        fi
    fi
else
    echo "zeromount: kernel_umount deferred to external module ($EXTERNAL_SUSFS)" > /dev/kmsg 2>/dev/null
fi

# Emoji needs pm (package manager), only available post-boot
"$BIN" emoji apply-apps 2>/dev/null || true

# try_umount: discover KSU bind mounts and register kernel umount paths
if [ "$("$BIN" config get brene.try_umount 2>/dev/null)" = "true" ]; then
    "$BIN" try-umount 2>/dev/null || true
fi

# vold-app-data: wait for FUSE sdcard like susfs4ksu
if [ "$("$BIN" config get brene.emulate_vold_app_data 2>/dev/null)" = "true" ]; then
    # 60-second timeout in case FUSE never mounts (no sdcard, encryption failure)
    _waited=0
    until [ -d "/sdcard/Android/data" ] || [ $_waited -ge 60 ]; do
        sleep 1
        _waited=$((_waited + 1))
    done
    [ -d "/sdcard/Android/data" ] && "$BIN" vold-app-data 2>/dev/null || true
fi
