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

FRESH_INSTALL=false
if [ ! -f "$ZM_DATA/config.toml" ]; then
    FRESH_INSTALL=true
fi

# Upgrade detection (PIF-style): old module dir exists → preserve settings,
# clean stale state, and restore SELinux contexts on peer modules.
IS_UPGRADE=false
OLD_MODULE="/data/adb/modules/meta-zeromount"
if [ -d "$OLD_MODULE" ] && [ "$FRESH_INSTALL" = false ]; then
    IS_UPGRADE=true
fi

mkdir -p "$ZM_DATA"
mkdir -p "$ZM_DATA/logs"
zm_print "  ✅ Data directory ready"

if [ "$FRESH_INSTALL" = true ]; then
    zm_print "  🔧 Writing default config"
    "$BIN" config defaults > "$ZM_DATA/config.toml" 2>/dev/null || true
else
    zm_print "  ✅ Existing config preserved"
fi

if [ "$IS_UPGRADE" = true ]; then
    # Purge staged fonts/emoji so next boot regenerates them with correct SELinux context
    rm -rf "$ZM_DATA/fonts" 2>/dev/null
    rm -rf "$ZM_DATA/emoji" 2>/dev/null
    # Restore system_file on peer modules' system/ dirs — KSU sets this during
    # their install, but it can be lost between reboots on some firmware
    if command -v chcon >/dev/null 2>&1; then
        for mod_dir in /data/adb/modules/*/system; do
            case "$mod_dir" in */meta-zeromount/system) continue ;; esac
            [ -d "$mod_dir" ] && chcon -R u:object_r:system_file:s0 "$mod_dir" 2>/dev/null
        done
    fi
    zm_print "  ✅ Upgrade: peer module contexts restored"
fi

# Stage emoji font for runtime use
EMOJI_DIR="$ZM_DATA/emoji"
mkdir -p "$EMOJI_DIR"
if [ -f "$MODPATH/emoji/NotoColorEmoji.ttf" ]; then
    cp "$MODPATH/emoji/NotoColorEmoji.ttf" "$EMOJI_DIR/" 2>/dev/null
    chmod 644 "$EMOJI_DIR/NotoColorEmoji.ttf" 2>/dev/null
    chcon u:object_r:system_file:s0 "$EMOJI_DIR/NotoColorEmoji.ttf" 2>/dev/null
    zm_print "  ✅ Emoji font staged"
fi

# Xiaomi/Redmi/POCO devices have mi_ext overlay mounts that trigger detection
BRAND=$(getprop ro.product.brand 2>/dev/null | tr '[:upper:]' '[:lower:]')
MANUFACTURER=$(getprop ro.product.manufacturer 2>/dev/null | tr '[:upper:]' '[:lower:]')

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

if [ "$SUSFS_DETECTED" != true ]; then
    zm_print "  ⚠️ SUSFS not detected in kernel"
fi

zm_print "🚀 Finalizing" 0.3 "h"

echo 0 > "$ZM_DATA/.bootcount"

if command -v chcon >/dev/null 2>&1; then
    find "$MODPATH" -path "*/webroot" -prune -o -exec chcon u:object_r:system_file:s0 {} + 2>/dev/null || true
    chcon -R u:object_r:adb_data_file:s0 "$ZM_DATA" 2>/dev/null || true
fi

rm -rf "$MODPATH/webroot/webroot" 2>/dev/null

# Stage adbex libraries
if [ -d "$MODPATH/lib/${ABI}" ]; then
    chmod 644 "$MODPATH/lib/${ABI}"/*.so 2>/dev/null
fi
if [ -f "$MODPATH/bin/${ABI}/adbex_inject" ]; then
    chmod 755 "$MODPATH/bin/${ABI}/adbex_inject"
fi

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
