#!/system/bin/sh
# ZeroMount install hook — runs during KSU/APatch module installation.
# KSU04: No manage.kernel_umount declaration (VFS mode has no mounts).
SKIPUNZIP=1

# Extract module files
unzip -o "$ZIPFILE" -d "$MODPATH" >&2

ZM_VERSION=$(grep '^version=' "$MODPATH/module.prop" | cut -d= -f2)
ui_print "- Installing ZeroMount ${ZM_VERSION}"

# Detect architecture and map to ABI directory
case "$(uname -m)" in
    aarch64)       ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64)        ABI=x86_64 ;;
    i686|i386)     ABI=x86 ;;
    *)
        abort "! Unsupported architecture: $(uname -m)"
        ;;
esac

ui_print "- Architecture: $ABI"

BIN="$MODPATH/bin/${ABI}/zeromount"

if [ ! -f "$BIN" ]; then
    abort "! Binary not found: bin/${ABI}/zeromount"
fi

# Set executable permissions on this arch's binaries
chmod 755 "$MODPATH/bin/${ABI}"/*

# Remove other architecture directories to save space
for d in "$MODPATH"/bin/*/; do
    [ "$d" = "$MODPATH/bin/${ABI}/" ] && continue
    rm -rf "$d"
done

# Stable binary path for WebUI and shell scripts — no symlinks
cp "$BIN" "$MODPATH/bin/zm"
chmod 755 "$MODPATH/bin/zm"

# Create persistent data directory
ZM_DATA="/data/adb/zeromount"
mkdir -p "$ZM_DATA"
mkdir -p "$ZM_DATA/logs"

# Write default config if not present (preserve user config on upgrade)
if [ ! -f "$ZM_DATA/config.toml" ]; then
    ui_print "- Writing default config.toml"
    "$BIN" config defaults > "$ZM_DATA/config.toml" 2>/dev/null || true
fi

# Reset bootloop counter on fresh install/upgrade
echo 0 > "$ZM_DATA/.bootcount"

# Set SELinux contexts
if command -v chcon >/dev/null 2>&1; then
    chcon -R u:object_r:system_file:s0 "$MODPATH" 2>/dev/null || true
    chcon -R u:object_r:adb_data_file:s0 "$ZM_DATA" 2>/dev/null || true
fi

# Set permissions on shell scripts and binaries
set_perm_recursive "$MODPATH" 0 0 0755 0644
chmod 755 "$MODPATH"/*.sh
chmod 755 "$MODPATH/bin/${ABI}"/*
chmod 755 "$MODPATH/bin/zm"

ui_print "- ZeroMount installed successfully"
