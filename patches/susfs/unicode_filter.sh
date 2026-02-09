#!/bin/bash
# CVE-2024-43093 unicode path traversal mitigation
# Injects susfs_check_unicode_bypass() at VFS entry points

set -e
cd "${1:-.}" || exit 1

inject_susfs_include() {
    sed -i "/$1/a\\
#ifdef CONFIG_KSU_SUSFS\\
#include <linux/susfs.h>\\
#endif" "$2"
}

patch_namei() {
    local f="fs/namei.c"
    [ -f "$f" ] && grep -q "CONFIG_KSU_SUSFS_UNICODE_FILTER" "$f" && return

    echo "[+] $f"

    inject_susfs_include '#include <linux\/uaccess.h>' "$f"

    # do_mkdirat
    sed -i '/unsigned int lookup_flags = LOOKUP_DIRECTORY;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -EPERM;\
	}\
#endif' "$f"

    # unlinkat
    sed -i '/if ((flag & ~AT_REMOVEDIR) != 0)/,/return -EINVAL;/{
        /return -EINVAL;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"

    # do_symlinkat
    sed -i '/^static long do_symlinkat/,/unsigned int lookup_flags = 0;/{
        /unsigned int lookup_flags = 0;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(newname)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"

    # do_linkat
    sed -i '/^static int do_linkat/,/int error;/{
        /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(newname)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"

    # renameat2
    sed -i '/^SYSCALL_DEFINE5(renameat2,.*flags)$/,/^{$/{
        /^{$/a\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(oldname) ||\
	    susfs_check_unicode_bypass(newname)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"
}

patch_open() {
    local f="fs/open.c"
    [ -f "$f" ] && grep -q "CONFIG_KSU_SUSFS_UNICODE_FILTER" "$f" && return

    echo "[+] $f"

    inject_susfs_include '#include <linux\/compat.h>' "$f"

    # do_sys_openat2
    sed -i '/^static long do_sys_openat2/,/struct filename \*tmp;/{
        /struct filename \*tmp;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"
}

patch_stat() {
    local f="fs/stat.c"
    [ -f "$f" ] && grep -q "CONFIG_KSU_SUSFS_UNICODE_FILTER" "$f" && return

    echo "[+] $f"

    inject_susfs_include '#include <linux\/compat.h>' "$f"

    # vfs_statx
    sed -i '/^static int vfs_statx/,/int error;/{
        /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename)) {\
		return -EPERM;\
	}\
#endif
    }' "$f"

    # do_readlinkat
    sed -i '/unsigned int lookup_flags = LOOKUP_EMPTY;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -EPERM;\
	}\
#endif' "$f"
}

patch_namei
patch_open
patch_stat

echo "[+] Unicode filter applied"
