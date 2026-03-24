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

zm_print "🔍 Checking Metamodule Conflicts" 0.3 "h"

_found_conflict=false
for _mod_dir in /data/adb/modules/*/; do
    [ -d "$_mod_dir" ] || continue
    _mod_id=$(basename "$_mod_dir")
    [ "$_mod_id" = "meta-zeromount" ] && continue
    _prop="$_mod_dir/module.prop"
    [ -f "$_prop" ] || continue
    _meta=$(grep -E '^metamodule[[:space:]]*=' "$_prop" 2>/dev/null | head -1 | sed 's/.*=[[:space:]]*//' | tr -d '[:space:]')
    case "$_meta" in
        1|true)
            _conflict_name=$(grep -E '^name[[:space:]]*=' "$_prop" 2>/dev/null | head -1 | sed 's/.*=[[:space:]]*//')
            zm_print "  ⚠️ Removing metamodule: ${_conflict_name:-$_mod_id}"
            # Run its uninstall hook if present
            [ -f "$_mod_dir/uninstall.sh" ] && sh "$_mod_dir/uninstall.sh" 2>/dev/null
            touch "$_mod_dir/remove"
            touch "$_mod_dir/disable"
            _found_conflict=true
            ;;
    esac
done
unset _mod_dir _mod_id _prop _meta _conflict_name

if [ "$_found_conflict" = true ]; then
    zm_print "  ✅ Conflicting metamodules marked for removal"
else
    zm_print "  ✅ No conflicts found"
fi
unset _found_conflict

# Older KSU/APatch use sparse-backed overlayfs that conflicts with zeromount's overlay mode
if [ -n "$KSU" ] && [ "${KSU_VER_CODE:-0}" -lt 22098 ] && [ "$KSU_MAGIC_MOUNT" != "true" ]; then
    zm_print "  ⚠️ KSU $KSU_VER_CODE uses sparse overlayfs; overlay mode may conflict"
    zm_print "  ℹ️ Update KSU to 22098+ or enable magic mount"
fi
if [ -n "$APATCH" ] && [ "${APATCH_VER_CODE:-0}" -lt 11170 ] && [ "$APATCH_BIND_MOUNT" != "true" ]; then
    zm_print "  ⚠️ APatch $APATCH_VER_CODE uses sparse overlayfs; overlay mode may conflict"
    zm_print "  ℹ️ Update APatch to 11170+ or enable bind mount"
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
    # Detect device language for WebUI + description strings
    DEVICE_LANG=$(getprop ro.system.locale 2>/dev/null)
    [ -z "$DEVICE_LANG" ] && DEVICE_LANG=$(getprop persist.sys.locale 2>/dev/null)
    [ -z "$DEVICE_LANG" ] && DEVICE_LANG=$(getprop ro.product.locale 2>/dev/null)
    LANG_CODE=$(printf '%s' "$DEVICE_LANG" | sed 's/_/-/g')
    case "$LANG_CODE" in
      zh-Hans*|zh-CN*) LANG_CODE="zh-CN" ;;
      zh-Hant*|zh-TW*) LANG_CODE="zh-TW" ;;
      pt-BR*) LANG_CODE="pt-BR" ;;
      pt*) LANG_CODE="pt-PT" ;;
      *) LANG_CODE=$(printf '%s' "$LANG_CODE" | cut -d- -f1) ;;
    esac
    case "$LANG_CODE" in
      af|ar|bg|bn|ca|cs|da|de|el|en|es|fa|fi|fr|he|hi|hu|id|it|ja|ko|nl|no|pl|pt-BR|pt-PT|ro|ru|sr|sv|th|tr|uk|vi|zh-CN|zh-TW) ;;
      *) LANG_CODE="en" ;;
    esac
    "$BIN" config set ui.language "$LANG_CODE" 2>/dev/null
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

_detect_out=$("$BIN" detect 2>/dev/null)
if echo "$_detect_out" | grep -q 'susfs: true'; then
    if [ "$KSU_SUKISU" = "true" ]; then
        zm_print "  ✅ SUSFS +enhanced (manager-integrated)"
    else
        zm_print "  ✅ SUSFS detected in kernel"
    fi
    _susfs_ver=$(echo "$_detect_out" | grep '  version:' | sed 's/.*version: //')
    case "$_susfs_ver" in
        1.5.10|1.5.11)
            zm_print "  ⚠️ SUSFS $_susfs_ver has known overlay mount conflicts"
            zm_print "  ℹ️ VFS mode is primary; overlay fallback may be unreliable"
            ;;
    esac
    unset _susfs_ver
else
    zm_print "  ⚠️ SUSFS not detected in kernel"
fi
unset _detect_out

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
