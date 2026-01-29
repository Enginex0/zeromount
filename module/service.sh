MODDIR=${0%/*}
ZEROMOUNT_DATA="/data/adb/zeromount"
LOADER="$MODDIR/bin/zm"
LOG_FILE="$ZEROMOUNT_DATA/service.log"

mkdir -p "$ZEROMOUNT_DATA"

if [ -f "$ZEROMOUNT_DATA/.exclusion_list" ]; then
    while IFS= read -r uid; do
        [ -z "$uid" ] && continue
        if ! "$LOADER" block "$uid" 2>/dev/null; then
            echo "[ERROR] Failed to block UID $uid" >> "$LOG_FILE"
        fi
    done < "$ZEROMOUNT_DATA/.exclusion_list"
fi
