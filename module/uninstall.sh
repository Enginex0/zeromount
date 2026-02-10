#!/system/bin/sh
MODDIR="${0%/*}"
# Remove skip_mount flags we created
if [ -f /data/adb/zeromount/.skipped_modules ]; then
    while IFS= read -r mod_id; do
        rm -f "/data/adb/modules/${mod_id}/skip_mount"
    done < /data/adb/zeromount/.skipped_modules
fi

rm -rf /data/adb/zeromount
