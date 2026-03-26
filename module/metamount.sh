#!/system/bin/sh
# KSU/APatch metamodule mount hook (post-fs-data phase).
# Runs the full mount pipeline BEFORE notify-module-mounted so all
# overlays/bind mounts are established before Zygote starts.
MODDIR="${0%/*}"
LOG="zeromount"

# Single-instance lock -- KSU metamodule mode can double-fire.
# noclobber makes the redirect use O_CREAT|O_EXCL, which is atomic.
LOCKFILE="/dev/zeromount_metamount_lock"
( set -o noclobber; > "$LOCKFILE" ) 2>/dev/null || { ksud kernel notify-module-mounted 2>/dev/null; exit 0; }

echo "$LOG: metamount.sh entered (post-fs-data)" > /dev/kmsg 2>/dev/null

. "$MODDIR/common.sh"

mkdir -p /data/adb/zeromount/flags

# Reconcile external SUSFS module config changes made outside zeromount
EXTERNAL=$(cat /data/adb/zeromount/flags/external_susfs 2>/dev/null || echo none)
if [ "$EXTERNAL" != "none" ] && [ -n "$ABI" ] && [ -x "$BIN" ]; then
    "$BIN" bridge reconcile "$EXTERNAL" 2>/dev/null
fi

if [ -n "$ABI" ] && [ -x "$BIN" ]; then
    # Run other modules' post-fs-data scripts BEFORE the mount pipeline.
    # Modules like MSD create files at runtime (e.g., seapp_contexts) that
    # must exist before ZeroMount scans and mounts. In stock KSU, scripts
    # run before mounts; the metamodule model inverts this, so we restore
    # the expected ordering by executing scripts here.
    #
    # KSU will re-execute these scripts after notify-module-mounted; well-
    # behaved scripts (including MSD) produce idempotent output, so double
    # execution is safe.
    echo "$LOG: running other modules' post-fs-data scripts (pre-mount)" > /dev/kmsg 2>/dev/null
    for _pfd in /data/adb/modules/*/post-fs-data.sh; do
        [ ! -f "$_pfd" ] && continue
        _pfd_dir="${_pfd%/post-fs-data.sh}"
        _pfd_mod="${_pfd_dir##*/}"
        # Skip ourselves, disabled, and removed modules
        case "$_pfd_mod" in
            zeromount|meta-zeromount) continue ;;
        esac
        [ -f "${_pfd_dir}/disable" ] && continue
        [ -f "${_pfd_dir}/remove" ] && continue
        echo "$LOG: pre-mount: executing $_pfd_mod/post-fs-data.sh" > /dev/kmsg 2>/dev/null
        (cd "$_pfd_dir" && timeout 30 sh post-fs-data.sh) 2>/dev/null
        echo "$LOG: pre-mount: $_pfd_mod exited (rc=$?)" > /dev/kmsg 2>/dev/null
    done

    echo "$LOG: starting mount pipeline (pre-zygote)" > /dev/kmsg 2>/dev/null
    timeout 60 "$BIN" mount
    RET=$?
    if [ "$RET" -eq 124 ]; then
        echo "$LOG: mount pipeline hung after 60s, forced termination" > /dev/kmsg 2>/dev/null
    fi
    echo "$LOG: mount pipeline exited (rc=$RET)" > /dev/kmsg 2>/dev/null

else
    echo "$LOG: binary not found (ABI=$ABI), skipping pipeline" > /dev/kmsg 2>/dev/null
fi

echo "$LOG: calling notify-module-mounted" > /dev/kmsg 2>/dev/null
ksud kernel notify-module-mounted 2>/dev/null
exit 0
