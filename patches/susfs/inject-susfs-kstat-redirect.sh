#!/bin/bash
# inject-susfs-kstat-redirect.sh
# Injects CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT (0x55573), the st_susfs_sus_kstat_redirect
# struct, and susfs_add_sus_kstat_redirect() function into upstream SUSFS source.
#
# Usage: ./inject-susfs-kstat-redirect.sh <SUSFS_KERNEL_PATCHES_DIR>

set -e

SUSFS_DIR="$1"

if [ -z "$SUSFS_DIR" ]; then
    echo "Usage: $0 <SUSFS_KERNEL_PATCHES_DIR>"
    exit 1
fi

SUSFS_DEF_H="$SUSFS_DIR/include/linux/susfs_def.h"
SUSFS_H="$SUSFS_DIR/include/linux/susfs.h"
SUSFS_C="$SUSFS_DIR/fs/susfs.c"

for f in "$SUSFS_DEF_H" "$SUSFS_H" "$SUSFS_C"; do
    if [ ! -f "$f" ]; then
        echo "FATAL: missing $f"
        exit 1
    fi
done

echo "=== inject-susfs-kstat-redirect ==="
inject_count=0

# --- 1. CMD code in susfs_def.h ---
if grep -q 'CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT' "$SUSFS_DEF_H"; then
    echo "[=] CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT already present in susfs_def.h"
else
    echo "[+] Injecting CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT into susfs_def.h"
    sed -i '/CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY/a #define CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT 0x55573' "$SUSFS_DEF_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT' "$SUSFS_DEF_H"; then
    echo "FATAL: CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT injection failed"
    exit 1
fi

# --- 2. Struct in susfs.h ---
if grep -q 'st_susfs_sus_kstat_redirect' "$SUSFS_H"; then
    echo "[=] st_susfs_sus_kstat_redirect already present in susfs.h"
else
    echo "[+] Injecting st_susfs_sus_kstat_redirect struct into susfs.h"
    # Anchor: after the closing }; of st_susfs_sus_kstat_hlist struct
    sed -i '/^struct st_susfs_sus_kstat_hlist {/,/^};/ {
        /^};/ a\
\
struct st_susfs_sus_kstat_redirect {\
\tchar                                    virtual_pathname[SUSFS_MAX_LEN_PATHNAME];\
\tchar                                    real_pathname[SUSFS_MAX_LEN_PATHNAME];\
\tunsigned long                           spoofed_ino;\
\tunsigned long                           spoofed_dev;\
\tunsigned int                            spoofed_nlink;\
\tlong long                               spoofed_size;\
\tlong                                    spoofed_atime_tv_sec;\
\tlong                                    spoofed_mtime_tv_sec;\
\tlong                                    spoofed_ctime_tv_sec;\
\tlong                                    spoofed_atime_tv_nsec;\
\tlong                                    spoofed_mtime_tv_nsec;\
\tlong                                    spoofed_ctime_tv_nsec;\
\tunsigned long                           spoofed_blksize;\
\tunsigned long long                      spoofed_blocks;\
\tint                                     err;\
};
    }' "$SUSFS_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'st_susfs_sus_kstat_redirect' "$SUSFS_H"; then
    echo "FATAL: st_susfs_sus_kstat_redirect struct injection failed"
    exit 1
fi

# --- 3. Function declaration in susfs.h ---
if grep -q 'susfs_add_sus_kstat_redirect' "$SUSFS_H"; then
    echo "[=] susfs_add_sus_kstat_redirect declaration already present in susfs.h"
else
    echo "[+] Injecting susfs_add_sus_kstat_redirect declaration into susfs.h"
    # Anchor: after susfs_add_sus_kstat declaration
    sed -i '/void susfs_add_sus_kstat(void __user \*\*user_info);/a void susfs_add_sus_kstat_redirect(void __user **user_info);' "$SUSFS_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'susfs_add_sus_kstat_redirect' "$SUSFS_H"; then
    echo "FATAL: susfs_add_sus_kstat_redirect declaration injection failed"
    exit 1
fi

# --- 4. Function body in susfs.c ---
if grep -q 'susfs_add_sus_kstat_redirect' "$SUSFS_C"; then
    echo "[=] susfs_add_sus_kstat_redirect function already present in susfs.c"
else
    echo "[+] Injecting susfs_add_sus_kstat_redirect function into susfs.c"
    # Anchor: after the closing brace of susfs_add_sus_kstat() function.
    # We find the SUSFS_LOGI for CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY and inject after
    # the next closing brace (end of susfs_add_sus_kstat).
    # Strategy: find the unique log line at end of susfs_add_sus_kstat, then its closing }
    sed -i '/CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY -> ret/,/^}/ {
        /^}/ a\
\
void susfs_add_sus_kstat_redirect(void __user **user_info) {\
\tstruct st_susfs_sus_kstat_redirect info = {0};\
\tstruct st_susfs_sus_kstat_hlist *new_entry = NULL;\
\tstruct st_susfs_sus_kstat_hlist *virtual_entry = NULL;\
\tstruct path p_real;\
\tstruct path p_virtual;\
\tstruct inode *inode_real = NULL;\
\tstruct inode *inode_virtual = NULL;\
\tunsigned long virtual_ino = 0;\
\tbool virtual_path_resolved = false;\
\n\tif (copy_from_user(&info, (struct st_susfs_sus_kstat_redirect __user*)*user_info, sizeof(info))) {\
\t\tinfo.err = -EFAULT;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tif (strlen(info.virtual_pathname) == 0 || strlen(info.real_pathname) == 0) {\
\t\tinfo.err = -EINVAL;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tnew_entry = kzalloc(sizeof(struct st_susfs_sus_kstat_hlist), GFP_KERNEL);\
\tif (!new_entry) {\
\t\tinfo.err = -ENOMEM;\
\t\tgoto out_copy_to_user;\
\t}\
\n#if defined(__ARCH_WANT_STAT64) || defined(__ARCH_WANT_COMPAT_STAT64)\
#ifdef CONFIG_MIPS\
\tinfo.spoofed_dev = new_decode_dev(info.spoofed_dev);\
#else\
\tinfo.spoofed_dev = huge_decode_dev(info.spoofed_dev);\
#endif /* CONFIG_MIPS */\
#else\
\tinfo.spoofed_dev = old_decode_dev(info.spoofed_dev);\
#endif /* defined(__ARCH_WANT_STAT64) || defined(__ARCH_WANT_COMPAT_STAT64) */\
\n\tSUSFS_LOGI("kstat_redirect: ENTRY vpath='"'"'%s'"'"' rpath='"'"'%s'"'"'\\n",\
\t           info.virtual_pathname, info.real_pathname);\
\tif (!kern_path(info.virtual_pathname, 0, &p_virtual)) {\
\t\tinode_virtual = d_inode(p_virtual.dentry);\
\t\tif (inode_virtual) {\
\t\t\tvirtual_ino = inode_virtual->i_ino;\
\t\t\tif (!(inode_virtual->i_mapping->flags & BIT_SUS_KSTAT)) {\
\t\t\t\tspin_lock(&inode_virtual->i_lock);\
\t\t\t\tset_bit(AS_FLAGS_SUS_KSTAT, &inode_virtual->i_mapping->flags);\
\t\t\t\tspin_unlock(&inode_virtual->i_lock);\
\t\t\t}\
\t\t\tvirtual_path_resolved = true;\
\t\t\tSUSFS_LOGI("kstat_redirect: VPATH_OK ino=%lu flagged='"'"'%s'"'"'\\n",\
\t\t\t           virtual_ino, info.virtual_pathname);\
\t\t}\
\t\tpath_put(&p_virtual);\
\t} else {\
\t\tSUSFS_LOGI("kstat_redirect: VPATH_MISSING '"'"'%s'"'"' (new file)\\n",\
\t\t           info.virtual_pathname);\
\t}\
\n\tinfo.err = kern_path(info.real_pathname, 0, &p_real);\
\tif (info.err) {\
\t\tSUSFS_LOGE("Failed opening real file '"'"'%s'"'"'\\n", info.real_pathname);\
\t\tkfree(new_entry);\
\t\tgoto out_copy_to_user;\
\t}\
\n\tinode_real = d_inode(p_real.dentry);\
\tif (!inode_real) {\
\t\tpath_put(&p_real);\
\t\tkfree(new_entry);\
\t\tSUSFS_LOGE("inode is NULL for real file '"'"'%s'"'"'\\n", info.real_pathname);\
\t\tinfo.err = -EINVAL;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tif (!(inode_real->i_mapping->flags & BIT_SUS_KSTAT)) {\
\t\tspin_lock(&inode_real->i_lock);\
\t\tset_bit(AS_FLAGS_SUS_KSTAT, &inode_real->i_mapping->flags);\
\t\tspin_unlock(&inode_real->i_lock);\
\t}\
\n\tnew_entry->target_ino = inode_real->i_ino;\
\tnew_entry->info.is_statically = 0;\
\tnew_entry->info.target_ino = inode_real->i_ino;\
\tstrncpy(new_entry->info.target_pathname, info.virtual_pathname, SUSFS_MAX_LEN_PATHNAME - 1);\
\tnew_entry->info.target_pathname[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';\
\tnew_entry->info.spoofed_ino = info.spoofed_ino;\
\tnew_entry->info.spoofed_dev = info.spoofed_dev;\
\tnew_entry->info.spoofed_nlink = info.spoofed_nlink;\
\tnew_entry->info.spoofed_size = info.spoofed_size;\
\tnew_entry->info.spoofed_atime_tv_sec = info.spoofed_atime_tv_sec;\
\tnew_entry->info.spoofed_mtime_tv_sec = info.spoofed_mtime_tv_sec;\
\tnew_entry->info.spoofed_ctime_tv_sec = info.spoofed_ctime_tv_sec;\
\tnew_entry->info.spoofed_atime_tv_nsec = info.spoofed_atime_tv_nsec;\
\tnew_entry->info.spoofed_mtime_tv_nsec = info.spoofed_mtime_tv_nsec;\
\tnew_entry->info.spoofed_ctime_tv_nsec = info.spoofed_ctime_tv_nsec;\
\tnew_entry->info.spoofed_blksize = info.spoofed_blksize;\
\tnew_entry->info.spoofed_blocks = info.spoofed_blocks;\
\n\tpath_put(&p_real);\
\n\tif (virtual_path_resolved && virtual_ino != 0 && virtual_ino != new_entry->target_ino) {\
\t\tvirtual_entry = kzalloc(sizeof(struct st_susfs_sus_kstat_hlist), GFP_KERNEL);\
\t\tif (!virtual_entry) {\
\t\t\tSUSFS_LOGE("kstat_redirect: ALLOC_FAIL virtual_entry, aborting\\n");\
\t\t\tkfree(new_entry);\
\t\t\tinfo.err = -ENOMEM;\
\t\t\tgoto out_copy_to_user;\
\t\t}\
\t\tmemcpy(&virtual_entry->info, &new_entry->info, sizeof(new_entry->info));\
\t\tvirtual_entry->target_ino = virtual_ino;\
\t\tvirtual_entry->info.target_ino = virtual_ino;\
\t}\
\n\tspin_lock(&susfs_spin_lock_sus_kstat);\
\thash_add(SUS_KSTAT_HLIST, &new_entry->node, new_entry->target_ino);\
\tif (virtual_entry) {\
\t\thash_add(SUS_KSTAT_HLIST, &virtual_entry->node, virtual_ino);\
\t}\
\tspin_unlock(&susfs_spin_lock_sus_kstat);\
\n\tSUSFS_LOGI("kstat_redirect: RPATH_OK ino=%lu dev=%lu '"'"'%s'"'"'\\n",\
\t           new_entry->target_ino, new_entry->info.spoofed_dev, info.real_pathname);\
\tif (virtual_entry) {\
\t\tSUSFS_LOGI("kstat_redirect: DUAL_INODE vino=%lu rino=%lu '"'"'%s'"'"'\\n",\
\t\t           virtual_ino, new_entry->target_ino, info.virtual_pathname);\
\t} else if (virtual_path_resolved && virtual_ino == new_entry->target_ino) {\
\t\tSUSFS_LOGI("kstat_redirect: SAME_INODE ino=%lu '"'"'%s'"'"'\\n",\
\t\t           virtual_ino, info.virtual_pathname);\
\t}\
\n#if LINUX_VERSION_CODE >= KERNEL_VERSION(6, 1, 0)\
\tSUSFS_LOGI("redirect: virtual: '"'"'%s'"'"', real: '"'"'%s'"'"', target_ino: '"'"'%lu'"'"', spoofed_ino: '"'"'%lu'"'"', spoofed_dev: '"'"'%lu'"'"', spoofed_nlink: '"'"'%u'"'"', spoofed_size: '"'"'%llu'"'"', spoofed_atime_tv_sec: '"'"'%ld'"'"', spoofed_mtime_tv_sec: '"'"'%ld'"'"', spoofed_ctime_tv_sec: '"'"'%ld'"'"', spoofed_atime_tv_nsec: '"'"'%ld'"'"', spoofed_mtime_tv_nsec: '"'"'%ld'"'"', spoofed_ctime_tv_nsec: '"'"'%ld'"'"', spoofed_blksize: '"'"'%lu'"'"', spoofed_blocks: '"'"'%llu'"'"', added to SUS_KSTAT_HLIST\\n",\
\t\t\tinfo.virtual_pathname, info.real_pathname, new_entry->target_ino,\
\t\t\tnew_entry->info.spoofed_ino, new_entry->info.spoofed_dev,\
\t\t\tnew_entry->info.spoofed_nlink, new_entry->info.spoofed_size,\
\t\t\tnew_entry->info.spoofed_atime_tv_sec, new_entry->info.spoofed_mtime_tv_sec, new_entry->info.spoofed_ctime_tv_sec,\
\t\t\tnew_entry->info.spoofed_atime_tv_nsec, new_entry->info.spoofed_mtime_tv_nsec, new_entry->info.spoofed_ctime_tv_nsec,\
\t\t\tnew_entry->info.spoofed_blksize, new_entry->info.spoofed_blocks);\
#else\
\tSUSFS_LOGI("redirect: virtual: '"'"'%s'"'"', real: '"'"'%s'"'"', target_ino: '"'"'%lu'"'"', spoofed_ino: '"'"'%lu'"'"', spoofed_dev: '"'"'%lu'"'"', spoofed_nlink: '"'"'%u'"'"', spoofed_size: '"'"'%llu'"'"', spoofed_atime_tv_sec: '"'"'%ld'"'"', spoofed_mtime_tv_sec: '"'"'%ld'"'"', spoofed_ctime_tv_sec: '"'"'%ld'"'"', spoofed_atime_tv_nsec: '"'"'%ld'"'"', spoofed_mtime_tv_nsec: '"'"'%ld'"'"', spoofed_ctime_tv_nsec: '"'"'%ld'"'"', spoofed_blksize: '"'"'%lu'"'"', spoofed_blocks: '"'"'%llu'"'"', added to SUS_KSTAT_HLIST\\n",\
\t\t\tinfo.virtual_pathname, info.real_pathname, new_entry->target_ino,\
\t\t\tnew_entry->info.spoofed_ino, new_entry->info.spoofed_dev,\
\t\t\tnew_entry->info.spoofed_nlink, new_entry->info.spoofed_size,\
\t\t\tnew_entry->info.spoofed_atime_tv_sec, new_entry->info.spoofed_mtime_tv_sec, new_entry->info.spoofed_ctime_tv_sec,\
\t\t\tnew_entry->info.spoofed_atime_tv_nsec, new_entry->info.spoofed_mtime_tv_nsec, new_entry->info.spoofed_ctime_tv_nsec,\
\t\t\tnew_entry->info.spoofed_blksize, new_entry->info.spoofed_blocks);\
#endif\
\n\tinfo.err = 0;\
out_copy_to_user:\
\tif (copy_to_user(&((struct st_susfs_sus_kstat_redirect __user*)*user_info)->err, &info.err, sizeof(info.err))) {\
\t\tinfo.err = -EFAULT;\
\t}\
\tSUSFS_LOGI("kstat_redirect: EXIT ret=%d vpath='"'"'%s'"'"'\\n", info.err, info.virtual_pathname);\
}
    }' "$SUSFS_C"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'susfs_add_sus_kstat_redirect' "$SUSFS_C"; then
    echo "FATAL: susfs_add_sus_kstat_redirect function injection failed"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
