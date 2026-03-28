#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

rm -f /data/adb/zeromount/.bootcount

"$BIN" hide-paths 2>/dev/null || true

"$BIN" emoji apply-apps 2>/dev/null || true

if [ "$("$BIN" config get brene.try_umount 2>/dev/null)" = "true" ]; then
    "$BIN" try-umount 2>/dev/null || true
fi

if [ "$("$BIN" config get brene.emulate_vold_app_data 2>/dev/null)" = "true" ]; then
    _waited=0
    until [ -d "/sdcard/Android/data" ] || [ $_waited -ge 60 ]; do
        sleep 1
        _waited=$((_waited + 1))
    done
    [ $_waited -ge 60 ] && echo "zeromount: vold-app-data skipped, /sdcard/Android/data not ready after 60s" > /dev/kmsg 2>/dev/null
    [ -d "/sdcard/Android/data" ] && "$BIN" vold-app-data 2>/dev/null || true
fi

start logd
echo "zeromount: logd health check (start)" > /dev/kmsg 2>/dev/null
