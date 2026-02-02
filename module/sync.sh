#!/system/bin/sh
# ZeroMount Sync Script - Can be called by other modules to force sync
# Usage: sh /data/adb/modules/zeromount/sync.sh [module_name]
# If module_name provided, sync only that module
# If no argument, sync all tracked modules
#
# This script is self-contained and does not depend on monitor.sh running.
# Safe to call multiple times - idempotent operation.

ZEROMOUNT_DATA="/data/adb/zeromount"
MODULES_DIR="/data/adb/modules"
TRACKING_DIR="$ZEROMOUNT_DATA/module_paths"
LOG_FILE="$ZEROMOUNT_DATA/zeromount.log"
TARGET_PARTITIONS="system vendor product system_ext odm oem mi_ext my_heytap prism optics oem_dlkm system_dlkm vendor_dlkm"

mkdir -p "$ZEROMOUNT_DATA" 2>/dev/null
mkdir -p "$TRACKING_DIR" 2>/dev/null

log_err() {
    echo "[$(date '+%H:%M:%S')] [SYNC] [ERROR] $*" >> "$LOG_FILE"
}

log_info() {
    echo "[$(date '+%H:%M:%S')] [SYNC] [INFO] $*" >> "$LOG_FILE"
}

log_debug() {
    echo "[$(date '+%H:%M:%S')] [SYNC] [DEBUG] $*" >> "$LOG_FILE"
}

find_zm_binary() {
    local possible_paths="
        /data/adb/modules/zeromount/bin/zm
        /data/adb/modules/zeromount/zm-arm64
        /data/adb/modules/zeromount/zm
    "
    for path in $possible_paths; do
        if [ -x "$path" ]; then
            echo "$path"
            return 0
        fi
    done
    return 1
}

sync_single_module() {
    local mod_name="$1"
    local mod_path="$MODULES_DIR/$mod_name"
    local tracking_file="$TRACKING_DIR/$mod_name"

    if [ ! -d "$mod_path" ] || [ -f "$mod_path/disable" ] || [ -f "$mod_path/remove" ]; then
        if [ -f "$tracking_file" ]; then
            local removed=0
            while IFS='|' read -r virtual_path _type _extra; do
                [ -z "$virtual_path" ] && continue
                "$LOADER" del "$virtual_path" < /dev/null 2>/dev/null && removed=$((removed + 1)) || log_err "Failed to remove: $virtual_path"
            done < "$tracking_file"
            rm -f "$tracking_file"
            log_info "Cleaned $mod_name: $removed rules removed"
        fi
        return 0
    fi

    [ ! -f "$tracking_file" ] && return 0

    local current_files="$ZEROMOUNT_DATA/.sync_tmp_$$"
    : > "$current_files"
    for partition in $TARGET_PARTITIONS; do
        [ -d "$mod_path/$partition" ] && \
            (cd "$mod_path" && find "$partition" -type f -o -type c 2>/dev/null) | sed 's|^|/|' >> "$current_files"
    done

    local removed=0 added=0

    while IFS='|' read -r tracked_path _type _extra; do
        [ -z "$tracked_path" ] && continue
        if ! grep -qxF "$tracked_path" "$current_files"; then
            "$LOADER" del "$tracked_path" < /dev/null 2>/dev/null && removed=$((removed + 1)) || log_err "Failed to remove: $tracked_path"
        fi
    done < "$tracking_file"

    while IFS= read -r current_path; do
        [ -z "$current_path" ] && continue
        if ! grep -q "^${current_path}|" "$tracking_file"; then
            local real_path="$mod_path$current_path"
            if [ -c "$real_path" ]; then
                "$LOADER" add "$current_path" "/nonexistent" < /dev/null 2>/dev/null && added=$((added + 1)) || log_err "Failed to add: $current_path"
            elif [ -f "$real_path" ]; then
                "$LOADER" add "$current_path" "$real_path" < /dev/null 2>/dev/null && added=$((added + 1)) || log_err "Failed to add: $current_path"
            fi
        fi
    done < "$current_files"

    if [ "$removed" -gt 0 ] || [ "$added" -gt 0 ]; then
        : > "$tracking_file.new"
        while IFS= read -r path; do
            old_entry=$(grep "^${path}|" "$tracking_file" 2>/dev/null | head -1)
            if [ -n "$old_entry" ]; then
                echo "$old_entry" >> "$tracking_file.new"
            else
                echo "${path}|file" >> "$tracking_file.new"
            fi
        done < "$current_files"
        mv "$tracking_file.new" "$tracking_file"
        log_info "Synced $mod_name: +$added -$removed"
    fi

    rm -f "$current_files"
}

sync_all_modules() {
    local synced=0
    for tracking_file in "$TRACKING_DIR"/*; do
        [ ! -f "$tracking_file" ] && continue
        sync_single_module "$(basename "$tracking_file")"
        synced=$((synced + 1))
    done
    log_info "Synced $synced modules"
}

LOADER=$(find_zm_binary)
[ -z "$LOADER" ] && { log_err "zm binary not found"; exit 1; }
[ ! -c "/dev/zeromount" ] && { log_err "ZeroMount driver not available"; exit 1; }

if [ -n "$1" ]; then
    sync_single_module "$1"
else
    sync_all_modules
fi
exit 0
