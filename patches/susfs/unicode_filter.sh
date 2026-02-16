#!/bin/bash
# CVE-2024-43093 unicode path traversal mitigation
# Injects susfs_check_unicode_bypass() at VFS entry points

set -e
cd "${1:-.}" || exit 1

KV="${KERNEL_VERSION:-}"
if [[ -z "$KV" ]]; then
    echo "FATAL: KERNEL_VERSION not set"; exit 1
fi

# 5.15 has split API: namei.c functions take struct filename * (6.1-era)
# but vfs_statx still takes const char __user * (5.10-era)
# NOTE: 5.4 is GKI but upstream SUSFS has no 5.4 branch and the build workflow
# does not offer 5.4 as a kernel_version choice. The branch is kept defensively
# in case SUSFS adds 5.4 support in the future.
case "$KV" in
    5.10|5.4)      USE_UPTR=false ;;
    5.15)          USE_UPTR=true; STAT_UPTR=false ;;
    6.1|6.6|6.12)  USE_UPTR=true  ;;
    *)              echo "FATAL: Unsupported kernel: $KV"; exit 1 ;;
esac
STAT_UPTR="${STAT_UPTR:-$USE_UPTR}"

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

    # do_mkdirat — 5.10: pathname (const char __user *), 5.15+: name (struct filename *)
    if $USE_UPTR; then
        sed -i '/unsigned int lookup_flags = LOOKUP_DIRECTORY;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(name->uptr)) {\
		return -ENOENT;\
	}\
#endif' "$f"
    else
        sed -i '/unsigned int lookup_flags = LOOKUP_DIRECTORY;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -ENOENT;\
	}\
#endif' "$f"
    fi

    # unlinkat — SYSCALL wrapper, always const char __user *
    sed -i '/if ((flag & ~AT_REMOVEDIR) != 0)/,/return -EINVAL;/{
        /return -EINVAL;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -ENOENT;\
	}\
#endif
    }' "$f"

    # do_symlinkat — 5.10: static long, newname (const char __user *); 5.15+: int, to (struct filename *)
    if $USE_UPTR; then
        sed -i '/^int do_symlinkat/,/unsigned int lookup_flags = 0;/{
            /unsigned int lookup_flags = 0;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(to->uptr)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    else
        sed -i '/^static long do_symlinkat/,/unsigned int lookup_flags = 0;/{
            /unsigned int lookup_flags = 0;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(newname)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    fi

    # do_linkat — 5.10: static int, newname (const char __user *); 5.15+: int, new (struct filename *)
    if $USE_UPTR; then
        sed -i '/^int do_linkat/,/int error;/{
            /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(new->uptr)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    else
        sed -i '/^static int do_linkat/,/int error;/{
            /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(newname)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    fi

    # renameat2 — SYSCALL wrapper, always const char __user *
    # SYSCALL_DEFINE5(renameat2,...) spans multiple lines in all kernel versions,
    # so anchor on just the first line, then match the opening brace
    sed -i '/^SYSCALL_DEFINE5(renameat2,/,/^{$/{
        /^{$/a\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(oldname) ||\
	    susfs_check_unicode_bypass(newname)) {\
		return -ENOENT;\
	}\
#endif
    }' "$f"
}

patch_open() {
    local f="fs/open.c"
    [ -f "$f" ] && grep -q "CONFIG_KSU_SUSFS_UNICODE_FILTER" "$f" && return

    echo "[+] $f"

    inject_susfs_include '#include <linux\/compat.h>' "$f"

    # do_sys_openat2 — always const char __user *filename
    sed -i '/^static long do_sys_openat2/,/struct filename \*tmp;/{
        /struct filename \*tmp;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename)) {\
		return -ENOENT;\
	}\
#endif
    }' "$f"

    # do_faccessat — always const char __user *filename
    sed -i '/^static long do_faccessat/,/const struct cred \*old_cred/{
        /const struct cred \*old_cred/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename)) {\
		return -ENOENT;\
	}\
#endif
    }' "$f"
}

patch_stat() {
    local f="fs/stat.c"
    [ -f "$f" ] && grep -q "CONFIG_KSU_SUSFS_UNICODE_FILTER" "$f" && return

    echo "[+] $f"

    inject_susfs_include '#include <linux\/compat.h>' "$f"

    # vfs_statx — 5.10/5.15: filename (const char __user *), 6.1+: filename (struct filename *)
    if $STAT_UPTR; then
        sed -i '/^static int vfs_statx/,/int error;/{
            /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename->uptr)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    else
        sed -i '/^static int vfs_statx/,/int error;/{
            /int error;$/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(filename)) {\
		return -ENOENT;\
	}\
#endif
        }' "$f"
    fi

    # do_readlinkat — always const char __user *pathname
    sed -i '/unsigned int lookup_flags = LOOKUP_EMPTY;/a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
	if (susfs_check_unicode_bypass(pathname)) {\
		return -ENOENT;\
	}\
#endif' "$f"
}

patch_namei
patch_open
patch_stat

echo "[+] Unicode filter applied (kernel $KV, uptr=$USE_UPTR)"
