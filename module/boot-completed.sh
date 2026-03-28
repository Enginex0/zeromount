#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

rm -f /data/adb/zeromount/.bootcount

# Deferred BRENE path hiding (add_sus_path, add_sus_path_loop, add_sus_map)
# Real BRENE runs all path ops at boot-completed, not post-fs-data
"$BIN" hide-paths 2>/dev/null || true

# Emoji needs pm (package manager), only available post-boot
"$BIN" emoji apply-apps 2>/dev/null || true

# try_umount: discover KSU bind mounts and register kernel umount paths
if [ "$("$BIN" config get brene.try_umount 2>/dev/null)" = "true" ]; then
    "$BIN" try-umount 2>/dev/null || true
fi

# vold-app-data: wait for FUSE sdcard like susfs4ksu
if [ "$("$BIN" config get brene.emulate_vold_app_data 2>/dev/null)" = "true" ]; then
    _waited=0
    until [ -d "/sdcard/Android/data" ] || [ $_waited -ge 60 ]; do
        sleep 1
        _waited=$((_waited + 1))
    done
    [ $_waited -ge 60 ] && echo "zeromount: vold-app-data skipped, /sdcard/Android/data not ready after 60s" > /dev/kmsg 2>/dev/null
    [ -d "/sdcard/Android/data" ] && "$BIN" vold-app-data 2>/dev/null || true
fi
