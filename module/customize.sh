#!/system/bin/sh
# ZeroMount install hook — runs during KSU/APatch module installation.
# KSU04: No manage.kernel_umount declaration (VFS mode has no mounts).
SKIPUNZIP=1

ui_print "- Installing ZeroMount v2.0.0"

# Extract module files
unzip -o "$ZIPFILE" -d "$MODPATH" >&2

# Detect architecture
case "$(uname -m)" in
    aarch64)       ARCH=arm64 ;;
    armv7*|armv8l) ARCH=arm ;;
    x86_64)        ARCH=x86_64 ;;
    i686|i386)     ARCH=x86 ;;
    *)
        abort "! Unsupported architecture: $(uname -m)"
        ;;
esac

ui_print "- Architecture: $ARCH"

BIN="$MODPATH/zm-${ARCH}"

if [ ! -f "$BIN" ]; then
    abort "! Binary not found: zm-${ARCH}"
fi

# Set executable permissions on all binaries
chmod 755 "$MODPATH"/zm-*

# Remove binaries for other architectures to save space
for f in "$MODPATH"/zm-*; do
    [ "$f" = "$BIN" ] && continue
    rm -f "$f"
done

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

# Set permissions on shell scripts
set_perm_recursive "$MODPATH" 0 0 0755 0644
chmod 755 "$MODPATH"/*.sh
chmod 755 "$BIN"

ui_print "- ZeroMount installed successfully"
