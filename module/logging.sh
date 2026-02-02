#!/system/bin/sh
# ============================================================
# ZeroMount Unified Logging Library
# ============================================================
# Provides structured, organized logging for all ZeroMount components.
#
# Log Directory Structure:
#   /data/adb/zeromount/logs/
#   ├── kernel/           # Kernel-level logs (dmesg filtered)
#   ├── frontend/         # Module scripts logs
#   │   ├── service.log
#   │   ├── post-fs-data.log
#   │   ├── monitor.log
#   │   └── sync.log
#   ├── susfs/            # SUSFS integration logs
#   │   └── susfs.log
#   └── archive/          # Rotated old logs
#
# Usage:
#   . "$MODDIR/logging.sh"
#   log_init "service"      # Initialize for service.sh
#   log_info "Message"      # Log info message
#   log_func_enter "func_name" "$arg1" "$arg2"
#   log_func_exit "func_name" "$result"
# ============================================================

# ============================================================
# CONFIGURATION
# ============================================================
ZEROMOUNT_BASE="${ZEROMOUNT_BASE:-/data/adb/zeromount}"
ZEROMOUNT_LOG_BASE="$ZEROMOUNT_BASE/logs"

# Log directories
LOG_DIR_KERNEL="$ZEROMOUNT_LOG_BASE/kernel"
LOG_DIR_FRONTEND="$ZEROMOUNT_LOG_BASE/frontend"
LOG_DIR_SUSFS="$ZEROMOUNT_LOG_BASE/susfs"
LOG_DIR_ARCHIVE="$ZEROMOUNT_LOG_BASE/archive"

# Log levels: 0=OFF, 1=ERROR, 2=WARN, 3=INFO, 4=DEBUG, 5=TRACE
LOG_LEVEL="${LOG_LEVEL:-3}"

# Current component (set by log_init)
LOG_COMPONENT=""
LOG_FILE=""

# Max log file size (1MB)
LOG_MAX_SIZE=1048576

# ANSI colors for terminal output (disabled by default in scripts)
LOG_USE_COLORS="${LOG_USE_COLORS:-0}"

# ============================================================
# INITIALIZATION
# ============================================================

# Create all log directories
log_create_dirs() {
    mkdir -p "$LOG_DIR_KERNEL" 2>/dev/null
    mkdir -p "$LOG_DIR_FRONTEND" 2>/dev/null
    mkdir -p "$LOG_DIR_SUSFS" 2>/dev/null
    mkdir -p "$LOG_DIR_ARCHIVE" 2>/dev/null
    chmod 755 "$ZEROMOUNT_LOG_BASE" 2>/dev/null
}

# Initialize logging for a component
# Usage: log_init "service" | "post-fs-data" | "monitor" | "sync" | "susfs"
log_init() {
    local component="$1"
    LOG_COMPONENT="$component"

    # Create directories
    log_create_dirs

    # Set log file based on component
    case "$component" in
        service|post-fs-data|monitor|sync|uninstall|metamount)
            LOG_FILE="$LOG_DIR_FRONTEND/${component}.log"
            ;;
        susfs)
            LOG_FILE="$LOG_DIR_SUSFS/susfs.log"
            ;;
        kernel)
            LOG_FILE="$LOG_DIR_KERNEL/kernel.log"
            ;;
        *)
            LOG_FILE="$LOG_DIR_FRONTEND/${component}.log"
            ;;
    esac

    # Rotate log if too large
    log_rotate_if_needed

    # Write session header
    {
        echo ""
        echo "========================================"
        echo "[$component] Session Start: $(date '+%Y-%m-%d %H:%M:%S')"
        echo "========================================"
    } >> "$LOG_FILE" 2>/dev/null
}

# ============================================================
# LOG ROTATION
# ============================================================

log_rotate_if_needed() {
    [ -z "$LOG_FILE" ] && return
    [ ! -f "$LOG_FILE" ] && return

    local size=$(stat -c %s "$LOG_FILE" 2>/dev/null || echo 0)
    if [ "$size" -gt "$LOG_MAX_SIZE" ]; then
        local archive_name="${LOG_FILE##*/}.$(date '+%Y%m%d_%H%M%S')"
        mv "$LOG_FILE" "$LOG_DIR_ARCHIVE/$archive_name" 2>/dev/null
        # Keep only last 5 archived logs per component
        ls -t "$LOG_DIR_ARCHIVE/${LOG_COMPONENT}."* 2>/dev/null | awk 'NR>5' | xargs rm -f 2>/dev/null
    fi
}

# ============================================================
# CORE LOGGING FUNCTIONS
# ============================================================

# Internal: Format and write log entry
_log_write() {
    local level="$1"
    local message="$2"
    local timestamp=$(date '+%H:%M:%S.%3N' 2>/dev/null || date '+%H:%M:%S')
    local entry="[$timestamp] [$level] $message"

    [ -n "$LOG_FILE" ] && echo "$entry" >> "$LOG_FILE" 2>/dev/null
}

# Check if log level is enabled
_log_level_enabled() {
    local required="$1"
    [ "$LOG_LEVEL" -ge "$required" ]
}

# ERROR (level 1) - Always logged
log_err() {
    _log_level_enabled 1 && _log_write "ERROR" "$*"
}

# WARN (level 2)
log_warn() {
    _log_level_enabled 2 && _log_write "WARN " "$*"
}

# INFO (level 3)
log_info() {
    _log_level_enabled 3 && _log_write "INFO " "$*"
}

# DEBUG (level 4)
log_debug() {
    _log_level_enabled 4 && _log_write "DEBUG" "$*"
}

# TRACE (level 5) - Most verbose
log_trace() {
    _log_level_enabled 5 && _log_write "TRACE" "$*"
}

# ============================================================
# STRUCTURED LOGGING HELPERS
# ============================================================

# Log function entry with parameters
# Usage: log_func_enter "function_name" "$param1" "$param2"
log_func_enter() {
    local func="$1"
    shift
    local params=""
    local i=1
    for arg in "$@"; do
        [ -n "$params" ] && params="$params, "
        params="${params}arg${i}='$arg'"
        i=$((i + 1))
    done
    log_debug ">>> ENTER: ${func}(${params})"
}

# Log function exit with result
# Usage: log_func_exit "function_name" "$result" ["reason"]
log_func_exit() {
    local func="$1"
    local result="$2"
    local reason="${3:-}"
    if [ -n "$reason" ]; then
        log_debug "<<< EXIT: ${func} (result=${result}, ${reason})"
    else
        log_debug "<<< EXIT: ${func} (result=${result})"
    fi
}

# Log external command execution
# Usage: log_cmd "command" ["description"]
log_cmd() {
    local cmd="$1"
    local desc="${2:-}"
    [ -n "$desc" ] && log_debug "CMD: $desc"
    log_trace "CMD: $cmd"
}

# Log command result
# Usage: log_cmd_result "$?" "$output" ["command_name"]
log_cmd_result() {
    local rc="$1"
    local output="$2"
    local name="${3:-command}"
    if [ "$rc" -eq 0 ]; then
        log_trace "CMD_OK: $name (rc=0)"
    else
        log_warn "CMD_FAIL: $name (rc=$rc) output='$output'"
    fi
}

# Log a section header
# Usage: log_section "Section Name"
log_section() {
    local name="$1"
    log_info "========== $name =========="
}

# Log a subsection
# Usage: log_subsection "Subsection Name"
log_subsection() {
    local name="$1"
    log_info "--- $name ---"
}

# ============================================================
# SUSFS-SPECIFIC LOGGING
# ============================================================

# Log SUSFS command (before execution)
# Usage: log_susfs_cmd "add_sus_path" "/path/to/hide"
log_susfs_cmd() {
    local cmd="$1"
    shift
    log_debug "[SUSFS] Executing: ksu_susfs $cmd $*"
}

# Log SUSFS command result
# Usage: log_susfs_result "$?" "add_sus_path" "/path"
log_susfs_result() {
    local rc="$1"
    local cmd="$2"
    local target="$3"
    if [ "$rc" -eq 0 ]; then
        log_debug "[SUSFS] OK: $cmd '$target'"
    else
        log_warn "[SUSFS] FAIL: $cmd '$target' (rc=$rc)"
    fi
}

# ============================================================
# KERNEL LOG COLLECTION
# ============================================================

# Collect ZeroMount kernel logs from dmesg
# Usage: log_collect_kernel
log_collect_kernel() {
    local kernel_log="$LOG_DIR_KERNEL/kernel.log"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')

    {
        echo ""
        echo "========================================"
        echo "Kernel Log Snapshot: $timestamp"
        echo "========================================"
        dmesg 2>/dev/null | grep -iE "zeromount|vfs_dcache|susfs" | tail -200
    } >> "$kernel_log" 2>/dev/null
}

# ============================================================
# SUMMARY LOGGING
# ============================================================

# Log execution summary
# Usage: log_summary "modules=5" "files=100" "errors=0"
log_summary() {
    log_info "---------- Summary ----------"
    for item in "$@"; do
        log_info "  $item"
    done
    log_info "-----------------------------"
}

# Log timing information
# Usage: log_timing "$start_time" "$end_time"
log_timing() {
    local start="$1"
    local end="$2"
    local elapsed=$((end - start))
    log_info "Execution time: ${elapsed}s"
}

# ============================================================
# ERROR TRACKING
# ============================================================

# Global error counter
_LOG_ERROR_COUNT=0

# Increment error count
log_error_inc() {
    _LOG_ERROR_COUNT=$((_LOG_ERROR_COUNT + 1))
}

# Get error count
log_error_count() {
    echo "$_LOG_ERROR_COUNT"
}

# Reset error count
log_error_reset() {
    _LOG_ERROR_COUNT=0
}

# ============================================================
# COMPATIBILITY LAYER
# ============================================================

# Legacy log function (for backward compatibility)
log() {
    log_info "$@"
}

# Early boot logging (when log_init hasn't been called)
log_early() {
    local msg="$1"
    local early_log="$ZEROMOUNT_BASE/early_boot.log"
    mkdir -p "$ZEROMOUNT_BASE" 2>/dev/null
    echo "[$(date '+%H:%M:%S')] $msg" >> "$early_log" 2>/dev/null
}

# ============================================================
# UTILITY FUNCTIONS
# ============================================================

# Get all log files for a component
log_get_files() {
    local component="$1"
    case "$component" in
        kernel)
            ls -la "$LOG_DIR_KERNEL/"*.log 2>/dev/null
            ;;
        frontend)
            ls -la "$LOG_DIR_FRONTEND/"*.log 2>/dev/null
            ;;
        susfs)
            ls -la "$LOG_DIR_SUSFS/"*.log 2>/dev/null
            ;;
        all)
            ls -la "$ZEROMOUNT_LOG_BASE/"*/*.log 2>/dev/null
            ;;
    esac
}

# Clear all logs
log_clear_all() {
    rm -f "$LOG_DIR_KERNEL/"*.log 2>/dev/null
    rm -f "$LOG_DIR_FRONTEND/"*.log 2>/dev/null
    rm -f "$LOG_DIR_SUSFS/"*.log 2>/dev/null
    rm -f "$LOG_DIR_ARCHIVE/"* 2>/dev/null
    log_info "All logs cleared"
}

# Export log status
log_status() {
    echo "ZeroMount Logging Status"
    echo "======================"
    echo "Log Level: $LOG_LEVEL"
    echo "Component: $LOG_COMPONENT"
    echo "Log File: $LOG_FILE"
    echo ""
    echo "Directory Structure:"
    echo "  Kernel:   $LOG_DIR_KERNEL"
    echo "  Frontend: $LOG_DIR_FRONTEND"
    echo "  SUSFS:    $LOG_DIR_SUSFS"
    echo "  Archive:  $LOG_DIR_ARCHIVE"
    echo ""
    echo "Log Files:"
    log_get_files "all"
}

# ============================================================
# AUTO-INITIALIZATION
# ============================================================

# Create directories on source (safe operation)
log_create_dirs 2>/dev/null || true
