#!/system/bin/sh
# ZeroMount Monitor - Watches for module changes and manages VFS rules
# Lightweight design for VFS path redirection architecture (no overlays)

MODDIR=${0%/*}
[ "$MODDIR" = "$0" ] && MODDIR="."
PROP_FILE="$MODDIR/module.prop"
MODULES_DIR="/data/adb/modules"
ZEROMOUNT_DATA="/data/adb/zeromount"
CONFIG_FILE="$ZEROMOUNT_DATA/config.sh"
PID_FILE="$ZEROMOUNT_DATA/.monitor.pid"
TRACKING_DIR="$ZEROMOUNT_DATA/module_paths"

TARGET_PARTITIONS="system vendor product system_ext odm oem mi_ext my_heytap prism optics"

mkdir -p "$TRACKING_DIR" 2>/dev/null

# Source unified logging
[ -f "$MODDIR/logging.sh" ] && . "$MODDIR/logging.sh"
log_init "monitor"

find_zm_binary() {
    for path in "$MODDIR/bin/zm" "$MODDIR/zm-arm64" "$MODDIR/zm"; do
        [ -x "$path" ] && { echo "$path"; return 0; }
    done
    return 1
}

LOADER=$(find_zm_binary)
[ -z "$LOADER" ] && { log_err "zm binary not found"; exit 1; }

# SUSFS integration
[ -f "$MODDIR/susfs_integration.sh" ] && . "$MODDIR/susfs_integration.sh" && susfs_init 2>/dev/null

cleanup() {
    [ -n "$INOTIFY_PID" ] && kill "$INOTIFY_PID" 2>/dev/null
    rm -f "$PID_FILE" "$ZEROMOUNT_DATA/.sync_tmp_"* 2>/dev/null
}
trap cleanup EXIT INT TERM

# Single instance check
echo $$ > "$PID_FILE.$$"
if [ -f "$PID_FILE" ]; then
    old_pid=$(cat "$PID_FILE" 2>/dev/null)
    [ -n "$old_pid" ] && [ "$old_pid" != "$$" ] && kill -0 "$old_pid" 2>/dev/null && { rm -f "$PID_FILE.$$"; exit 0; }
fi
mv "$PID_FILE.$$" "$PID_FILE"

# Process camouflage - hide as kernel worker thread
camouflage_process() {
    local rnd=$(($(date +%s) % 8))
    local name="kworker/u${rnd}:zm"
    echo "$name" > /proc/self/comm 2>/dev/null || true
}
camouflage_process

MODULE_COUNT="${1:-0}"

log_info "Monitor started (PID: $$)"

# Load config
excluded_modules=""
[ -f "$CONFIG_FILE" ] && . "$CONFIG_FILE" 2>/dev/null

is_excluded() { echo "$excluded_modules" | grep -qw "$1"; }

update_status() {
    local desc
    if [ "$MODULE_COUNT" -gt 0 ]; then
        desc="⚡ Active | $MODULE_COUNT modules redirected"
    else
        desc="⚠️ Idle | No modules"
    fi
    sed -i "s/^description=.*/description=$desc/" "$PROP_FILE" 2>/dev/null
}
update_status

count_module_files() {
    local mod_path="$1"
    local count=0
    for p in $TARGET_PARTITIONS; do
        [ -d "$mod_path/$p" ] && count=$((count + $(find "$mod_path/$p" -type f 2>/dev/null | wc -l)))
    done
    echo "$count"
}

register_module() {
    local mod_path="$1"
    local mod_name=$(basename "$mod_path")
    local tracking_file="$TRACKING_DIR/$mod_name"

    : > "$tracking_file"

    for partition in $TARGET_PARTITIONS; do
        [ ! -d "$mod_path/$partition" ] && continue
        find "$mod_path/$partition" -type f 2>/dev/null | while read -r rel; do
            local vpath="/${rel#$mod_path/}"
            local rpath="$mod_path/$rel"
            "$LOADER" add "$vpath" "$rpath" </dev/null 2>/dev/null
            echo "$vpath" >> "$tracking_file"
        done
    done

    local count=$(wc -l < "$tracking_file" 2>/dev/null || echo 0)
    log_info "Registered $mod_name ($count rules)"
}

unregister_module() {
    local mod_name="$1"
    local tracking_file="$TRACKING_DIR/$mod_name"
    [ ! -f "$tracking_file" ] && return

    local count=0
    while IFS= read -r vpath; do
        [ -z "$vpath" ] && continue
        "$LOADER" del "$vpath" </dev/null 2>/dev/null && count=$((count + 1))
    done < "$tracking_file"

    rm -f "$tracking_file"
    log_info "Unregistered $mod_name ($count rules)"
    MODULE_COUNT=$((MODULE_COUNT - 1))
    [ "$MODULE_COUNT" -lt 0 ] && MODULE_COUNT=0
    update_status
}

sync_module() {
    local mod_path="$1"
    local mod_name=$(basename "$mod_path")
    local tracking_file="$TRACKING_DIR/$mod_name"
    [ ! -f "$tracking_file" ] && return

    local tmp="$ZEROMOUNT_DATA/.sync_tmp_$$"
    : > "$tmp"
    for p in $TARGET_PARTITIONS; do
        [ -d "$mod_path/$p" ] && find "$mod_path/$p" -type f 2>/dev/null | sed "s|^$mod_path|/|" >> "$tmp"
    done

    local added=0 removed=0

    while IFS= read -r vpath; do
        [ -z "$vpath" ] && continue
        grep -qxF "$vpath" "$tmp" || { "$LOADER" del "$vpath" </dev/null 2>/dev/null; removed=$((removed + 1)); }
    done < "$tracking_file"

    while IFS= read -r vpath; do
        [ -z "$vpath" ] && continue
        grep -qxF "$vpath" "$tracking_file" || {
            "$LOADER" add "$vpath" "$mod_path$vpath" </dev/null 2>/dev/null
            added=$((added + 1))
        }
    done < "$tmp"

    [ "$added" -gt 0 ] || [ "$removed" -gt 0 ] && { cp "$tmp" "$tracking_file"; log_info "Synced $mod_name (+$added -$removed)"; }
    rm -f "$tmp"
}

handle_module_change() {
    local mod_name="$1"
    local mod_path="$MODULES_DIR/$mod_name"

    [ "$mod_name" = "zeromount" ] && return
    is_excluded "$mod_name" && return

    if [ ! -d "$mod_path" ] || [ -f "$mod_path/disable" ] || [ -f "$mod_path/remove" ]; then
        [ -f "$TRACKING_DIR/$mod_name" ] && unregister_module "$mod_name"
        return
    fi

    if [ -f "$TRACKING_DIR/$mod_name" ]; then
        sync_module "$mod_path"
    else
        local files=$(count_module_files "$mod_path")
        if [ "$files" -gt 0 ]; then
            register_module "$mod_path"
            MODULE_COUNT=$((MODULE_COUNT + 1))
            update_status
        fi
    fi
}

# Polling loop (simple, reliable)
poll_modules() {
    log_info "Polling started (5s interval)"

    while true; do
        sleep 5
        [ -f "$MODDIR/disable" ] || [ -f "$MODDIR/remove" ] && break

        for mod_path in "$MODULES_DIR"/*; do
            [ -d "$mod_path" ] || continue
            handle_module_change "$(basename "$mod_path")"
        done

        # Check for removed modules
        for tracking_file in "$TRACKING_DIR"/*; do
            [ -f "$tracking_file" ] || continue
            local mod_name=$(basename "$tracking_file")
            [ ! -d "$MODULES_DIR/$mod_name" ] && unregister_module "$mod_name"
        done
    done
}

poll_modules &
