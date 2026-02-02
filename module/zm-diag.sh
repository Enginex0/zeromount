#!/system/bin/sh
# ZeroMount Diagnostic CLI
# Usage: zm-diag.sh <command>
# Commands: status, modules, conflicts, rules

ZEROMOUNT_DATA="/data/adb/zeromount"
MODULES_DIR="/data/adb/modules"
MODDIR="${0%/*}"
LOADER="$MODDIR/bin/zm"
TARGET_PARTITIONS="system vendor product system_ext odm oem"

[ ! -x "$LOADER" ] && LOADER="/data/adb/modules/zeromount/bin/zm"

cmd_status() {
    echo "=== ZeroMount Status ==="

    if [ -c "/dev/zeromount" ]; then
        echo "Driver: ✓ /dev/zeromount"
        local ver=$("$LOADER" ver 2>/dev/null)
        [ -n "$ver" ] && echo "Version: $ver"
    else
        echo "Driver: ✗ not loaded"
    fi

    local rule_count=$("$LOADER" list 2>/dev/null | wc -l)
    echo "Rules: $rule_count active"

    if [ -f "/data/adb/ksu/bin/ksu_susfs" ]; then
        echo "SUSFS: ✓ available"
    else
        echo "SUSFS: ✗ not found"
    fi

    local mod_count=$(ls -1 "$ZEROMOUNT_DATA/module_paths" 2>/dev/null | wc -l)
    echo "Modules tracked: $mod_count"

    if ps 2>/dev/null | grep -q "[m]onitor.sh"; then
        echo "Monitor: ✓ running"
    else
        echo "Monitor: ✗ not running"
    fi
}

cmd_modules() {
    echo "=== Active Modules ==="

    for tracking_file in "$ZEROMOUNT_DATA/module_paths"/*; do
        [ -f "$tracking_file" ] || continue
        local mod_name=$(basename "$tracking_file")
        local rule_count=$(wc -l < "$tracking_file" 2>/dev/null || echo 0)
        local mod_path="$MODULES_DIR/$mod_name"

        local status="✓"
        [ -f "$mod_path/disable" ] && status="⏸"
        [ ! -d "$mod_path" ] && status="✗"

        printf "%s %-30s %4d rules\n" "$status" "$mod_name" "$rule_count"
    done
}

cmd_conflicts() {
    echo "=== File Conflicts ==="

    local conflict_map="$ZEROMOUNT_DATA/.conflict_tmp_$$"
    : > "$conflict_map"
    local found=0

    for mod_path in "$MODULES_DIR"/*; do
        [ -d "$mod_path" ] || continue
        local mod_name="${mod_path##*/}"
        [ "$mod_name" = "zeromount" ] && continue
        [ -f "$mod_path/disable" ] && continue

        for partition in $TARGET_PARTITIONS; do
            [ -d "$mod_path/$partition" ] || continue
            find "$mod_path/$partition" -type f 2>/dev/null | while read -r rel; do
                echo "/${rel#$mod_path/}|$mod_name"
            done
        done
    done > "$conflict_map"

    sort "$conflict_map" | awk -F'|' '
        { paths[$1] = paths[$1] ? paths[$1] ", " $2 : $2; count[$1]++ }
        END {
            for (p in paths) {
                if (count[p] > 1) {
                    print p
                    print "  └─ " paths[p]
                    print ""
                }
            }
        }
    '

    local conflict_count=$(sort "$conflict_map" | cut -d'|' -f1 | uniq -d | wc -l)
    rm -f "$conflict_map"

    if [ "$conflict_count" -eq 0 ]; then
        echo "No conflicts detected."
    else
        echo "Total: $conflict_count conflicting paths"
    fi
}

cmd_rules() {
    echo "=== Active VFS Rules ==="
    "$LOADER" list 2>/dev/null || echo "Failed to list rules"
}

cmd_help() {
    echo "ZeroMount Diagnostic CLI"
    echo ""
    echo "Usage: $0 <command>"
    echo ""
    echo "Commands:"
    echo "  status     Show ZeroMount status"
    echo "  modules    List tracked modules"
    echo "  conflicts  Detect file conflicts between modules"
    echo "  rules      List active VFS redirection rules"
    echo "  help       Show this help"
}

case "${1:-help}" in
    status)    cmd_status ;;
    modules)   cmd_modules ;;
    conflicts) cmd_conflicts ;;
    rules)     cmd_rules ;;
    *)         cmd_help ;;
esac
