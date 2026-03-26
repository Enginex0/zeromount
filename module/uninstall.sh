#!/system/bin/sh
MODDIR="${0%/*}"
# Remove skip_mount flags we created
if [ -f /data/adb/zeromount/.skipped_modules ]; then
    while IFS= read -r mod_id; do
        rm -f "/data/adb/modules/${mod_id}/skip_mount"
    done < /data/adb/zeromount/.skipped_modules
fi

ZM_DATA="/data/adb/zeromount"
STASH="$ZM_DATA/.stash"

mkdir -p "$STASH"
cp "$ZM_DATA/config.toml" "$STASH/config.toml" 2>/dev/null
cp "$ZM_DATA/config.toml.bak" "$STASH/config.toml.bak" 2>/dev/null

# Remove everything except the stash
find "$ZM_DATA" -mindepth 1 -not -path "$STASH" -not -path "$STASH/*" -exec rm -rf {} + 2>/dev/null
