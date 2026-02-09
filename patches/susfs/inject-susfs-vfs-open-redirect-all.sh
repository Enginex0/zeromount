#!/bin/bash
# Adds BIT_OPEN_REDIRECT_ALL two-branch check into do_filp_open.
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

echo "=== inject-susfs-vfs-open-redirect-all ==="
echo "    Target: $NAMEI"
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
if grep -q 'BIT_OPEN_REDIRECT_ALL' "$NAMEI"; then
    echo "[=] BIT_OPEN_REDIRECT_ALL check already present in do_filp_open"
else
    echo "[+] Replacing do_filp_open redirect block with two-branch check"
    # Match the #ifdef CONFIG_KSU_SUSFS_OPEN_REDIRECT block containing BIT_OPEN_REDIRECT
    awk '
    /^#ifdef CONFIG_KSU_SUSFS_OPEN_REDIRECT/ {
        block = $0 "\n"
        is_redirect_block = 0
        while ((getline line) > 0) {
            block = block line "\n"
            if (line ~ /BIT_OPEN_REDIRECT/) is_redirect_block = 1
            if (line ~ /^#endif/) break
        }
        if (is_redirect_block && !already_replaced) {
            already_replaced = 1
            print "#ifdef CONFIG_KSU_SUSFS_OPEN_REDIRECT"
            print "\tif (!IS_ERR(filp)) {"
            print "\t\tunsigned long __susfs_as_flags = filp->f_inode->i_mapping->flags;"
            print "\t\tif (unlikely(__susfs_as_flags & BIT_OPEN_REDIRECT_ALL)) {"
            print "\t\t\tfake_pathname = susfs_get_redirected_path_all(filp->f_inode->i_ino);"
            print "\t\t\tif (!IS_ERR(fake_pathname)) {"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tfilp_close(filp, NULL);"
            print "\t\t\t\tset_nameidata(&nd, dfd, fake_pathname);"
            print "\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_RCU);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ECHILD)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags);"
            print "\t\t\t\tif (unlikely(filp == ERR_PTR(-ESTALE)))"
            print "\t\t\t\t\tfilp = path_openat(&nd, op, flags | LOOKUP_REVAL);"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tputname(fake_pathname);"
            print "\t\t\t\treturn filp;"
            print "\t\t\t}"
            print "\t\t} else if (unlikely(__susfs_as_flags & BIT_OPEN_REDIRECT) && current_uid().val < 2000) {"
            print "\t\t\tfake_pathname = susfs_get_redirected_path(filp->f_inode->i_ino);"
            print "\t\t\tif (!IS_ERR(fake_pathname)) {"
            print "\t\t\t\trestore_nameidata();"
            print "\t\t\t\tfilp_close(filp, NULL);"
            print "\t\t\t\tset_nameidata(&nd, dfd, fake_pathname);"
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
if ! grep -q 'BIT_OPEN_REDIRECT_ALL' "$NAMEI"; then
    echo "FATAL: BIT_OPEN_REDIRECT_ALL not found after injection"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
