#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

if command -v getevent >/dev/null 2>&1; then
    _keys=$(timeout 3 getevent -lq 2>/dev/null || true)
    if echo "$_keys" | grep -q 'KEY_VOLUMEDOWN.*DOWN' && \
       echo "$_keys" | grep -q 'KEY_VOLUMEUP.*DOWN'; then
        echo "zeromount: vol-up+down safe mode triggered, running guard recovery" > /dev/kmsg 2>/dev/null
        "$BIN" guard recover 2>/dev/null
    fi
fi

"$BIN" detect

mkdir -p /data/adb/zeromount/flags

# Magisk has no metamount.sh — run mount pipeline here
if [ -z "$KSU" ] && [ -z "$APATCH" ]; then
    if [ ! -f "/dev/zeromount_metamount_lock" ]; then
        touch "/dev/zeromount_metamount_lock"
        EXTERNAL=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null || echo none)
        [ "$EXTERNAL" != "none" ] && "$BIN" bridge reconcile "$EXTERNAL" 2>/dev/null
        timeout 60 "$BIN" mount
        echo "zeromount: magisk mount pipeline exited (rc=$?)" > /dev/kmsg 2>/dev/null
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
timeout 5 "$INJECT" 1 "$AXON_PATH/libaxon_init.so"
INJECT_RC=$?

if [ "$INJECT_RC" -eq 0 ]; then
    echo "zeromount: axon injection successful" > /dev/kmsg 2>/dev/null
else
    echo "zeromount: axon injection failed (rc=$INJECT_RC)" > /dev/kmsg 2>/dev/null
fi
