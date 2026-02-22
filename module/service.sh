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

# Track background PIDs for cleanup
_bg_pids=""

hide_usb_debugging() {
    ENABLED=$("$BIN" config get adb.hide_usb_debugging 2>/dev/null)
    if [ "$ENABLED" != "true" ]; then
        echo "zeromount: hide_usb_debugging disabled, skipping" > /dev/kmsg 2>/dev/null
        return 0
    fi

    if ! command -v resetprop >/dev/null 2>&1; then
        echo "zeromount: resetprop not found, skipping USB debug hiding" > /dev/kmsg 2>/dev/null
        return 1
    fi

    echo "zeromount: hide_usb_debugging starting" > /dev/kmsg 2>/dev/null

    # Dynamic prop discovery from build.prop files
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
    echo "zeromount: dynamic props overridden: ${_dyn_count}" > /dev/kmsg 2>/dev/null

    resetprop -n init.svc.adbd stopped
    resetprop -n service.adb.root 0
    resetprop -n service.adb.tcp.port -1
    resetprop -n persist.service.adb.enable 0
    resetprop -n persist.vendor.usb.config none
    resetprop -n vendor.usb.config none

    resetprop -n ro.debuggable 0
    resetprop -n persist.sys.debuggable 0
    resetprop -n persist.service.debuggerd.enable 0
    resetprop -n dalvik.vm.checkjni false
    resetprop -n ro.kernel.android.checkjni 0
    resetprop -n ro.boot.vbmeta.device_state locked
    resetprop -n ro.boot.verifiedbootstate green
    resetprop -n ro.boot.flash.locked 1
    resetprop -n ro.boot.warranty_bit 0
    resetprop -n ro.warranty_bit 0
    resetprop -n ro.boot.mode normal
    resetprop -n ro.bootmode normal

    echo "zeromount: static debug props set (19 props via resetprop -n)" > /dev/kmsg 2>/dev/null

    # USB props + adbd loop: must wait for boot_completed so USB HAL is stable
    {
        while [ "$(getprop sys.boot_completed)" != "1" ]; do sleep 1; done
        echo "zeromount: boot completed, spoofing USB props" > /dev/kmsg 2>/dev/null
        while true; do
            resetprop -n persist.sys.usb.config mtp
            resetprop -n sys.usb.config mtp
            resetprop -n sys.usb.state mtp
            resetprop -n sys.usb.ffs.ready 0
            resetprop -n sys.usb.ffs.adb.ready 0
            resetprop -n persist.sys.usb.reboot.func mtp
            resetprop -n init.svc.adbd stopped
            sleep 2
        done
    } &
    _bg_pids="$_bg_pids $!"
    echo "zeromount: USB spoof + adbd loop started (pid $!)" > /dev/kmsg 2>/dev/null
}
hide_usb_debugging

# Performance tuning + input boost daemon (Rust-native, auto-detects device)
if [ "$("$BIN" config get perf.enabled 2>/dev/null)" = "true" ]; then
    "$BIN" perf &
    _bg_pids="$_bg_pids $!"
fi

# Final trap covers all background PIDs
trap 'kill $_bg_pids 2>/dev/null; rm -f "$LOCKFILE"' EXIT
wait
