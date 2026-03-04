#!/system/bin/sh
# ZeroMount install hook — runs during module installation.
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

rm -f "$MODPATH/disable"

for _mgr_bin in /data/adb/ksu/bin /data/adb/ap/bin /data/adb/magisk; do
    [ -d "$_mgr_bin" ] && export PATH="$_mgr_bin:$PATH"
done

ZM_VERSION=$(grep '^version=' "$MODPATH/module.prop" | cut -d= -f2)

ui_print ""
ui_print "==========================================="
ui_print "  ⚡ ZeroMount ${ZM_VERSION} ⚡"
ui_print "==========================================="
ui_print "  🛡️  Ghost-level mount management"
ui_print "  ✅ KSU / APatch / Magisk"
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

set_perm_recursive "$MODPATH/bin/${ABI}" 0 0 0755 0755

for d in "$MODPATH"/bin/*/; do
    [ "$d" = "$MODPATH/bin/${ABI}/" ] && continue
    rm -rf "$d"
done

cp "$BIN" "$MODPATH/bin/zm" || abort "  ❌ Failed to copy binary"
set_perm "$MODPATH/bin/zm" 0 0 0755
zm_print "  ✅ Binary ready"

if [ -c /dev/zeromount ] || [ -e /dev/zeromount ]; then
    zm_print "  ✅ ZeroMount VFS driver detected"
else
    zm_print "  ⚠️ ZeroMount VFS driver not found"
    zm_print "  ⚠️ Module will use overlay/magic mount mode"
    zm_print "  ℹ️ Flash a ZeroMount-patched kernel for VFS mode"
fi

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
    # Snapshot current Android settings so boot-completed doesn't override them
    [ "$(settings get global development_settings_enabled 2>/dev/null)" = "1" ] && \
        "$BIN" config set adb.developer_options true 2>/dev/null
    [ "$(settings get global adb_enabled 2>/dev/null)" = "1" ] && \
        "$BIN" config set adb.usb_debugging true 2>/dev/null
    VBS=$(( 4096 + ($(od -An -tu1 -N1 /dev/urandom) % 8) * 1024 ))
    "$BIN" config set brene.vbmeta_size "$VBS" 2>/dev/null
    zm_print "  ✅ vbmeta_size randomized: $VBS"
else
    zm_print "  ✅ Existing config preserved"
    # Merge new keys: load fills defaults for missing fields, dump writes them back
    "$BIN" config dump > "$ZM_DATA/config.toml.tmp" 2>/dev/null && mv "$ZM_DATA/config.toml.tmp" "$ZM_DATA/config.toml"
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
    set_perm "$EMOJI_DIR/NotoColorEmoji.ttf" 0 0 0644 u:object_r:system_file:s0
    zm_print "  ✅ Emoji font staged"
fi

# Xiaomi/Redmi/POCO devices have mi_ext overlay mounts that trigger detection
BRAND=$(getprop ro.product.brand 2>/dev/null | tr '[:upper:]' '[:lower:]')
MANUFACTURER=$(getprop ro.product.manufacturer 2>/dev/null | tr '[:upper:]' '[:lower:]')

zm_print "🛡️ External Module Bridge" 0.3 "h"

# Import VerifiedBootHash from susfs4ksu's standalone file if present
VBH_FILE="/data/adb/VerifiedBootHash/VerifiedBootHash.txt"
if [ -f "$VBH_FILE" ]; then
    VBH=$(cat "$VBH_FILE" 2>/dev/null | head -1 | tr -d '[:space:]')
    if [ -n "$VBH" ]; then
        "$BIN" config set brene.verified_boot_hash "$VBH" 2>/dev/null
        zm_print "  ✅ VerifiedBootHash imported"
    fi
fi

if "$BIN" detect 2>/dev/null | grep -q 'susfs: true'; then
    if [ "$KSU_SUKISU" = "true" ]; then
        zm_print "  ✅ SUSFS +enhanced (manager-integrated)"
    else
        zm_print "  ✅ SUSFS detected in kernel"
    fi
else
    zm_print "  ⚠️ SUSFS not detected in kernel"
fi

zm_print "  🔄 Syncing SUSFS bidirectional"
"$BIN" bridge init 2>/dev/null && zm_print "  ✅ External configs synced" || zm_print "  ⚠️ Bridge init skipped (binary error)"

zm_print "🚀 Finalizing" 0.3 "h"

echo 0 > "$ZM_DATA/.bootcount"

if command -v chcon >/dev/null 2>&1; then
    find "$MODPATH" -path "*/webroot" -prune -o -exec chcon u:object_r:system_file:s0 {} + 2>/dev/null || true
    chcon -R u:object_r:adb_data_file:s0 "$ZM_DATA" 2>/dev/null || true
fi

rm -rf "$MODPATH/webroot/webroot" 2>/dev/null

# Stage axon libraries
if [ -d "$MODPATH/lib/${ABI}" ]; then
    set_perm_recursive "$MODPATH/lib/${ABI}" 0 0 0755 0644
fi
if [ -f "$MODPATH/bin/${ABI}/axon_inject" ]; then
    set_perm "$MODPATH/bin/${ABI}/axon_inject" 0 0 0755
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
