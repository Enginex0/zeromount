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

# Rust detect phase writes which external SUSFS module (if any) is active
EXTERNAL_SUSFS=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null)

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
            resetprop -n "$1" "$2"
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

    set_prop ro.crypto.state encrypted
    set_prop ro.force.debuggable 0
    set_prop ro.kernel.qemu ""
    set_prop ro.secureboot.lockstate locked
    set_prop ro.is_ever_orange 0
    set_prop ro.bootmode normal
    set_prop ro.bootimage.build.tags release-keys
    set_prop vendor.boot.vbmeta.device_state locked
    set_prop vendor.boot.verifiedbootstate green
    set_prop ro.boot.realme.lockstate 1
    set_prop ro.boot.realmebootstate green
    set_prop ro.boot.verifiedbooterror ""
    set_prop ro.boot.veritymode.managed yes
    VBS=$("$BIN" config get brene.vbmeta_size 2>/dev/null)
    [ -z "$VBS" ] && VBS=4096
    set_prop ro.boot.vbmeta.size "$VBS"
    set_prop ro.boot.vbmeta.hash_alg sha256
    set_prop ro.boot.vbmeta.avb_version 1.3
    set_prop ro.boot.vbmeta.invalidate_on_error yes
    set_prop sys.oem_unlock_allowed 0

    for prop in ro.warranty_bit ro.vendor.boot.warranty_bit \
                ro.vendor.warranty_bit ro.boot.warranty_bit; do
        set_prop "$prop" "0"
    done

    VBH=$("$BIN" config get brene.verified_boot_hash 2>/dev/null)
    [ -n "$VBH" ] && set_prop "ro.boot.vbmeta.digest" "$VBH"

    echo "zeromount: prop spoofing applied" > /dev/kmsg 2>/dev/null
}
# External module handles prop spoofing — skip to avoid redundant resetprop calls
if [ "$EXTERNAL_SUSFS" = "none" ] || [ -z "$EXTERNAL_SUSFS" ]; then
    spoof_props
else
    echo "zeromount: prop spoofing deferred to external module ($EXTERNAL_SUSFS)" > /dev/kmsg 2>/dev/null
fi

# Track background PIDs for cleanup
_bg_pids=""

# Cosmetic prop spoofing — hides debug build fingerprints without affecting ADB/USB functionality.
# Per-app ADB hiding requires Zygisk hooks (Java) and kernel patches (procfs/sysfs).
spoof_cosmetic_debug_props() {
    ENABLED=$("$BIN" config get adb.invisible_debugging 2>/dev/null)
    if [ "$ENABLED" != "true" ]; then
        echo "zeromount: invisible_debugging disabled, skipping" > /dev/kmsg 2>/dev/null
        return 0
    fi
    command -v resetprop >/dev/null 2>&1 || return 1

    PROP_FILE="/data/adb/zeromount/.hide_debug_props"
    rm -f "$PROP_FILE"
    _dyn_count=0
    for propfile in /default.prop /system/build.prop /vendor/build.prop \
                    /product/build.prop /vendor/odm/etc/build.prop \
                    /system/system/build.prop /system_ext/build.prop; do
        [ -f "$propfile" ] || continue
        grep "^ro\." "$propfile" | grep "userdebug" >> "$PROP_FILE" 2>/dev/null
        grep "^ro\." "$propfile" | grep "test-keys" >> "$PROP_FILE" 2>/dev/null
    done
    if [ -f "$PROP_FILE" ]; then
        _dyn_count=$(wc -l < "$PROP_FILE")
        sed -i 's/userdebug/user/g' "$PROP_FILE"
        sed -i 's/test-keys/release-keys/g' "$PROP_FILE"
        resetprop --file "$PROP_FILE" 2>/dev/null
        rm -f "$PROP_FILE"
    fi

    resetprop -n ro.debuggable 0
    resetprop -n ro.boot.vbmeta.device_state locked
    resetprop -n ro.boot.verifiedbootstate green
    resetprop -n ro.boot.flash.locked 1
    resetprop -n ro.boot.warranty_bit 0
    resetprop -n ro.warranty_bit 0
    resetprop -n ro.boot.mode normal
    resetprop -n ro.bootmode normal

    echo "zeromount: cosmetic debug props set (${_dyn_count} dynamic + 8 static)" > /dev/kmsg 2>/dev/null
}
spoof_cosmetic_debug_props

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
