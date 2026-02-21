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

spoof_props() {
    ENABLED=$("$BIN" config get brene.prop_spoofing 2>/dev/null)
    [ "$ENABLED" != "true" ] && return 0

    if ! command -v resetprop >/dev/null 2>&1; then
        echo "zeromount: resetprop not found, skipping prop spoofing" > /dev/kmsg 2>/dev/null
        return 1
    fi

    set_prop() {
        CURRENT=$(getprop "$1" 2>/dev/null)
        if [ "$CURRENT" != "$2" ]; then
            resetprop "$1" "$2"
        fi
    }

    set_prop ro.debuggable 0
    set_prop ro.secure 1
    set_prop ro.build.type user
    set_prop ro.build.tags release-keys
    set_prop ro.boot.vbmeta.device_state locked
    set_prop ro.boot.verifiedbootstate green
    set_prop ro.boot.flash.locked 1
    set_prop ro.boot.veritymode enforcing
    set_prop ro.adb.secure 1

    echo "zeromount: prop spoofing applied" > /dev/kmsg 2>/dev/null
}
spoof_props

# Performance tuning + input boost daemon (Rust-native, auto-detects device)
if [ "$("$BIN" config get perf.enabled 2>/dev/null)" = "true" ]; then
    "$BIN" perf &
    _perf_pid=$!
fi

# Reset bootloop counter only after the system actually finishes booting
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
_susfs_pid=$!
trap 'kill $_perf_pid $_susfs_pid 2>/dev/null; rm -f "$LOCKFILE"' EXIT

# Keep service.sh alive while perf daemon and SUSFS retry run
wait
