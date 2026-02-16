#!/bin/bash
# Adds OPEN_REDIRECT_ALL two-branch check into do_filp_open.
# Runs AFTER the SUSFS GKI patch has been applied to kernel source.
#
# Usage: ./inject-susfs-vfs-open-redirect-all.sh <KERNEL_COMMON_DIR>

set -e

KERNEL_DIR="$1"

if [ -z "$KERNEL_DIR" ]; then
    echo "Usage: $0 <KERNEL_COMMON_DIR>"
    exit 1
fi

NAMEI="$KERNEL_DIR/fs/namei.c"

if [ ! -f "$NAMEI" ]; then
    echo "FATAL: $NAMEI not found"
    exit 1
fi

KV="${KERNEL_VERSION:-}"
if [[ -z "$KV" ]]; then
    echo "FATAL: KERNEL_VERSION not set"; exit 1
fi

# set_nameidata gained a 4th arg (const struct path *root) between 5.10 and 5.15
# NOTE: 5.4 is GKI but upstream SUSFS has no 5.4 branch and the build workflow
# does not offer 5.4 as a kernel_version choice. The branch is kept defensively
# in case SUSFS adds 5.4 support in the future.
case "$KV" in
    5.10|5.4)          NAMEIDATA_ARGS=3 ;;
    5.15|6.1|6.6|6.12) NAMEIDATA_ARGS=4 ;;
    *)                 echo "FATAL: Unsupported kernel: $KV"; exit 1 ;;
esac

echo "=== inject-susfs-vfs-open-redirect-all ==="
echo "    Target: $NAMEI (kernel $KV, set_nameidata=${NAMEIDATA_ARGS}-arg)"
inject_count=0

# --- 1. Add extern for susfs_get_redirected_path_all ---
if grep -q 'susfs_get_redirected_path_all' "$NAMEI"; then
    echo "[=] susfs_get_redirected_path_all extern already present"
else
    echo "[+] Injecting susfs_get_redirected_path_all extern"
    sed -i '/^extern struct filename\* susfs_get_redirected_path(unsigned long ino);/a extern struct filename* susfs_get_redirected_path_all(unsigned long ino);' "$NAMEI"
    ((inject_count++)) || true
fi

# --- 2. Replace single-check redirect with two-branch (ALL first, then per-UID) ---
if grep -q 'susfs_get_redirected_path_all.*i_ino' "$NAMEI"; then
    echo "[=] OPEN_REDIRECT_ALL two-branch check already present in do_filp_open"
else
    echo "[+] Replacing do_filp_open redirect block with two-branch check"

    if [ "$NAMEIDATA_ARGS" -eq 4 ]; then
        SET_ND='set_nameidata(&nd, dfd, fake_pathname, NULL);'
    else
        SET_ND='set_nameidata(&nd, dfd, fake_pathname);'
    fi

    # Upstream GKI patches use test_bit(AS_FLAGS_OPEN_REDIRECT, ...) — match both patterns
    awk -v set_nd="$SET_ND" '
    /^#ifdef CONFIG_KSU_SUSFS_OPEN_REDIRECT/ {
        block = $0 "\n"
        is_redirect_block = 0
        while ((getline line) > 0) {
            block = block line "\n"
            if (line ~ /BIT_OPEN_REDIRECT|AS_FLAGS_OPEN_REDIRECT/) is_redirect_block = 1
            if (line ~ /^#endif/) break
        }
        if (is_redirect_block && !already_replaced) {
            already_replaced = 1
            print "#ifdef CONFIG_KSU_SUSFS_OPEN_REDIRECT"
            print "\tif (!IS_ERR(filp)) {"
            print "\t\tif (unlikely(test_bit(AS_FLAGS_OPEN_REDIRECT_ALL, &filp->f_inode->i_mapping->flags))) {"
            print "\t\t\tfake_pathname = susfs_get_redirected_path_all(filp->f_inode->i_ino);"
            print "\t\t\tif (!IS_ERR(fake_pathname)) {"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tfilp_close(filp, NULL);"
            printf "\t\t\t\t%s\n", set_nd
            print "\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_RCU);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ECHILD)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ESTALE)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_REVAL);"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tputname(fake_pathname);"
            print "\t\t\t\treturn filp;"
            print "\t\t\t}"
            print "\t\t} else if (unlikely(test_bit(AS_FLAGS_OPEN_REDIRECT, &filp->f_inode->i_mapping->flags)) &&"
            print "\t\t\t   current_uid().val < 2000) {"
            print "\t\t\tfake_pathname = susfs_get_redirected_path(filp->f_inode->i_ino);"
            print "\t\t\tif (!IS_ERR(fake_pathname)) {"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tfilp_close(filp, NULL);"
            printf "\t\t\t\t%s\n", set_nd
            print "\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_RCU);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ECHILD)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ESTALE)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_REVAL);"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tputname(fake_pathname);"
            print "\t\t\t\treturn filp;"
            print "\t\t\t}"
            print "\t\t}"
            print "\t}"
            print "#endif"
        } else {
            printf "%s", block
        }
        next
    }
    { print }
    ' "$NAMEI" > "$NAMEI.tmp" && mv "$NAMEI.tmp" "$NAMEI"
    ((inject_count++)) || true
fi

if ! grep -q 'susfs_get_redirected_path_all' "$NAMEI"; then
    echo "FATAL: susfs_get_redirected_path_all not found after injection"
    exit 1
fi
if ! grep -q 'AS_FLAGS_OPEN_REDIRECT_ALL' "$NAMEI"; then
    echo "FATAL: AS_FLAGS_OPEN_REDIRECT_ALL not found after injection"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
