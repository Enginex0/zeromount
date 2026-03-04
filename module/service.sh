#!/system/bin/sh
# Deferred post-boot tasks only — mount pipeline runs in metamount.sh.
MODDIR="${0%/*}"

# Single-instance guard (atomic via noclobber)
LOCKFILE="/dev/zeromount_lock"
( set -o noclobber; echo $$ > "$LOCKFILE" ) 2>/dev/null || exit 0
trap 'rm -f "$LOCKFILE"' EXIT
trap 'exit 0' INT TERM

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 1
[ -x "$BIN" ] || exit 1

if [ -f /data/adb/zeromount/.verbose ]; then
    echo "zeromount: restoring verbose mode from marker" > /dev/kmsg
    $BIN log level 3
fi

"$BIN" guard record-svc 2>/dev/null

# Track background PIDs for cleanup
_bg_pids=""

# Device-wide boot monitors
{
    "$BIN" guard watch-boot 2>/dev/null || \
        echo "zeromount: boot timeout, guard recovery" > /dev/kmsg 2>/dev/null
} &
_bg_pids="$_bg_pids $!"

{
    "$BIN" guard watch-zygote 2>/dev/null || \
        echo "zeromount: zygote unstable, guard recovery" > /dev/kmsg 2>/dev/null
} &
_bg_pids="$_bg_pids $!"

"$BIN" guard watch-systemui 2>/dev/null &
_bg_pids="$_bg_pids $!"

# Manual safe mode: volume key combo check every second until boot completes
if command -v getevent >/dev/null 2>&1; then
    {
        while [ "$(getprop sys.boot_completed)" != "1" ]; do
            if timeout 1 getevent -lqn 2>/dev/null | grep -q 'KEY_VOLUMEDOWN.*DOWN'; then
                echo "zeromount: vol-down safe mode triggered (service), running guard recovery" > /dev/kmsg 2>/dev/null
                "$BIN" guard recover 2>/dev/null
                break
            fi
        done
    } &
    _bg_pids="$_bg_pids $!"
fi

PROP_SPOOF=$("$BIN" config get brene.prop_spoofing 2>/dev/null)

if [ "$PROP_SPOOF" = "true" ]; then
    if command -v resetprop >/dev/null 2>&1; then
        {
            while true; do
                "$BIN" prop-watch
                rc=$?
                [ $rc -eq 0 ] && break
                echo "zeromount: prop-watch crashed ($rc), restarting" > /dev/kmsg 2>/dev/null
                sleep 1
            done
        } &
        _bg_pids="$_bg_pids $!"
        echo "zeromount: prop-watch daemon started (pid $!)" > /dev/kmsg 2>/dev/null
    else
        echo "zeromount: resetprop not found, skipping prop-watch" > /dev/kmsg 2>/dev/null
    fi
fi

# Performance tuning + input boost daemon (Rust-native, auto-detects device)
if [ "$("$BIN" config get perf.enabled 2>/dev/null)" = "true" ]; then
    "$BIN" perf &
    _bg_pids="$_bg_pids $!"
fi

# Magisk has no boot-completed callback — emulate it
if [ -z "$KSU" ] && [ -z "$APATCH" ]; then
    (
        while [ "$(getprop sys.boot_completed)" != "1" ]; do sleep 1; done
        sh "$MODDIR/boot-completed.sh"
    ) &
    _bg_pids="$_bg_pids $!"
fi

trap 'kill $_bg_pids 2>/dev/null; rm -f "$LOCKFILE"' EXIT
wait
