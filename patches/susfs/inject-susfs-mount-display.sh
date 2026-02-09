#!/bin/bash
# Adds zeromount UID exclusion checks to the 3 show_* functions
# (show_vfsmnt, show_mountinfo, show_vfsstat) in fs/proc_namespace.c.
# Runs AFTER the SUSFS GKI patch has been applied to kernel source.
#
# Usage: ./inject-susfs-mount-display.sh <KERNEL_COMMON_DIR>

set -e

KERNEL_DIR="$1"

if [ -z "$KERNEL_DIR" ]; then
    echo "Usage: $0 <KERNEL_COMMON_DIR>"
    exit 1
fi

PROC_NS="$KERNEL_DIR/fs/proc_namespace.c"

if [ ! -f "$PROC_NS" ]; then
    echo "FATAL: $PROC_NS not found"
    exit 1
fi

echo "=== inject-susfs-mount-display ==="
echo "    Target: $PROC_NS"
inject_count=0

# --- 1. Add extern declaration ---
if grep -q 'susfs_is_uid_zeromount_excluded' "$PROC_NS"; then
    echo "[=] zeromount extern already present"
else
    echo "[+] Injecting zeromount extern declarations"
    sed -i '/^extern bool susfs_is_current_ksu_domain(void);/a #ifdef CONFIG_ZEROMOUNT\nextern bool susfs_is_uid_zeromount_excluded(uid_t uid);\n#endif' "$PROC_NS"
    ((inject_count++)) || true
fi

# --- 2. Inject zeromount condition into show_* blocks ---
# Upstream SUSFS adds:    !susfs_is_current_ksu_domain())
# We split the closing ) and insert the zeromount guard before it.
inline_count=$(grep -c '!susfs_is_uid_zeromount_excluded' "$PROC_NS" || true)
if [ "$inline_count" -ge 3 ]; then
    echo "[=] zeromount inline checks already present ($inline_count found)"
else
    echo "[+] Injecting zeromount checks into show_* functions"
    awk '
    /^\t\t!susfs_is_current_ksu_domain\(\)\)$/ {
        print "\t\t!susfs_is_current_ksu_domain()"
        print "#ifdef CONFIG_ZEROMOUNT"
        print "\t\t&& !susfs_is_uid_zeromount_excluded(current_uid().val)"
        print "#endif"
        print "\t\t)"
        next
    }
    { print }
    ' "$PROC_NS" > "$PROC_NS.tmp" && mv "$PROC_NS.tmp" "$PROC_NS"
    ((inject_count++)) || true
fi

count=$(grep -c 'susfs_is_uid_zeromount_excluded' "$PROC_NS" || true)
if [ "$count" -lt 4 ]; then
    echo "FATAL: expected at least 4 zeromount references, found $count"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
