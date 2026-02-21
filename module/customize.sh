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

# Preserve user config across upgrades, wipe transient state
if [ -d "$ZM_DATA" ]; then
    for f in config.toml .exclusion_list .exclusion_meta.json; do
        [ -f "$ZM_DATA/$f" ] && cp "$ZM_DATA/$f" "/data/local/tmp/.zm_backup_$f"
    done
    rm -rf "$ZM_DATA"
    mkdir -p "$ZM_DATA"
    for f in config.toml .exclusion_list .exclusion_meta.json; do
        [ -f "/data/local/tmp/.zm_backup_$f" ] && mv "/data/local/tmp/.zm_backup_$f" "$ZM_DATA/$f"
    done
    zm_print "  🔄 Config preserved, state reset"
else
    mkdir -p "$ZM_DATA"
fi

mkdir -p "$ZM_DATA/logs"
zm_print "  ✅ Data directory ready"

if [ ! -f "$ZM_DATA/config.toml" ]; then
    zm_print "  🔧 Writing default config"
    "$BIN" config defaults > "$ZM_DATA/config.toml" 2>/dev/null || true
fi


zm_print "🛡️ SUSFS Detection" 0.3 "h"

# Kernel-first detection — probe the actual kernel, not userspace dirs
SUSFS_DETECTED=false

# Method 1: ksu_susfs binary probe (fastest, authoritative)
for susfs_bin in /data/adb/ksu/bin/ksu_susfs /data/adb/ap/bin/ksu_susfs; do
    if [ -x "$susfs_bin" ]; then
        SUSFS_VER=$("$susfs_bin" show version 2>/dev/null)
        if [ -n "$SUSFS_VER" ]; then
            SUSFS_DETECTED=true
            zm_print "  ✅ SUSFS detected via binary: $SUSFS_VER"
            break
        fi
    fi
done

# Method 2: /proc/config.gz kernel config check
if [ "$SUSFS_DETECTED" = false ] && [ -f /proc/config.gz ]; then
    if zcat /proc/config.gz 2>/dev/null | grep -q 'CONFIG_KSU_SUSFS=y'; then
        SUSFS_DETECTED=true
        zm_print "  ✅ SUSFS detected via kernel config"
    fi
fi

if [ "$SUSFS_DETECTED" = true ]; then
    SUSFS_DIR="/data/adb/susfs4ksu"
    SUSFS_CONFIG="$SUSFS_DIR/config.sh"
    mkdir -p "$SUSFS_DIR"
    if [ -f "$SUSFS_CONFIG" ]; then
        zm_print "  🔧 Syncing SUSFS config"
        grep -q '^susfs_log=' "$SUSFS_CONFIG" || echo 'susfs_log=0' >> "$SUSFS_CONFIG"
        grep -q '^avc_log_spoofing=' "$SUSFS_CONFIG" || echo 'avc_log_spoofing=1' >> "$SUSFS_CONFIG"
        grep -q '^hide_sus_mnts_for_all_or_non_su_procs=' "$SUSFS_CONFIG" || echo 'hide_sus_mnts_for_all_or_non_su_procs=1' >> "$SUSFS_CONFIG"
        grep -q '^emulate_vold_app_data=' "$SUSFS_CONFIG" || echo 'emulate_vold_app_data=1' >> "$SUSFS_CONFIG"
        grep -q '^force_hide_lsposed=' "$SUSFS_CONFIG" || echo 'force_hide_lsposed=1' >> "$SUSFS_CONFIG"
        grep -q '^spoof_cmdline=' "$SUSFS_CONFIG" || echo 'spoof_cmdline=0' >> "$SUSFS_CONFIG"
        grep -q '^hide_loops=' "$SUSFS_CONFIG" || echo 'hide_loops=1' >> "$SUSFS_CONFIG"
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
hide_loops=1
force_hide_lsposed=1
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
else
    zm_print "  ⚠️ SUSFS not detected in kernel — skipping config"
fi

zm_print "🚀 Finalizing" 0.3 "h"

echo 0 > "$ZM_DATA/.bootcount"

if command -v chcon >/dev/null 2>&1; then
    find "$MODPATH" -path "*/webroot" -prune -o -exec chcon u:object_r:system_file:s0 {} + 2>/dev/null || true
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
