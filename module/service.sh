MODDIR=${0%/*}
ZEROMOUNT_DATA="/data/adb/zeromount"
LOADER="$MODDIR/bin/zm"
LOG_FILE="$ZEROMOUNT_DATA/service.log"
SUSFS_BIN="/data/adb/ksu/bin/ksu_susfs"

mkdir -p "$ZEROMOUNT_DATA"

# Hide ZeroMount kernel artifacts from detection apps
# Use sus_path_loop for persistence across zygote spawns
if [ -x "$SUSFS_BIN" ]; then
    "$SUSFS_BIN" add_sus_path_loop /dev/zeromount 2>/dev/null
    "$SUSFS_BIN" add_sus_path_loop /sys/kernel/zeromount 2>/dev/null
fi

if [ -f "$ZEROMOUNT_DATA/.exclusion_list" ]; then
    while IFS= read -r uid; do
        [ -z "$uid" ] && continue
        if ! "$LOADER" block "$uid" 2>/dev/null; then
            echo "[ERROR] Failed to block UID $uid" >> "$LOG_FILE"
        fi
    done < "$ZEROMOUNT_DATA/.exclusion_list"
fi
