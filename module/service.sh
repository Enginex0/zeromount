MODDIR=${0%/*}
ZEROMOUNT_DATA="/data/adb/zeromount"
LOADER="$MODDIR/bin/zm"
SUSFS_BIN="/data/adb/ksu/bin/ksu_susfs"

mkdir -p "$ZEROMOUNT_DATA"

[ -f "$MODDIR/logging.sh" ] && . "$MODDIR/logging.sh"
log_init "service"

log_section "ZeroMount Service Start"

if [ -x "$SUSFS_BIN" ]; then
    [ -e "/dev/zeromount" ] && "$SUSFS_BIN" add_sus_path_loop /dev/zeromount 2>/dev/null && log_info "Hidden: /dev/zeromount"
    [ -e "/sys/kernel/zeromount" ] && "$SUSFS_BIN" add_sus_path_loop /sys/kernel/zeromount 2>/dev/null && log_info "Hidden: /sys/kernel/zeromount"
else
    log_warn "SUSFS binary not found at $SUSFS_BIN"
fi

if [ -f "$ZEROMOUNT_DATA/.exclusion_list" ]; then
    if [ ! -x "$LOADER" ]; then
        log_err "zm binary not found or not executable: $LOADER"
    else
        log_info "Applying UID exclusions..."
        exclusion_count=0
        exclusion_errors=0

        while IFS= read -r uid; do
            [ -z "$uid" ] && continue
            case "$uid" in
                *[!0-9]*)
                    log_warn "Invalid UID in exclusion list: $uid"
                    continue
                    ;;
            esac
            if "$LOADER" blk "$uid" 2>/dev/null; then
                log_debug "Blocked UID: $uid"
                exclusion_count=$((exclusion_count + 1))
            else
                log_err "Failed to block UID: $uid"
                exclusion_errors=$((exclusion_errors + 1))
            fi
        done < "$ZEROMOUNT_DATA/.exclusion_list"

        log_info "UID exclusions applied: $exclusion_count success, $exclusion_errors failed"
    fi
else
    log_debug "No exclusion list found"
fi

rm -rf "$MODDIR/webroot/link"
ln -sf "$ZEROMOUNT_DATA" "$MODDIR/webroot/link"
log_info "Symlink created: webroot/link -> $ZEROMOUNT_DATA"

log_section "Service Complete"
