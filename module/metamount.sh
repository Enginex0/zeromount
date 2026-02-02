MODDIR=${0%/*}
LOADER="$MODDIR/bin/zm"
MODULES_DIR="/data/adb/modules"
ZEROMOUNT_DATA="/data/adb/zeromount"

# Source unified logging system
[ -f "$MODDIR/logging.sh" ] && . "$MODDIR/logging.sh"

# Source SUSFS integration
[ -f "$MODDIR/susfs_integration.sh" ] && . "$MODDIR/susfs_integration.sh"

VERBOSE_FLAG="$ZEROMOUNT_DATA/.verbose"
# Note: /apex requires special handling due to APEX mount namespace (planned for future)
TARGET_PARTITIONS="system vendor product system_ext odm oem my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_stock mi_ext cust optics prism"
ACTIVE_MODULES_COUNT=0
BOOT_COUNTER_FILE="$ZEROMOUNT_DATA/boot_counter"

mkdir -p "$ZEROMOUNT_DATA"

# Initialize logging
log_init "metamount"

# 3-strike bootloop protection
if [ -f "$BOOT_COUNTER_FILE" ]; then
    BOOT_COUNT=$(cat "$BOOT_COUNTER_FILE")
else
    BOOT_COUNT=0
fi
BOOT_COUNT=$((BOOT_COUNT + 1))
echo "$BOOT_COUNT" > "$BOOT_COUNTER_FILE"

# Recovery protocol - config backup/restore
BACKUP_DIR="$ZEROMOUNT_DATA/backup"
CONFIG_FILE="$ZEROMOUNT_DATA/config.sh"

backup_config() {
    mkdir -p "$BACKUP_DIR"
    [ -f "$CONFIG_FILE" ] && cp "$CONFIG_FILE" "$BACKUP_DIR/config.sh.bak"
    [ -d "$ZEROMOUNT_DATA/module_paths" ] && cp -r "$ZEROMOUNT_DATA/module_paths" "$BACKUP_DIR/"
}

restore_config() {
    if [ -f "$BACKUP_DIR/config.sh.bak" ]; then
        cp "$BACKUP_DIR/config.sh.bak" "$CONFIG_FILE"
        log_warn "Restored config.sh from backup"
    fi
    if [ -d "$BACKUP_DIR/module_paths" ]; then
        rm -rf "$ZEROMOUNT_DATA/module_paths"
        cp -r "$BACKUP_DIR/module_paths" "$ZEROMOUNT_DATA/"
        log_warn "Restored module_paths from backup"
    fi
}

save_known_good() {
    mkdir -p "$BACKUP_DIR"
    [ -f "$CONFIG_FILE" ] && cp "$CONFIG_FILE" "$BACKUP_DIR/config.sh.good"
    [ -d "$ZEROMOUNT_DATA/module_paths" ] && {
        rm -rf "$BACKUP_DIR/module_paths.good"
        cp -r "$ZEROMOUNT_DATA/module_paths" "$BACKUP_DIR/module_paths.good"
    }
}

if [ "$BOOT_COUNT" -ge 3 ]; then
    log_err "Bootloop detected ($BOOT_COUNT failures). Restoring last known good config."
    restore_config
    touch "$MODDIR/disable"
    echo "0" > "$BOOT_COUNTER_FILE"
    exit 1
fi

# Backup current config before boot
backup_config

log_section "ZeroMount Boot Sequence"
log_info "Kernel: $(uname -r)"
log_info "Boot attempt: $BOOT_COUNT"

# Verbose mode
VERBOSE=false
if [ -f "$VERBOSE_FLAG" ]; then
    VERBOSE=true
    log_info "Verbose mode: ON"
else
    log_info "Verbose mode: OFF"
fi

# Check kernel driver
if [ ! -e "/dev/zeromount" ]; then
    log_err "/dev/zeromount missing - kernel not patched"
    touch "$MODDIR/disable"
    exit 1
fi
log_info "Kernel driver: /dev/zeromount OK"

# Clear existing rules
"$LOADER" clear 2>/dev/null
log_debug "Cleared existing VFS rules"

# Initialize SUSFS
if susfs_init 2>/dev/null; then
    log_info "SUSFS integration initialized"
else
    log_warn "SUSFS not available"
fi

# Conflict detection - scan for multiple modules touching same file
detect_conflicts() {
    local conflict_map="$ZEROMOUNT_DATA/.conflict_map"
    local conflict_count=0
    : > "$conflict_map"

    for mod_path in "$MODULES_DIR"/*; do
        [ -d "$mod_path" ] || continue
        local mod_name="${mod_path##*/}"
        [ "$mod_name" = "zeromount" ] && continue
        [ -f "$mod_path/disable" ] || [ -f "$mod_path/remove" ] && continue

        for partition in $TARGET_PARTITIONS; do
            [ -d "$mod_path/$partition" ] || continue
            find "$mod_path/$partition" -type f 2>/dev/null | while read -r rel; do
                echo "/${rel#$mod_path/}|$mod_name" >> "$conflict_map"
            done
        done
    done

    # Let awk count conflicts - avoids subshell variable clobbering from pipe
    conflict_output=$(sort "$conflict_map" | awk -F'|' '
        { paths[$1] = paths[$1] ? paths[$1] "," $2 : $2 }
        END {
            count = 0
            for (p in paths) {
                n = split(paths[p], mods, ",")
                if (n > 1) {
                    print "CONFLICT: " p " -> " paths[p]
                    count++
                }
            }
            print "COUNT:" count
        }
    ')

    echo "$conflict_output" | grep "^CONFLICT:" | while read -r line; do
        log_warn "$line"
    done

    conflict_count=$(echo "$conflict_output" | grep "^COUNT:" | cut -d: -f2)
    conflict_count=${conflict_count:-0}

    rm -f "$conflict_map"
    [ "$conflict_count" -gt 0 ] && log_warn "Found $conflict_count file conflicts"
}

# getfattr wrapper - multiple fallbacks for Android compatibility
HAS_GETFATTR=0
if /system/bin/getfattr -d /system/bin > /dev/null 2>&1; then
    getfattr() { /system/bin/getfattr "$@"; }
    HAS_GETFATTR=1
elif /system/bin/toybox getfattr -d /system/bin > /dev/null 2>&1; then
    getfattr() { /system/bin/toybox getfattr "$@"; }
    HAS_GETFATTR=1
elif busybox getfattr -d /system/bin > /dev/null 2>&1; then
    getfattr() { busybox getfattr "$@"; }
    HAS_GETFATTR=1
elif command -v getfattr > /dev/null 2>&1; then
    HAS_GETFATTR=1
else
    getfattr() { return 1; }
    log_warn "getfattr not available - overlay xattr detection disabled"
fi

is_whiteout() {
    local path="$1"
    [ -z "$path" ] && return 1

    # Format 1: Character device with major=0, minor=0
    if [ -c "$path" ]; then
        local major minor
        major=$(busybox stat -c '%t' "$path" 2>/dev/null)
        minor=$(busybox stat -c '%T' "$path" 2>/dev/null)
        [ "$major" = "0" ] && [ "$minor" = "0" ] && return 0
    fi

    # Format 2: Zero-size file with xattr (only if getfattr available)
    if [ "$HAS_GETFATTR" = "1" ] && [ -f "$path" ] && [ ! -s "$path" ]; then
        getfattr -n trusted.overlay.whiteout "$path" 2>/dev/null | grep -q "y" && return 0
    fi

    return 1
}

is_opaque_dir() {
    local path="$1"
    [ -z "$path" ] && return 1
    [ ! -d "$path" ] && return 1
    [ "$HAS_GETFATTR" = "0" ] && return 1
    getfattr -n trusted.overlay.opaque "$path" 2>/dev/null | grep -q "y"
}

is_aufs_whiteout() {
    local path="$1"
    [ -z "$path" ] && return 1
    local basename="${path##*/}"
    case "$basename" in
        .wh.*) return 0 ;;
    esac
    return 1
}

get_redirect_path() {
    local path="$1"
    [ -z "$path" ] && return
    [ "$HAS_GETFATTR" = "0" ] && return
    getfattr -n trusted.overlay.redirect "$path" 2>/dev/null | \
        sed -n 's/^trusted\.overlay\.redirect="\(.*\)"$/\1/p'
}


log_section "Conflict Detection"
detect_conflicts

log_section "Module Injection"

TRACKING_DIR="$ZEROMOUNT_DATA/module_paths"
mkdir -p "$TRACKING_DIR" 2>/dev/null

for mod_path in "$MODULES_DIR"/*; do
    [ -d "$mod_path" ] || continue
    mod_name="${mod_path##*/}"
    [ "$mod_name" = "zeromount" ] && continue
    [ -f "$mod_path/skip_mount" ] && continue

    if [ -f "$mod_path/disable" ] || [ -f "$mod_path/remove" ]; then
        log_debug "Skipping $mod_name (disabled/removed)"
        continue
    fi

    MODULE_INJECTED="false"
    TRACKING_FILE="$TRACKING_DIR/$mod_name"
    : > "$TRACKING_FILE"

    for partition in $TARGET_PARTITIONS; do
        if [ -d "$mod_path/$partition" ]; then
            MODULE_INJECTED="true"
            log_info "Processing: $mod_name (/$partition)"
            (
                cd "$mod_path" || exit

                # Scan all overlay-relevant types: files, dirs, symlinks, char devices
                find "$partition" \( -type f -o -type d -o -type l -o -type c \) 2>/dev/null | while read -r relative_path; do
                    real_path="$mod_path/$relative_path"
                    virtual_path="/$relative_path"

                    # Skip partition root
                    [ "$relative_path" = "$partition" ] && continue

                    # Character device (0,0) = overlay whiteout
                    if [ -c "$real_path" ]; then
                        if is_whiteout "$real_path"; then
                            log_info "  Whiteout: $virtual_path"
                            susfs_hide_path "$virtual_path"
                            echo "$virtual_path|whiteout" >> "$TRACKING_FILE"
                            continue
                        fi
                    fi

                    # Regular file - check for overlay markers
                    if [ -f "$real_path" ]; then
                        # Zero-byte whiteout with xattr
                        if is_whiteout "$real_path"; then
                            log_info "  Whiteout (xattr): $virtual_path"
                            susfs_hide_path "$virtual_path"
                            echo "$virtual_path|whiteout" >> "$TRACKING_FILE"
                            continue
                        fi

                        # AUFS-style whiteout (.wh.filename)
                        if is_aufs_whiteout "$real_path"; then
                            target_name=$(basename "$real_path" | sed 's/^\.wh\.//')
                            target_path="$(dirname "$virtual_path")/$target_name"
                            log_info "  AUFS Whiteout: $target_path"
                            susfs_hide_path "$target_path"
                            echo "$target_path|aufs_whiteout" >> "$TRACKING_FILE"
                            continue
                        fi

                        # Redirect xattr (renamed/moved file)
                        redirect=$(get_redirect_path "$real_path")
                        if [ -n "$redirect" ]; then
                            log_info "  Redirect: $redirect -> $real_path"
                            OUTPUT=$("$LOADER" add "$redirect" "$real_path" 2>&1)
                            RET_CODE=$?
                            echo "$redirect|redirect|$virtual_path" >> "$TRACKING_FILE"
                            zm_register_rule_with_susfs "$redirect" "$real_path" 2>/dev/null || true
                            [ $RET_CODE -ne 0 ] && log_err "Failed redirect: $redirect ($OUTPUT)"
                            continue
                        fi

                        # Standard file injection
                        $VERBOSE && log_debug "  Inject: $virtual_path"
                        OUTPUT=$("$LOADER" add "$virtual_path" "$real_path" 2>&1)
                        RET_CODE=$?
                        echo "$virtual_path|file" >> "$TRACKING_FILE"
                        zm_register_rule_with_susfs "$virtual_path" "$real_path" 2>/dev/null || true
                        [ $RET_CODE -ne 0 ] && log_err "Failed: $virtual_path ($OUTPUT)"
                        continue
                    fi

                    # Directory - check for opaque marker
                    if [ -d "$real_path" ]; then
                        if is_opaque_dir "$real_path"; then
                            log_info "  Opaque dir: $virtual_path"
                            susfs_hide_path "$virtual_path"
                            echo "$virtual_path|opaque_dir" >> "$TRACKING_FILE"
                        fi
                        continue
                    fi

                    # Symlink - resolve and redirect
                    if [ -L "$real_path" ]; then
                        link_target=$(busybox readlink -f "$real_path" 2>/dev/null)
                        if [ -e "$link_target" ]; then
                            $VERBOSE && log_debug "  Symlink: $virtual_path -> $link_target"
                            OUTPUT=$("$LOADER" add "$virtual_path" "$link_target" 2>&1)
                            RET_CODE=$?
                            echo "$virtual_path|symlink|$link_target" >> "$TRACKING_FILE"
                            zm_register_rule_with_susfs "$virtual_path" "$link_target" 2>/dev/null || true
                            [ $RET_CODE -ne 0 ] && log_err "Failed symlink: $virtual_path ($OUTPUT)"
                        else
                            log_warn "  Dangling symlink: $virtual_path -> $link_target"
                        fi
                        continue
                    fi
                done
            )
        fi
    done

    if [ "$MODULE_INJECTED" = "true" ]; then
        ACTIVE_MODULES_COUNT=$((ACTIVE_MODULES_COUNT + 1))
        log_info "Registered $mod_name ($(wc -l < "$TRACKING_FILE" 2>/dev/null || echo 0) rules)"
    else
        rm -f "$TRACKING_FILE"
    fi
done

log_section "Finalization"
log_info "Modules processed: $ACTIVE_MODULES_COUNT"

# Update module description with module names
update_module_description() {
    local prop_file="$MODDIR/module.prop"
    local module_names="" module_count=0

    for tracking_file in "$TRACKING_DIR"/*; do
        [ -f "$tracking_file" ] || continue
        local name=$(basename "$tracking_file")
        [ -n "$module_names" ] && module_names="$module_names, "
        module_names="$module_names$name"
        module_count=$((module_count + 1))
    done

    local desc
    if [ "$module_count" -gt 0 ]; then
        local label="Modules"
        [ "$module_count" -eq 1 ] && label="Module"
        desc="GHOST👻 | $module_count $label | $module_names"
    else
        desc="😴 Idle — No Module Mounted
Mountless VFS-level Redirection which Replaces Magic mount & Overlayfs. GHOST👻"
    fi
    grep -v "^description=" "$prop_file" > "${prop_file}.tmp"
    printf 'description=%s\n' "$desc" >> "${prop_file}.tmp"
    mv "${prop_file}.tmp" "$prop_file"
}

update_module_description

# Enable ZeroMount
if "$LOADER" enable 2>/dev/null; then
    log_info "ZeroMount enabled"
    EXIT_CODE=0
else
    log_err "Failed to enable ZeroMount"
    EXIT_CODE=1
fi

# Apply deferred sus_paths (overlays now unmounted)
if [ "$HAS_SUSFS" = "1" ]; then
    log_debug "Applying deferred sus_paths..."
    apply_deferred_sus_paths 2>/dev/null || true

    # Late kstat pass for any deferred entries
    log_debug "Running late kstat pass..."
    late_kstat_pass 2>/dev/null || true

    # Pre-warm cache by listing rules
    log_debug "Pre-warming rule cache..."
    "$LOADER" list >/dev/null 2>&1 || true
fi

# Start monitor
sh "$MODDIR/monitor.sh" "$ACTIVE_MODULES_COUNT" &
log_debug "Monitor started (PID: $!)"

# Notify KernelSU on success
if [ "$EXIT_CODE" = "0" ]; then
    /data/adb/ksud kernel notify-module-mounted
    log_info "KernelSU notified: module mounted"
    echo "0" > "$BOOT_COUNTER_FILE"
    log_info "Boot counter reset"
    save_known_good
    log_info "Config saved as known good"
fi

log_section "Complete"
log_summary "Modules: $ACTIVE_MODULES_COUNT" "Status: $([ $EXIT_CODE -eq 0 ] && echo OK || echo FAILED)"

exit $EXIT_CODE
