#!/system/bin/sh
# ZeroMount Metamodule Uninstall Hook

MODDIR="${0%/*}"
LOADER="$MODDIR/bin/zm"
ZEROMOUNT_DATA="/data/adb/zeromount"

# Clear all VFS redirection rules
if [ -x "$LOADER" ]; then
    "$LOADER" clear 2>/dev/null
    "$LOADER" disable 2>/dev/null
fi

# Cleanup data directory
[ -d "$ZEROMOUNT_DATA" ] && rm -rf "$ZEROMOUNT_DATA"

exit 0
