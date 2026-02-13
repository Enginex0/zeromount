#!/system/bin/sh
# ZeroMount install hook — runs during KSU/APatch module installation.
SKIPUNZIP=1

zm_print() {
  local msg="$1"
  local delay="${2:-0.3}"
  local mode="$3"
  local width=$(( ${#msg} + 3 ))
  [ "$width" -gt 60 ] && width=60
  if [ "$mode" = "h" ]; then
    ui_print ""
    ui_print "$(printf '%*s' "$width" | tr ' ' '=')"
    ui_print " $msg"
    ui_print "$(printf '%*s' "$width" | tr ' ' '=')"
  else
    ui_print "$msg"
  fi
  sleep "$delay"
}

unzip -o "$ZIPFILE" -d "$MODPATH" >&2

ZM_VERSION=$(grep '^version=' "$MODPATH/module.prop" | cut -d= -f2)

ui_print ""
ui_print "==========================================="
ui_print "  ⚡ ZeroMount ${ZM_VERSION} ⚡"
ui_print "==========================================="
ui_print "  🛡️  Ghost-level mount management"
ui_print "  ✅ KSU / APatch compatible"
ui_print "==========================================="
ui_print ""
sleep 0.5

zm_print "📱 Detecting Architecture" 0.3 "h"

. "$MODPATH/common.sh"
if [ -z "$ABI" ]; then
    abort "  ❌ Unsupported architecture: $(uname -m)"
fi

zm_print "  ✅ Architecture: $ABI"

BIN="$MODPATH/bin/${ABI}/zeromount"

if [ ! -f "$BIN" ]; then
    abort "  ❌ Binary not found: bin/${ABI}/zeromount"
fi

chmod 755 "$MODPATH/bin/${ABI}"/*

for d in "$MODPATH"/bin/*/; do
    [ "$d" = "$MODPATH/bin/${ABI}/" ] && continue
    rm -rf "$d"
done

cp "$BIN" "$MODPATH/bin/zm"
chmod 755 "$MODPATH/bin/zm"
zm_print "  ✅ Binary ready"

ZM_DATA="/data/adb/zeromount"
zm_print "📁 Preparing Data" 0.3 "h"

mkdir -p "$ZM_DATA"
mkdir -p "$ZM_DATA/logs"
zm_print "  ✅ Data directory ready"

if [ ! -f "$ZM_DATA/config.toml" ]; then
    zm_print "  🔧 Writing default config"
    "$BIN" config defaults > "$ZM_DATA/config.toml" 2>/dev/null || true
else
    zm_print "  ✅ Existing config preserved"
fi

if [ -d /data/adb/ksu ] || [ -d /data/adb/ap ]; then
    zm_print "🛡️ SUSFS Configuration" 0.3 "h"

    SUSFS_DIR="/data/adb/susfs4ksu"
    SUSFS_CONFIG="$SUSFS_DIR/config.sh"
    mkdir -p "$SUSFS_DIR"
    if [ -f "$SUSFS_CONFIG" ]; then
        zm_print "  🔧 Syncing SUSFS config"
        sed -i 's/^susfs_log=.*/susfs_log=0/' "$SUSFS_CONFIG"
        sed -i 's/^avc_log_spoofing=.*/avc_log_spoofing=1/' "$SUSFS_CONFIG"
        sed -i 's/^hide_sus_mnts_for_all_or_non_su_procs=.*/hide_sus_mnts_for_all_or_non_su_procs=1/' "$SUSFS_CONFIG"
        sed -i 's/^emulate_vold_app_data=.*/emulate_vold_app_data=1/' "$SUSFS_CONFIG"
        sed -i 's/^force_hide_lsposed=.*/force_hide_lsposed=0/' "$SUSFS_CONFIG"
        grep -q '^spoof_cmdline=' "$SUSFS_CONFIG" || echo 'spoof_cmdline=0' >> "$SUSFS_CONFIG"
        grep -q '^hide_loops=' "$SUSFS_CONFIG" || echo 'hide_loops=0' >> "$SUSFS_CONFIG"
        zm_print "  ✅ SUSFS config synced"
    else
        zm_print "  🔧 Seeding SUSFS config"
        cat > "$SUSFS_CONFIG" << 'SUSFS_EOF'
susfs_log=0
sus_su=-1
sus_su_active=2
hide_cusrom=0
hide_vendor_sepolicy=0
hide_compat_matrix=0
hide_gapps=0
hide_revanced=0
spoof_cmdline=0
hide_loops=0
force_hide_lsposed=0
spoof_uname=0
hide_sus_mnts_for_all_or_non_su_procs=1
umount_for_zygote_iso_service=0
auto_try_umount=1
avc_log_spoofing=1
emulate_vold_app_data=1
disable_webui_bin_update=0
kernel_version='default'
kernel_build='default'
SUSFS_EOF
        zm_print "  ✅ SUSFS config created"
    fi
fi

if command -v ksud >/dev/null 2>&1; then
    ksud module config set manage.kernel_umount false 2>/dev/null || true
fi

zm_print "🚀 Finalizing" 0.3 "h"

echo 0 > "$ZM_DATA/.bootcount"

if command -v chcon >/dev/null 2>&1; then
    chcon -R u:object_r:system_file:s0 "$MODPATH" 2>/dev/null || true
    chcon -R u:object_r:adb_data_file:s0 "$ZM_DATA" 2>/dev/null || true
fi

rm -rf "$MODPATH/webroot/webroot" 2>/dev/null

set_perm_recursive "$MODPATH/bin" 0 0 0755 0755
chmod 755 "$MODPATH"/*.sh
set_perm "$MODPATH/module.prop" 0 0 0644

zm_print "  ✅ Permissions set"
zm_print "  ✅ Boot counter reset"

ui_print ""
ui_print "==========================================="
ui_print "  ✨ ZeroMount installed successfully ✨"
ui_print "==========================================="
ui_print ""
