#!/system/bin/sh
# ZeroMount + SUSFS Tight Coupling Integration
#
# Provides automatic SUSFS configuration when ZeroMount rules are added.
# Logs: /data/adb/zeromount/logs/susfs/susfs.log

# Logging fallback
if ! type log_debug >/dev/null 2>&1; then
    _SUSFS_LOG_FILE="${ZEROMOUNT_DATA:-/data/adb/zeromount}/logs/susfs/susfs.log"
    mkdir -p "$(dirname "$_SUSFS_LOG_FILE")" 2>/dev/null

    log_debug() { echo "[$(date '+%H:%M:%S')] [DEBUG] [SUSFS] $*" >> "$_SUSFS_LOG_FILE" 2>/dev/null; }
    log_info() { echo "[$(date '+%H:%M:%S')] [INFO ] [SUSFS] $*" >> "$_SUSFS_LOG_FILE" 2>/dev/null; }
    log_warn() { echo "[$(date '+%H:%M:%S')] [WARN ] [SUSFS] $*" >> "$_SUSFS_LOG_FILE" 2>/dev/null; }
    log_err() { echo "[$(date '+%H:%M:%S')] [ERROR] [SUSFS] $*" >> "$_SUSFS_LOG_FILE" 2>/dev/null; }
    log_trace() { echo "[$(date '+%H:%M:%S')] [TRACE] [SUSFS] $*" >> "$_SUSFS_LOG_FILE" 2>/dev/null; }
    log_func_enter() { local f="$1"; shift; log_debug ">>> ENTER: $f($*)"; }
    log_func_exit() { log_debug "<<< EXIT: $1 (result=$2)"; }
    log_susfs_cmd() { log_debug "Executing: ksu_susfs $*"; }
    log_susfs_result() { [ "$1" -eq 0 ] && log_debug "OK: $2 '$3'" || log_warn "FAIL: $2 '$3' (rc=$1)"; }
fi

# SUSFS configuration
SUSFS_CONFIG_DIR=""
SUSFS_BIN=""
HAS_SUSFS=0
HAS_SUS_PATH=0
HAS_SUS_PATH_LOOP=0
HAS_SUS_MOUNT=0
HAS_SUS_KSTAT=0
HAS_SUS_KSTAT_REDIRECT=0
HAS_SUS_MAPS=0
HAS_OPEN_REDIRECT=0

# Metadata cache directory
METADATA_CACHE_DIR="${ZEROMOUNT_DATA:-/data/adb/zeromount}/metadata_cache"

# Track hidden mounts
SUSFS_HIDDEN_MOUNTS=""

# Statistics
SUSFS_STATS_PATH=0
SUSFS_STATS_KSTAT=0
SUSFS_STATS_MOUNT=0
SUSFS_STATS_MAPS=0
SUSFS_STATS_OPEN_REDIRECT=0
SUSFS_STATS_ERRORS=0

# Deferred sus_path list - applied AFTER overlays unmounted to avoid EINVAL
SUSFS_DEFERRED_PATHS=""

susfs_init() {
    log_func_enter "susfs_init"
    log_info "Initializing SUSFS integration..."

    local bin_paths="/data/adb/ksu/bin/ksu_susfs /data/adb/ksu/bin/susfs"
    log_debug "Searching for SUSFS binary in: $bin_paths"

    for bin_path in $bin_paths; do
        log_trace "Checking: $bin_path"
        if [ -x "$bin_path" ]; then
            SUSFS_BIN="$bin_path"
            log_debug "Found SUSFS binary: $bin_path"
            break
        fi
    done

    if [ -z "$SUSFS_BIN" ]; then
        log_trace "Trying command lookup..."
        SUSFS_BIN=$(command -v ksu_susfs 2>/dev/null)
        [ -z "$SUSFS_BIN" ] && SUSFS_BIN=$(command -v susfs 2>/dev/null)
    fi

    if [ -z "$SUSFS_BIN" ] || [ ! -x "$SUSFS_BIN" ]; then
        HAS_SUSFS=0
        log_warn "SUSFS binary not found - running in ZeroMount-only mode"
        log_func_exit "susfs_init" "1" "no binary"
        return 1
    fi

    HAS_SUSFS=1
    log_info "Found SUSFS binary: $SUSFS_BIN"

    log_debug "Detecting SUSFS capabilities..."
    local help_output
    help_output=$("$SUSFS_BIN" 2>&1)
    log_trace "SUSFS help output length: ${#help_output} chars"

    echo "$help_output" | grep -q "add_sus_path[^_]" && { HAS_SUS_PATH=1; log_debug "Capability: add_sus_path"; }
    echo "$help_output" | grep -q "add_sus_path_loop" && { HAS_SUS_PATH_LOOP=1; log_debug "Capability: add_sus_path_loop"; }
    echo "$help_output" | grep -q "add_sus_mount" && { HAS_SUS_MOUNT=1; log_debug "Capability: add_sus_mount"; }
    echo "$help_output" | grep -q "add_sus_kstat_statically" && { HAS_SUS_KSTAT=1; log_debug "Capability: add_sus_kstat_statically"; }
    echo "$help_output" | grep -q "add_sus_kstat_redirect" && { HAS_SUS_KSTAT_REDIRECT=1; log_debug "Capability: add_sus_kstat_redirect"; }
    echo "$help_output" | grep -q "add_sus_map" && { HAS_SUS_MAPS=1; log_debug "Capability: add_sus_map"; }
    echo "$help_output" | grep -q "add_open_redirect" && { HAS_OPEN_REDIRECT=1; log_debug "Capability: add_open_redirect"; }

    export SUSFS_BIN HAS_SUSFS HAS_SUS_PATH HAS_SUS_PATH_LOOP HAS_SUS_MOUNT HAS_SUS_KSTAT HAS_SUS_KSTAT_REDIRECT HAS_SUS_MAPS HAS_OPEN_REDIRECT

    log_info "Capabilities: path=$HAS_SUS_PATH loop=$HAS_SUS_PATH_LOOP mount=$HAS_SUS_MOUNT kstat=$HAS_SUS_KSTAT redirect=$HAS_SUS_KSTAT_REDIRECT maps=$HAS_SUS_MAPS open_redirect=$HAS_OPEN_REDIRECT"

    log_debug "Searching for SUSFS config directory..."
    for config_dir in "/data/adb/susfs4ksu" "/data/adb/ksu/susfs4ksu" "/data/adb/susfs"; do
        if [ -d "$config_dir" ]; then
            SUSFS_CONFIG_DIR="$config_dir"
            log_debug "Found config directory: $config_dir"
            break
        fi
    done
    [ -n "$SUSFS_CONFIG_DIR" ] && log_info "Config directory: $SUSFS_CONFIG_DIR" || log_debug "No config directory found"

    log_debug "Creating metadata cache directory: $METADATA_CACHE_DIR"
    mkdir -p "$METADATA_CACHE_DIR" 2>/dev/null || log_warn "Failed to create metadata cache directory"

    log_func_exit "susfs_init" "0" "initialized"
    return 0
}

susfs_classify_path() {
    local vpath="$1"
    log_func_enter "susfs_classify_path" "$vpath"

    local actions="sus_path"

    case "$vpath" in
        *.so|*.jar|*.dex|*.oat|*.vdex|*.art|*.odex)
            actions="$actions,sus_maps,sus_kstat"
            log_debug "Classified as LIBRARY: $vpath" ;;
        /system/bin/*|/system/xbin/*|/vendor/bin/*|/product/bin/*)
            actions="$actions,sus_kstat"
            log_debug "Classified as BINARY: $vpath" ;;
        /system/fonts/*|/system/media/*|/product/fonts/*|/product/media/*)
            actions="sus_kstat,sus_maps"
            log_debug "Classified as MEDIA/FONT: $vpath" ;;
        /system/app/*|/system/priv-app/*|/product/app/*|/product/priv-app/*|/vendor/app/*)
            actions="$actions,sus_kstat,sus_mount_check"
            log_debug "Classified as APP: $vpath" ;;
        /system/framework/*|/system_ext/framework/*|/product/framework/*)
            actions="$actions,sus_maps,sus_kstat"
            log_debug "Classified as FRAMEWORK: $vpath" ;;
        *.xml|*.prop|*.conf|*.rc)
            actions="$actions,sus_kstat"
            log_debug "Classified as CONFIG: $vpath" ;;
        /data/adb/*)
            actions="sus_path_loop,sus_kstat"
            log_debug "Classified as MODULE_PATH: $vpath" ;;
        *)
            actions="$actions,sus_kstat"
            log_debug "Classified as DEFAULT: $vpath" ;;
    esac

    log_func_exit "susfs_classify_path" "$actions"
    echo "$actions"
}

# Capture original file metadata (EARLY - before overlays)
susfs_capture_metadata() {
    local vpath="$1"
    log_func_enter "susfs_capture_metadata" "$vpath"

    local cache_key
    cache_key=$(echo "$vpath" | md5sum 2>/dev/null | cut -d' ' -f1)
    [ -z "$cache_key" ] && cache_key=$(echo "$vpath" | cksum | cut -d' ' -f1)

    local cache_file="$METADATA_CACHE_DIR/$cache_key"
    log_trace "Cache file: $cache_file"

    if [ -e "$vpath" ]; then
        log_debug "Capturing metadata for existing file: $vpath"
        local stat_data
        stat_data=$(stat -c '%i|%d|%h|%s|%X|%Y|%Z|%b|%B' "$vpath" 2>/dev/null)

        if [ -n "$stat_data" ]; then
            local fstype
            fstype=$(awk -v path="$vpath" '$2 == path || index(path, $2) == 1 {print $3; exit}' /proc/mounts 2>/dev/null)
            [ -z "$fstype" ] && fstype="ext4"
            echo "${stat_data}|${fstype}" > "$cache_file"
            log_debug "Captured: ino=$(echo "$stat_data" | cut -d'|' -f1) fstype=$fstype"
            log_func_exit "susfs_capture_metadata" "0" "captured"
            return 0
        else
            log_warn "stat failed for $vpath"
            log_func_exit "susfs_capture_metadata" "1" "stat failed"
            return 1
        fi
    else
        echo "NEW|$vpath" > "$cache_file"
        log_debug "Marked as NEW file (will synthesize metadata): $vpath"
        log_func_exit "susfs_capture_metadata" "0" "marked new"
        return 0
    fi
}

susfs_get_cached_metadata() {
    local vpath="$1"
    log_func_enter "susfs_get_cached_metadata" "$vpath"

    local cache_key
    cache_key=$(echo "$vpath" | md5sum 2>/dev/null | cut -d' ' -f1)
    [ -z "$cache_key" ] && cache_key=$(echo "$vpath" | cksum | cut -d' ' -f1)

    local cache_file="$METADATA_CACHE_DIR/$cache_key"

    if [ -f "$cache_file" ]; then
        local metadata
        metadata=$(cat "$cache_file")
        log_debug "Cache hit: $vpath -> $(printf '%.50s' "$metadata")..."
        log_func_exit "susfs_get_cached_metadata" "found"
        echo "$metadata"
    else
        log_debug "Cache miss: $vpath"
        log_func_exit "susfs_get_cached_metadata" "not found"
        echo ""
    fi
}

# Apply SUSFS sus_path hiding
# Args: vpath use_loop [defer]
#   defer=1 (default): collect path for post-unmount application
#   defer=0: apply immediately (call from apply_deferred_sus_paths)
susfs_apply_path() {
    local vpath="$1"
    local use_loop="${2:-0}"
    local defer="${3:-1}"
    log_func_enter "susfs_apply_path" "$vpath" "loop=$use_loop" "defer=$defer"

    [ "$HAS_SUSFS" != "1" ] && { log_debug "SUSFS not available"; log_func_exit "susfs_apply_path" "skip"; return 0; }
    [ "$HAS_SUS_PATH" != "1" ] && { log_debug "sus_path not supported"; log_func_exit "susfs_apply_path" "skip"; return 0; }

    # Check if NEW file (not in stock system)
    local cache_key cache_file
    cache_key=$(echo "$vpath" | md5sum 2>/dev/null | cut -d' ' -f1)
    [ -z "$cache_key" ] && cache_key=$(echo "$vpath" | cksum | cut -d' ' -f1)
    cache_file="$METADATA_CACHE_DIR/$cache_key"

    if [ -f "$cache_file" ] && grep -q "^NEW|" "$cache_file" 2>/dev/null; then
        log_debug "Path is NEW file (not in stock), sus_path not needed: $vpath"
        log_func_exit "susfs_apply_path" "skip" "new file"
        return 0
    fi

    # Skip zero-byte files (whiteouts)
    if [ -f "$vpath" ] && [ ! -s "$vpath" ]; then
        log_debug "Path is zero-byte whiteout, sus_path not needed: $vpath"
        log_func_exit "susfs_apply_path" "skip" "whiteout"
        return 0
    fi

    # Defer to post-unmount phase (overlays still mounted causes EINVAL)
    if [ "$defer" = "1" ]; then
        SUSFS_DEFERRED_PATHS="${SUSFS_DEFERRED_PATHS}${vpath}|${use_loop}
"
        log_debug "Deferred sus_path: $vpath (loop=$use_loop)"
        log_func_exit "susfs_apply_path" "0" "deferred"
        return 0
    fi

    local cmd result rc

    if [ "$use_loop" = "1" ] && [ "$HAS_SUS_PATH_LOOP" = "1" ]; then
        cmd="add_sus_path_loop"
        log_susfs_cmd "$cmd" "$vpath"
        result=$("$SUSFS_BIN" "$cmd" "$vpath" 2>&1)
        rc=$?
        log_susfs_result "$rc" "$cmd" "$vpath"

        if [ $rc -eq 0 ]; then
            SUSFS_STATS_PATH=$((SUSFS_STATS_PATH + 1))
            susfs_update_config "sus_path_loop.txt" "$vpath"
            log_func_exit "susfs_apply_path" "0" "loop applied"
            return 0
        else
            log_err "sus_path_loop failed for $vpath: $result"
            SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
        fi
    fi

    cmd="add_sus_path"
    log_susfs_cmd "$cmd" "$vpath"
    result=$("$SUSFS_BIN" "$cmd" "$vpath" 2>&1)
    rc=$?
    log_susfs_result "$rc" "$cmd" "$vpath"

    if [ $rc -eq 0 ]; then
        SUSFS_STATS_PATH=$((SUSFS_STATS_PATH + 1))
        susfs_update_config "sus_path.txt" "$vpath"
        log_func_exit "susfs_apply_path" "0" "path applied"
        return 0
    else
        log_err "sus_path failed for $vpath: $result"
        SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
    fi

    log_func_exit "susfs_apply_path" "1" "failed"
    return 1
}

susfs_hide_path() {
    local path="$1"
    log_func_enter "susfs_hide_path" "$path"

    [ -z "$path" ] && { log_warn "susfs_hide_path: empty path"; return 1; }

    if [ "$HAS_SUSFS" != "1" ] || [ -z "$SUSFS_BIN" ]; then
        log_debug "SUSFS not available, skipping hide"
        log_func_exit "susfs_hide_path" "0" "no susfs"
        return 0
    fi

    if [ "$HAS_SUS_PATH" != "1" ]; then
        log_debug "add_sus_path not supported"
        log_func_exit "susfs_hide_path" "0" "not supported"
        return 0
    fi

    log_susfs_cmd "add_sus_path" "$path"
    local result rc
    result=$("$SUSFS_BIN" add_sus_path "$path" 2>&1)
    rc=$?
    log_susfs_result "$rc" "add_sus_path" "$path"

    if [ $rc -eq 0 ]; then
        SUSFS_STATS_PATH=$((SUSFS_STATS_PATH + 1))
        log_info "Hidden path: $path"
        log_func_exit "susfs_hide_path" "0"
        return 0
    else
        log_warn "Failed to hide path: $path ($result)"
        SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
        log_func_exit "susfs_hide_path" "1"
        return 1
    fi
}

# Apply all deferred sus_path entries (call AFTER overlays unmounted)
apply_deferred_sus_paths() {
    log_func_enter "apply_deferred_sus_paths"

    if [ -z "$SUSFS_DEFERRED_PATHS" ]; then
        log_debug "No deferred sus_paths to apply"
        log_func_exit "apply_deferred_sus_paths" "0" "empty"
        return 0
    fi

    [ "$HAS_SUS_PATH" != "1" ] && { SUSFS_DEFERRED_PATHS=""; log_func_exit "apply_deferred_sus_paths" "0" "not supported"; return 0; }

    log_info "Applying deferred sus_paths (overlays unmounted)..."
    local count=0
    local failed=0

    # Here-doc avoids subshell from pipe - counters persist
    while IFS='|' read -r vpath use_loop; do
        [ -z "$vpath" ] && continue

        if susfs_apply_path "$vpath" "$use_loop" 0; then
            count=$((count + 1))
        else
            failed=$((failed + 1))
        fi
    done <<EOF
$SUSFS_DEFERRED_PATHS
EOF

    log_info "Deferred sus_paths applied: $count success, $failed failed"
    SUSFS_DEFERRED_PATHS=""
    log_func_exit "apply_deferred_sus_paths" "0"
    return 0
}

susfs_apply_maps() {
    local vpath="$1"
    log_func_enter "susfs_apply_maps" "$vpath"

    [ "$HAS_SUSFS" != "1" ] && { log_debug "SUSFS not available"; log_func_exit "susfs_apply_maps" "skip"; return 0; }
    [ "$HAS_SUS_MAPS" != "1" ] && { log_debug "sus_map not supported"; log_func_exit "susfs_apply_maps" "skip"; return 0; }

    # Check if NEW file
    local cache_key cache_file
    cache_key=$(echo "$vpath" | md5sum 2>/dev/null | cut -d' ' -f1)
    [ -z "$cache_key" ] && cache_key=$(echo "$vpath" | cksum | cut -d' ' -f1)
    cache_file="$METADATA_CACHE_DIR/$cache_key"

    if [ -f "$cache_file" ] && grep -q "^NEW|" "$cache_file" 2>/dev/null; then
        log_debug "Path is NEW file (not in stock), sus_maps not needed: $vpath"
        log_func_exit "susfs_apply_maps" "skip" "new file"
        return 0
    fi

    log_susfs_cmd "add_sus_map" "$vpath"
    local result rc
    result=$("$SUSFS_BIN" add_sus_map "$vpath" 2>&1)
    rc=$?
    log_susfs_result "$rc" "add_sus_map" "$vpath"

    if [ $rc -eq 0 ]; then
        SUSFS_STATS_MAPS=$((SUSFS_STATS_MAPS + 1))
        susfs_update_config "sus_maps.txt" "$vpath"
        log_func_exit "susfs_apply_maps" "0"
        return 0
    else
        log_err "sus_map failed for $vpath: $result"
        SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
        log_func_exit "susfs_apply_maps" "1"
        return 1
    fi
}

# Apply SUSFS kstat spoofing
# Args: vpath metadata [rpath]
susfs_apply_kstat() {
    local vpath="$1"
    local metadata="$2"
    local rpath="$3"
    log_func_enter "susfs_apply_kstat" "$vpath" "metadata_len=${#metadata}" "rpath=$rpath"

    [ "$HAS_SUSFS" != "1" ] && { log_debug "SUSFS not available"; log_func_exit "susfs_apply_kstat" "skip"; return 0; }
    [ "$HAS_SUS_KSTAT" != "1" ] && { log_debug "sus_kstat not supported"; log_func_exit "susfs_apply_kstat" "skip"; return 0; }

    local ino dev nlink size atime mtime ctime blocks blksize fstype

    if [ -z "$metadata" ]; then
        log_debug "No cached metadata, deriving from parent directory"
        local parent
        parent=$(dirname "$vpath")
        local parent_stat
        parent_stat=$(stat -c '%i|%d|%h|%s|%X|%Y|%Z|%b|%B' "$parent" 2>/dev/null)
        if [ -z "$parent_stat" ]; then
            log_warn "Cannot stat parent $parent for $vpath"
            log_func_exit "susfs_apply_kstat" "1" "no parent"
            return 1
        fi
        IFS='|' read -r _ dev _ _ atime mtime ctime _ blksize <<EOF
$parent_stat
EOF
        ino=$(($(date +%s%N 2>/dev/null || date +%s) % 2147483647))
        nlink=1
        if [ -n "$rpath" ] && [ -f "$rpath" ]; then
            local real_stat=$(stat -c '%s|%b' "$rpath" 2>/dev/null)
            size=$(echo "$real_stat" | cut -d'|' -f1)
            blocks=$(echo "$real_stat" | cut -d'|' -f2)
        else
            size=0
            blocks=0
        fi
        [ -z "$blksize" ] && blksize=4096
        log_info "Derived kstat: dev=$dev from parent $parent"
    elif [ "${metadata%%|*}" = "NEW" ]; then
        log_debug "Synthesizing metadata for NEW file"
        local parent
        parent=$(dirname "$vpath")
        local parent_stat
        parent_stat=$(stat -c '%d|%X|%Y|%Z' "$parent" 2>/dev/null)

        if [ -n "$parent_stat" ]; then
            dev=$(echo "$parent_stat" | cut -d'|' -f1)
            atime=$(echo "$parent_stat" | cut -d'|' -f2)
            mtime=$(echo "$parent_stat" | cut -d'|' -f3)
            ctime=$(echo "$parent_stat" | cut -d'|' -f4)
            ino=$(( ($(date +%s) + $$) % 2147483647 ))
            nlink=1
            if [ -n "$rpath" ] && [ -f "$rpath" ]; then
                local rpath_stat=$(stat -c '%s|%b|%B' "$rpath" 2>/dev/null)
                if [ -n "$rpath_stat" ]; then
                    size=$(echo "$rpath_stat" | cut -d'|' -f1)
                    blocks=$(echo "$rpath_stat" | cut -d'|' -f2)
                    blksize=$(echo "$rpath_stat" | cut -d'|' -f3)
                else
                    size=0; blocks=0; blksize=4096
                fi
            else
                size=0; blocks=0; blksize=4096
            fi
            log_debug "Synthesized: ino=$ino dev=$dev size=$size times from parent"
        else
            log_warn "Cannot get parent stats for $parent"
            log_func_exit "susfs_apply_kstat" "1" "no parent stats"
            return 1
        fi
    else
        log_trace "Parsing cached metadata: $metadata"
        IFS='|' read -r ino dev nlink size atime mtime ctime blocks blksize fstype <<EOF
$metadata
EOF
        log_debug "Parsed: ino=$ino dev=$dev nlink=$nlink size=$size"
    fi

    # CRITICAL: Always derive dev from parent directory - cached metadata may be wrong
    local parent_dev
    parent_dev=$(stat -c '%d' "$(dirname "$vpath")" 2>/dev/null)
    if [ -n "$parent_dev" ] && [ "$parent_dev" != "$dev" ]; then
        log_info "Device ID override: $dev -> $parent_dev (from parent directory)"
        dev="$parent_dev"
    fi

    local result rc cmd
    if [ -n "$rpath" ] && [ "$HAS_SUS_KSTAT_REDIRECT" = "1" ]; then
        local real_stat
        real_stat=$(stat -c '%s|%b|%B' "$rpath" 2>/dev/null)
        if [ -n "$real_stat" ]; then
            size=$(echo "$real_stat" | cut -d'|' -f1)
            blocks=$(echo "$real_stat" | cut -d'|' -f2)
            blksize=$(echo "$real_stat" | cut -d'|' -f3)
            log_info "Kstat size override: $size (from $rpath)"
        else
            log_info "Stat failed for $rpath - using kstat_statically fallback"
            size=0; blocks=0
        fi
        cmd="add_sus_kstat_redirect"
        log_susfs_cmd "$cmd" "$vpath $rpath ino=$ino dev=$dev"
        result=$("$SUSFS_BIN" "$cmd" "$vpath" "$rpath" \
            "$ino" "$dev" "$nlink" "$size" \
            "$atime" 0 "$mtime" 0 "$ctime" 0 \
            "$blocks" "$blksize" 2>&1)
        rc=$?
    else
        log_info "Redirect unavailable for $vpath - falling back to kstat_statically"
        cmd="add_sus_kstat_statically"
        log_susfs_cmd "$cmd" "$vpath ino=$ino dev=$dev"
        result=$("$SUSFS_BIN" "$cmd" "$vpath" \
            "$ino" "$dev" "$nlink" "$size" \
            "$atime" 0 "$mtime" 0 "$ctime" 0 \
            "$blocks" "$blksize" 2>&1)
        rc=$?
    fi
    log_susfs_result "$rc" "$cmd" "$vpath"

    if [ $rc -eq 0 ]; then
        SUSFS_STATS_KSTAT=$((SUSFS_STATS_KSTAT + 1))
        log_func_exit "susfs_apply_kstat" "0"
        return 0
    else
        log_err "$cmd failed for $vpath: $result"
        SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
        log_func_exit "susfs_apply_kstat" "1"
        return 1
    fi
}

# Apply open_redirect + kstat for font files
apply_font_redirect() {
    local vpath="$1"
    local rpath="$2"
    log_func_enter "apply_font_redirect" "$vpath" "$rpath"

    [ "$HAS_SUSFS" != "1" ] && { log_debug "SUSFS not available"; log_func_exit "apply_font_redirect" "skip"; return 1; }
    [ -z "$vpath" ] || [ -z "$rpath" ] && { log_err "apply_font_redirect: missing vpath or rpath"; return 1; }

    local rpath_stat_cached
    rpath_stat_cached=$(stat -c '%s|%b|%B' "$rpath" 2>/dev/null)
    [ -z "$rpath_stat_cached" ] && { log_err "apply_font_redirect: rpath does not exist: $rpath"; return 1; }

    local cached_size cached_blocks cached_blksize
    cached_size=$(echo "$rpath_stat_cached" | cut -d'|' -f1)
    cached_blocks=$(echo "$rpath_stat_cached" | cut -d'|' -f2)
    cached_blksize=$(echo "$rpath_stat_cached" | cut -d'|' -f3)

    local result rc
    local open_redirect_applied=0

    # Set SELinux context
    if [ -e "$vpath" ]; then
        busybox chcon --reference="$vpath" "$rpath" 2>/dev/null || log_warn "Failed to copy SELinux context for $rpath"
    else
        busybox chcon u:object_r:system_file:s0 "$rpath" 2>/dev/null || log_warn "Failed to set SELinux context for $rpath"
    fi

    # Apply open_redirect
    if [ "$HAS_OPEN_REDIRECT" = "1" ]; then
        log_susfs_cmd "add_open_redirect" "$vpath $rpath"
        result=$("$SUSFS_BIN" add_open_redirect "$vpath" "$rpath" 2>&1)
        rc=$?
        log_susfs_result "$rc" "add_open_redirect" "$vpath"

        if [ $rc -eq 0 ]; then
            SUSFS_STATS_OPEN_REDIRECT=$((SUSFS_STATS_OPEN_REDIRECT + 1))
            susfs_update_config "open_redirect.txt" "$vpath $rpath"
            log_debug "open_redirect applied: $vpath -> $rpath"
            open_redirect_applied=1
            "$SUSFS_BIN" add_sus_path "$rpath" 2>/dev/null
        else
            log_err "add_open_redirect failed for $vpath: $result"
            SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
            log_func_exit "apply_font_redirect" "1" "open_redirect failed"
            return 1
        fi
    else
        log_warn "add_open_redirect not supported, font redirect may be incomplete"
    fi

    [ "$HAS_SUS_KSTAT_REDIRECT" != "1" ] && { log_warn "add_sus_kstat_redirect not supported"; log_func_exit "apply_font_redirect" "0" "partial"; return 0; }

    local ino=0 dev="" nlink=1 size=0 atime=0 mtime=0 ctime=0 blocks=0 blksize=4096
    size="$cached_size"
    blocks="$cached_blocks"
    blksize="$cached_blksize"

    if [ -e "$vpath" ]; then
        local vpath_stat
        vpath_stat=$(stat -c '%i|%d|%h|%X|%Y|%Z' "$vpath" 2>/dev/null)
        if [ -n "$vpath_stat" ]; then
            ino=$(echo "$vpath_stat" | cut -d'|' -f1)
            dev=$(echo "$vpath_stat" | cut -d'|' -f2)
            nlink=$(echo "$vpath_stat" | cut -d'|' -f3)
            atime=$(echo "$vpath_stat" | cut -d'|' -f4)
            mtime=$(echo "$vpath_stat" | cut -d'|' -f5)
            ctime=$(echo "$vpath_stat" | cut -d'|' -f6)
            log_debug "Using original file metadata: ino=$ino dev=$dev"
        fi
    fi

    if [ -z "$dev" ]; then
        local parent parent_stat
        parent=$(dirname "$vpath")
        parent_stat=$(stat -c '%d|%X|%Y|%Z' "$parent" 2>/dev/null)
        if [ -n "$parent_stat" ]; then
            dev=$(echo "$parent_stat" | cut -d'|' -f1)
            atime=$(echo "$parent_stat" | cut -d'|' -f2)
            mtime=$(echo "$parent_stat" | cut -d'|' -f3)
            ctime=$(echo "$parent_stat" | cut -d'|' -f4)
            ino=$(( ($(date +%s) + $$) % 2147483647 ))
            nlink=1
            log_debug "Using parent-derived metadata: ino=$ino dev=$dev"
        else
            log_err "Cannot derive metadata from parent $parent"
            log_func_exit "apply_font_redirect" "1" "no metadata"
            return 1
        fi
    fi

    log_susfs_cmd "add_sus_kstat_redirect" "$vpath $rpath ino=$ino dev=$dev size=$size"
    result=$("$SUSFS_BIN" add_sus_kstat_redirect "$vpath" "$rpath" \
        "$ino" "$dev" "$nlink" "$size" \
        "$atime" 0 "$mtime" 0 "$ctime" 0 \
        "$blocks" "$blksize" 2>&1)
    rc=$?
    log_susfs_result "$rc" "add_sus_kstat_redirect" "$vpath"

    if [ $rc -eq 0 ]; then
        SUSFS_STATS_KSTAT=$((SUSFS_STATS_KSTAT + 1))
        log_info "Font redirect complete: $vpath -> $rpath (ino=$ino dev=$dev size=$size)"
        log_func_exit "apply_font_redirect" "0"
        return 0
    else
        log_err "add_sus_kstat_redirect failed for $vpath: $result"
        SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
        if [ "$open_redirect_applied" = "1" ]; then
            log_warn "Partial success: open_redirect OK, kstat_redirect failed"
            log_func_exit "apply_font_redirect" "0" "partial"
            return 0
        fi
        log_func_exit "apply_font_redirect" "1"
        return 1
    fi
}

# Re-apply kstat for deferred entries after boot settles
late_kstat_pass() {
    log_func_enter "late_kstat_pass"
    local deferred="${ZEROMOUNT_DATA:-/data/adb/zeromount}/.deferred_kstat"
    [ ! -f "$deferred" ] && { log_debug "No deferred kstat entries"; log_func_exit "late_kstat_pass" "0"; return 0; }
    [ "$HAS_SUSFS" != "1" ] && { rm -f "$deferred"; return 0; }

    local ok=0 fail=0
    while IFS='|' read -r vpath metadata rpath; do
        [ -z "$vpath" ] && continue
        local real_stat=$(stat -c '%s|%b|%B' "$rpath" 2>/dev/null)
        if [ -z "$real_stat" ]; then
            log_warn "Late kstat: still cannot stat $rpath"
            fail=$((fail+1))
            continue
        fi

        local size=$(echo "$real_stat" | cut -d'|' -f1)
        local blocks=$(echo "$real_stat" | cut -d'|' -f2)
        local blksize=$(echo "$real_stat" | cut -d'|' -f3)
        local ino dev nlink atime mtime ctime
        IFS='|' read -r ino dev nlink _ atime mtime ctime _ _ _ <<EOF
$metadata
EOF

        if [ "$HAS_SUS_KSTAT_REDIRECT" = "1" ]; then
            if "$SUSFS_BIN" add_sus_kstat_redirect "$vpath" "$rpath" \
                "$ino" "$dev" "$nlink" "$size" "$atime" 0 "$mtime" 0 "$ctime" 0 "$blocks" "$blksize" >/dev/null 2>&1; then
                log_info "Late kstat applied (redirect): $vpath"
                ok=$((ok+1))
                continue
            fi
        fi
        if "$SUSFS_BIN" add_sus_kstat_statically "$vpath" \
            "$ino" "$dev" "$nlink" "$size" "$atime" 0 "$mtime" 0 "$ctime" 0 "$blocks" "$blksize" >/dev/null 2>&1; then
            log_info "Late kstat applied (statically): $vpath"
            ok=$((ok+1))
        else
            log_warn "Late kstat failed: $vpath"
            fail=$((fail+1))
        fi
    done < "$deferred"
    rm -f "$deferred"
    log_info "Late kstat pass complete: $ok succeeded, $fail failed"
    log_func_exit "late_kstat_pass" "$ok"
}

susfs_apply_mount_hiding() {
    local vpath="$1"
    log_func_enter "susfs_apply_mount_hiding" "$vpath"

    [ "$HAS_SUSFS" != "1" ] && { log_func_exit "susfs_apply_mount_hiding" "skip"; return 0; }
    [ "$HAS_SUS_MOUNT" != "1" ] && { log_func_exit "susfs_apply_mount_hiding" "skip"; return 0; }

    local mount_point
    mount_point=$(awk -v path="$vpath" '
        ($3 == "overlay" || $3 == "tmpfs") && path ~ "^"$2 {
            print $2
            exit
        }
    ' /proc/mounts 2>/dev/null)

    if [ -n "$mount_point" ]; then
        log_debug "Found mount point: $mount_point"

        echo "$SUSFS_HIDDEN_MOUNTS" | grep -qF "|$mount_point|" && { log_func_exit "susfs_apply_mount_hiding" "0" "already hidden"; return 0; }

        log_susfs_cmd "add_sus_mount" "$mount_point"
        local result rc
        result=$("$SUSFS_BIN" add_sus_mount "$mount_point" 2>&1)
        rc=$?
        log_susfs_result "$rc" "add_sus_mount" "$mount_point"

        if [ $rc -eq 0 ]; then
            SUSFS_HIDDEN_MOUNTS="${SUSFS_HIDDEN_MOUNTS}|${mount_point}|"
            SUSFS_STATS_MOUNT=$((SUSFS_STATS_MOUNT + 1))
            susfs_update_config "sus_mount.txt" "$mount_point"
            log_func_exit "susfs_apply_mount_hiding" "0" "hidden"
            return 0
        else
            log_err "sus_mount failed for $mount_point: $result"
            SUSFS_STATS_ERRORS=$((SUSFS_STATS_ERRORS + 1))
            log_func_exit "susfs_apply_mount_hiding" "1"
            return 1
        fi
    else
        log_debug "No overlay mount found for $vpath"
        log_func_exit "susfs_apply_mount_hiding" "0" "no mount"
        return 0
    fi
}

susfs_update_config() {
    local config_name="$1"
    local entry="$2"
    log_func_enter "susfs_update_config" "$config_name" "$entry"

    [ -z "$SUSFS_CONFIG_DIR" ] && { log_func_exit "susfs_update_config" "skip"; return 0; }

    local config_path="$SUSFS_CONFIG_DIR/$config_name"

    grep -qxF "$entry" "$config_path" 2>/dev/null && { log_func_exit "susfs_update_config" "0" "exists"; return 0; }

    {
        echo "# [ZeroMount] $(date '+%Y-%m-%d %H:%M:%S')"
        echo "$entry"
    } >> "$config_path" 2>/dev/null

    [ $? -eq 0 ] && { log_debug "Updated $config_name: $entry"; log_func_exit "susfs_update_config" "0"; return 0; }
    log_warn "Failed to write to $config_path"
    log_func_exit "susfs_update_config" "1"
    return 1
}

susfs_clean_zeromount_entries() {
    log_func_enter "susfs_clean_zeromount_entries"

    [ -z "$SUSFS_CONFIG_DIR" ] && { log_func_exit "susfs_clean_zeromount_entries" "skip"; return 0; }

    log_info "Cleaning ZeroMount entries from SUSFS configs..."

    local cleaned=0
    for config in sus_path.txt sus_path_loop.txt sus_mount.txt sus_maps.txt; do
        local config_path="$SUSFS_CONFIG_DIR/$config"
        if [ -f "$config_path" ]; then
            local before=$(wc -l < "$config_path")
            busybox sed -i '/# \[ZeroMount\]/,+1d' "$config_path" 2>/dev/null
            local after=$(wc -l < "$config_path")
            local removed=$((before - after))
            [ $removed -gt 0 ] && cleaned=$((cleaned + removed))
        fi
    done

    log_info "Cleaned $cleaned total entries"
    log_func_exit "susfs_clean_zeromount_entries" "$cleaned"
}

susfs_clean_module_entries() {
    local mod_name="$1"
    local tracking_file="$2"
    log_func_enter "susfs_clean_module_entries" "$mod_name"

    [ "$HAS_SUSFS" != "1" ] && { log_func_exit "susfs_clean_module_entries" "skip"; return 0; }
    [ -z "$SUSFS_CONFIG_DIR" ] && { log_func_exit "susfs_clean_module_entries" "skip"; return 0; }
    [ ! -f "$tracking_file" ] && { log_func_exit "susfs_clean_module_entries" "skip"; return 0; }

    log_info "Cleaning SUSFS entries for module: $mod_name"

    local cleaned=0
    for config in sus_path.txt sus_path_loop.txt sus_maps.txt sus_mount.txt; do
        local config_path="$SUSFS_CONFIG_DIR/$config"
        [ ! -f "$config_path" ] && continue

        local before=$(wc -l < "$config_path")
        local temp_file="${config_path}.tmp.$$"
        : > "$temp_file" || return 1

        while IFS= read -r line; do
            local skip=0
            echo "$line" | grep -q "^# \[ZeroMount\]" && continue
            while IFS= read -r tracked_path; do
                [ -z "$tracked_path" ] && continue
                [ "$line" = "$tracked_path" ] && { skip=1; break; }
            done < "$tracking_file"
            [ "$skip" -eq 0 ] && echo "$line"
        done < "$config_path" > "$temp_file"

        mv "$temp_file" "$config_path" 2>/dev/null

        local after=$(wc -l < "$config_path")
        local removed=$((before - after))
        [ $removed -gt 0 ] && cleaned=$((cleaned + removed))
    done

    log_info "Cleaned $cleaned SUSFS entries for module $mod_name"
    log_func_exit "susfs_clean_module_entries" "$cleaned"
    return 0
}

susfs_clean_module_metadata_cache() {
    local mod_name="$1"
    local tracking_file="$2"
    log_func_enter "susfs_clean_module_metadata_cache" "$mod_name"

    [ ! -d "$METADATA_CACHE_DIR" ] && { log_func_exit "susfs_clean_module_metadata_cache" "skip"; return 0; }
    [ ! -f "$tracking_file" ] && { log_func_exit "susfs_clean_module_metadata_cache" "skip"; return 0; }

    log_info "Cleaning metadata cache for module: $mod_name"

    local cleaned=0
    while IFS= read -r vpath; do
        [ -z "$vpath" ] && continue

        local cache_key
        cache_key=$(echo "$vpath" | md5sum 2>/dev/null | cut -d' ' -f1)
        [ -z "$cache_key" ] && cache_key=$(echo "$vpath" | cksum | cut -d' ' -f1)

        local cache_file="$METADATA_CACHE_DIR/$cache_key"
        [ -f "$cache_file" ] && { rm -f "$cache_file"; cleaned=$((cleaned + 1)); }
    done < "$tracking_file"

    log_info "Cleaned $cleaned metadata cache entries for module $mod_name"
    log_func_exit "susfs_clean_module_metadata_cache" "$cleaned"
    return 0
}

# Apply SUSFS protections for a VFS rule (called after zm add)
zm_register_rule_with_susfs() {
    local vpath="$1"
    local rpath="$2"
    log_func_enter "zm_register_rule_with_susfs" "$vpath" "$rpath"

    [ -z "$vpath" ] && { log_warn "zm_register_rule_with_susfs: empty vpath"; return 1; }
    [ -z "$rpath" ] && { log_warn "zm_register_rule_with_susfs: empty rpath"; return 1; }
    [ ${#vpath} -gt 4096 ] && { log_warn "zm_register_rule_with_susfs: vpath exceeds PATH_MAX"; return 1; }
    [ ${#rpath} -gt 4096 ] && { log_warn "zm_register_rule_with_susfs: rpath exceeds PATH_MAX"; return 1; }

    case "$vpath" in
        */../*|*/./*|../*|./*) log_warn "zm_register_rule_with_susfs: path contains traversal sequence"; return 1 ;;
    esac

    log_info "Applying SUSFS protections: $vpath"

    local actions
    actions=$(susfs_classify_path "$vpath")
    log_debug "Classification: $actions"

    local metadata
    metadata=$(susfs_get_cached_metadata "$vpath")

    log_debug "Applying SUSFS actions"
    if [ "$HAS_SUSFS" = "1" ]; then
        case "$actions" in
            *sus_path_loop*) log_trace "Applying sus_path_loop"; susfs_apply_path "$vpath" 1 ;;
            *sus_path*) log_trace "Applying sus_path"; susfs_apply_path "$vpath" 0 ;;
        esac

        case "$actions" in *sus_maps*) log_trace "Applying sus_maps"; susfs_apply_maps "$vpath" ;; esac
        case "$actions" in *sus_kstat*) log_trace "Applying sus_kstat"; susfs_apply_kstat "$vpath" "$metadata" "$rpath" ;; esac
        case "$actions" in *sus_mount_check*) log_trace "Checking mount hiding"; susfs_apply_mount_hiding "$vpath" ;; esac
    else
        log_debug "SUSFS not available, skipping protections"
    fi

    log_func_exit "zm_register_rule_with_susfs" "0"
    return 0
}

# Batch capture metadata for a module
susfs_capture_module_metadata() {
    local mod_path="$1"
    local partitions="${2:-system vendor product system_ext odm oem}"
    log_func_enter "susfs_capture_module_metadata" "$mod_path"

    local total_files=0

    for partition in $partitions; do
        if [ -d "$mod_path/$partition" ]; then
            log_debug "Scanning partition: $partition"
            local partition_files=$(find "$mod_path/$partition" -type f 2>/dev/null | wc -l)
            total_files=$((total_files + partition_files))
            log_trace "Found $partition_files files in $partition"

            find "$mod_path/$partition" -type f 2>/dev/null | while read -r real_path; do
                local vpath="${real_path#$mod_path}"
                susfs_capture_metadata "$vpath"
            done
        fi
    done

    log_info "Captured metadata for $total_files files from $(basename "$mod_path")"
    log_func_exit "susfs_capture_module_metadata" "$total_files"
    return 0
}

susfs_status() {
    log_func_enter "susfs_status"

    echo "========================================"
    echo "SUSFS Integration Status"
    echo "========================================"

    if [ "$HAS_SUSFS" = "1" ]; then
        echo "Status: ENABLED"
        echo "Binary: $SUSFS_BIN"
        echo ""
        echo "Capabilities:"
        echo "  sus_path:       $HAS_SUS_PATH"
        echo "  sus_path_loop:  $HAS_SUS_PATH_LOOP"
        echo "  sus_mount:      $HAS_SUS_MOUNT"
        echo "  sus_kstat:      $HAS_SUS_KSTAT"
        echo "  kstat_redirect: $HAS_SUS_KSTAT_REDIRECT"
        echo "  sus_maps:       $HAS_SUS_MAPS"
        echo "  open_redirect:  $HAS_OPEN_REDIRECT"
        echo ""
        echo "Config Directory: ${SUSFS_CONFIG_DIR:-none}"
        echo ""
        echo "Statistics (this session):"
        echo "  Paths hidden:   $SUSFS_STATS_PATH"
        echo "  Kstat applied:  $SUSFS_STATS_KSTAT"
        echo "  Mounts hidden:  $SUSFS_STATS_MOUNT"
        echo "  Maps hidden:    $SUSFS_STATS_MAPS"
        echo "  Open redirects: $SUSFS_STATS_OPEN_REDIRECT"
        echo "  Errors:         $SUSFS_STATS_ERRORS"
    else
        echo "Status: DISABLED (ZeroMount-only mode)"
        echo "Reason: SUSFS binary not found"
    fi

    echo "========================================"
    log_func_exit "susfs_status" "displayed"
}

susfs_reset_stats() {
    log_func_enter "susfs_reset_stats"
    SUSFS_STATS_PATH=0
    SUSFS_STATS_KSTAT=0
    SUSFS_STATS_MOUNT=0
    SUSFS_STATS_MAPS=0
    SUSFS_STATS_OPEN_REDIRECT=0
    SUSFS_STATS_ERRORS=0
    log_debug "Statistics reset"
    log_func_exit "susfs_reset_stats" "0"
}
