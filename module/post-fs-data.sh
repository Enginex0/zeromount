#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

"$BIN" detect

mkdir -p /data/adb/zeromount/flags
echo -n "" > /data/adb/zeromount/flags/zygisk_status
chmod 666 /data/adb/zeromount/flags/zygisk_status
HIDE_USB=$("$BIN" config get adb.hide_usb_debugging 2>/dev/null)
if [ "$HIDE_USB" = "true" ] && [ -d "$MODDIR/.zygisk_stash" ]; then
    mv "$MODDIR/.zygisk_stash" "$MODDIR/zygisk"
    echo "zeromount: zygisk dir activated" > /dev/kmsg 2>/dev/null
elif [ "$HIDE_USB" != "true" ] && [ -d "$MODDIR/zygisk" ]; then
    mv "$MODDIR/zygisk" "$MODDIR/.zygisk_stash"
fi

spoof_cosmetic_props() {
    [ "$("$BIN" config get adb.invisible_debugging 2>/dev/null)" = "true" ] || return 0
    command -v resetprop >/dev/null 2>&1 || return 1

    # Cosmetic only — doesn't affect USB/ADB functionality
    resetprop -n ro.debuggable 0
    resetprop -n ro.boot.vbmeta.device_state locked
    resetprop -n ro.boot.verifiedbootstate green
    resetprop -n ro.boot.flash.locked 1
    resetprop -n ro.boot.warranty_bit 0
    resetprop -n ro.warranty_bit 0

    echo "zeromount: cosmetic debug props applied (pre-Zygote)" > /dev/kmsg 2>/dev/null
}
spoof_cosmetic_props

# Magisk has no metamount.sh — run mount pipeline here
if [ -z "$KSU" ] && [ -z "$APATCH" ]; then
    if [ ! -f "/dev/zeromount_metamount_lock" ]; then
        touch "/dev/zeromount_metamount_lock"
        COUNT=$(cat /data/adb/zeromount/.bootcount 2>/dev/null || echo 0)
        if [ "$COUNT" -eq 0 ]; then
            EXTERNAL=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null || echo none)
            [ "$EXTERNAL" != "none" ] && "$BIN" bridge reconcile "$EXTERNAL" 2>/dev/null
            timeout 60 "$BIN" mount
            echo "zeromount: magisk mount pipeline exited (rc=$?)" > /dev/kmsg 2>/dev/null
        else
            echo "zeromount: bootloop guard (count=$COUNT), skipping" > /dev/kmsg 2>/dev/null
        fi
    fi
fi

# ADB Root via axon injection
ADB_ROOT=$("$BIN" config get adb.adb_root 2>/dev/null)
if [ "$ADB_ROOT" != "true" ]; then
    echo "zeromount: adb_root disabled, skipping axon injection" > /dev/kmsg 2>/dev/null
    exit 0
fi

AXON_PATH=/data/adb/axon
INJECT="$MODDIR/bin/${ABI}/axon_inject"

if [ ! -x "$INJECT" ]; then
    echo "zeromount: axon_inject not found at $INJECT" > /dev/kmsg 2>/dev/null
    exit 0
fi
if [ ! -f "$MODDIR/lib/${ABI}/libaxon_init.so" ]; then
    echo "zeromount: libaxon_init.so not found for ${ABI}" > /dev/kmsg 2>/dev/null
    exit 0
fi
if [ ! -f "$MODDIR/lib/${ABI}/libaxon_adbd.so" ]; then
    echo "zeromount: libaxon_adbd.so not found for ${ABI}" > /dev/kmsg 2>/dev/null
    exit 0
fi

echo "zeromount: staging axon libraries to $AXON_PATH" > /dev/kmsg 2>/dev/null
mkdir -p "$AXON_PATH"
cp "$MODDIR/lib/${ABI}/libaxon_init.so" "$AXON_PATH/"
cp "$MODDIR/lib/${ABI}/libaxon_adbd.so" "$AXON_PATH/"
chcon -R u:object_r:system_file:s0 "$AXON_PATH"

# Patch linker config for ADBD APEX namespace
if [ -f /linkerconfig/com.android.adbd/ld.config.txt ]; then
    if ! grep -q "$AXON_PATH" /linkerconfig/com.android.adbd/ld.config.txt; then
        echo "# axon" >> /linkerconfig/com.android.adbd/ld.config.txt
        echo "namespace.default.permitted.paths += $AXON_PATH" >> /linkerconfig/com.android.adbd/ld.config.txt
        echo "zeromount: patched adbd linker config" > /dev/kmsg 2>/dev/null
    fi
fi

echo "zeromount: injecting axon into init (PID 1)" > /dev/kmsg 2>/dev/null
"$INJECT" 1 "$AXON_PATH/libaxon_init.so"
INJECT_RC=$?

if [ "$INJECT_RC" -eq 0 ]; then
    echo "zeromount: axon injection successful" > /dev/kmsg 2>/dev/null
else
    echo "zeromount: axon injection failed (rc=$INJECT_RC)" > /dev/kmsg 2>/dev/null
fi
