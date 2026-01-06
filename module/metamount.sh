MODDIR=${0%/*}
LOADER="$MODDIR/bin/nm"
MODULES_DIR="/data/adb/modules"
LOG_FILE="/data/adb/nomount.log"
TARGET_PARTITIONS="system vendor product system_ext odm oem"
log_msg() {
    echo "[$(date +%H:%M:%S)] $1" >> "$LOG_FILE"
}
if [ ! -e "/dev/nomount" ]; then
    echo "FATAL: /dev/nomount missing." > "$LOG_FILE"
    touch "$MODDIR/disable"
    exit 1
fi
"$LOADER" clear >> "$LOG_FILE" 2>&1
log_msg "NoMount: Rules cleared. Starting scan..."
for mod_path in "$MODULES_DIR"/*; do
    [ -d "$mod_path" ] || continue
    mod_name="${mod_path##*/}"
    [ "$mod_name" = "nomount" ] && continue
    if [ -f "$mod_path/disable" ] || [ -f "$mod_path/remove" ]; then
        continue
    fi
    for partition in $TARGET_PARTITIONS; do
        if [ -d "$mod_path/$partition" ]; then
            log_msg "Processing $mod_name -> /$partition"
            (
                cd "$mod_path" || exit
                find "$partition" -type f | while read -r relative_path; do
                    real_path="$mod_path/$relative_path"
                    virtual_path="/$relative_path"
                    "$LOADER" add "$virtual_path" "$real_path" >> /dev/null 2>&1
                done
            )
        fi
    done
done
log_msg "NoMount: Injection complete."
