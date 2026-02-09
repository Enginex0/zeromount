#!/bin/bash
# inject-susfs-open-redirect-all.sh
# Injects CMD_SUSFS_ADD_OPEN_REDIRECT_ALL (0x555c1), AS_FLAGS_OPEN_REDIRECT_ALL,
# BIT_OPEN_REDIRECT_ALL, st_susfs_open_redirect_all_hlist struct, hash table,
# and 3 functions into upstream SUSFS source.
#
# Usage: ./inject-susfs-open-redirect-all.sh <SUSFS_KERNEL_PATCHES_DIR>

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

echo "=== inject-susfs-open-redirect-all ==="
inject_count=0

# --- 1. CMD code in susfs_def.h ---
if grep -q 'CMD_SUSFS_ADD_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "[=] CMD_SUSFS_ADD_OPEN_REDIRECT_ALL already present in susfs_def.h"
else
    echo "[+] Injecting CMD_SUSFS_ADD_OPEN_REDIRECT_ALL into susfs_def.h"
    sed -i '/CMD_SUSFS_ADD_OPEN_REDIRECT 0x555c0/a #define CMD_SUSFS_ADD_OPEN_REDIRECT_ALL 0x555c1' "$SUSFS_DEF_H"
    ((inject_count++)) || true
fi

# --- 2. AS_FLAGS and BIT in susfs_def.h ---
if grep -q 'AS_FLAGS_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "[=] AS_FLAGS_OPEN_REDIRECT_ALL already present in susfs_def.h"
else
    echo "[+] Injecting AS_FLAGS_OPEN_REDIRECT_ALL into susfs_def.h"
    sed -i '/^#define AS_FLAGS_SUS_MAP/a #define AS_FLAGS_OPEN_REDIRECT_ALL 40' "$SUSFS_DEF_H"
    ((inject_count++)) || true
fi

if grep -q 'BIT_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "[=] BIT_OPEN_REDIRECT_ALL already present in susfs_def.h"
else
    echo "[+] Injecting BIT_OPEN_REDIRECT_ALL into susfs_def.h"
    sed -i '/^#define BIT_SUS_MAPS/a #define BIT_OPEN_REDIRECT_ALL BIT(40)' "$SUSFS_DEF_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'CMD_SUSFS_ADD_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "FATAL: CMD_SUSFS_ADD_OPEN_REDIRECT_ALL injection failed"
    exit 1
fi
if ! grep -q 'AS_FLAGS_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "FATAL: AS_FLAGS_OPEN_REDIRECT_ALL injection failed"
    exit 1
fi
if ! grep -q 'BIT_OPEN_REDIRECT_ALL' "$SUSFS_DEF_H"; then
    echo "FATAL: BIT_OPEN_REDIRECT_ALL injection failed"
    exit 1
fi

# --- 3. Struct in susfs.h ---
if grep -q 'st_susfs_open_redirect_all_hlist' "$SUSFS_H"; then
    echo "[=] st_susfs_open_redirect_all_hlist already present in susfs.h"
else
    echo "[+] Injecting st_susfs_open_redirect_all_hlist struct into susfs.h"
    # Anchor: after the closing }; of st_susfs_open_redirect_hlist
    sed -i '/^struct st_susfs_open_redirect_hlist {/,/^};/ {
        /^};/ a\
\
struct st_susfs_open_redirect_all_hlist {\
\tunsigned long                           target_ino;\
\tchar                                    target_pathname[SUSFS_MAX_LEN_PATHNAME];\
\tchar                                    redirected_pathname[SUSFS_MAX_LEN_PATHNAME];\
\tstruct hlist_node                       node;\
};
    }' "$SUSFS_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'st_susfs_open_redirect_all_hlist' "$SUSFS_H"; then
    echo "FATAL: st_susfs_open_redirect_all_hlist struct injection failed"
    exit 1
fi

# --- 4. Function declarations in susfs.h ---
if grep -q 'susfs_add_open_redirect_all' "$SUSFS_H"; then
    echo "[=] susfs_add_open_redirect_all declaration already present in susfs.h"
else
    echo "[+] Injecting open_redirect_all declarations into susfs.h"
    sed -i '/void susfs_add_open_redirect(void __user \*\*user_info);/a void susfs_add_open_redirect_all(void __user **user_info);\nstruct filename* susfs_get_redirected_path_all(unsigned long ino);' "$SUSFS_H"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'susfs_add_open_redirect_all' "$SUSFS_H"; then
    echo "FATAL: susfs_add_open_redirect_all declaration injection failed"
    exit 1
fi

# --- 5. Hash table + spinlock in susfs.c ---
if grep -q 'OPEN_REDIRECT_ALL_HLIST' "$SUSFS_C"; then
    echo "[=] OPEN_REDIRECT_ALL_HLIST already present in susfs.c"
else
    echo "[+] Injecting OPEN_REDIRECT_ALL hash table into susfs.c"
    sed -i '/DEFINE_HASHTABLE(OPEN_REDIRECT_HLIST, 10);/a static DEFINE_SPINLOCK(susfs_spin_lock_open_redirect_all);\nstatic DEFINE_HASHTABLE(OPEN_REDIRECT_ALL_HLIST, 10);' "$SUSFS_C"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'OPEN_REDIRECT_ALL_HLIST' "$SUSFS_C"; then
    echo "FATAL: OPEN_REDIRECT_ALL_HLIST injection failed"
    exit 1
fi

# --- 6. Three functions in susfs.c ---
if grep -q 'susfs_update_open_redirect_all_inode' "$SUSFS_C"; then
    echo "[=] open_redirect_all functions already present in susfs.c"
else
    echo "[+] Injecting open_redirect_all functions into susfs.c"
    # Anchor: after susfs_add_open_redirect() function
    # Find CMD_SUSFS_ADD_OPEN_REDIRECT -> ret log line and its closing brace
    sed -i '/CMD_SUSFS_ADD_OPEN_REDIRECT -> ret/,/^}/ {
        /^}/ a\
\
static int susfs_update_open_redirect_all_inode(struct st_susfs_open_redirect_all_hlist *new_entry) {\
\tstruct path path_target;\
\tstruct inode *inode_target;\
\tint err = 0;\
\n\terr = kern_path(new_entry->target_pathname, LOOKUP_FOLLOW, &path_target);\
\tif (err) {\
\t\tSUSFS_LOGE("Failed opening file '"'"'%s'"'"'\\n", new_entry->target_pathname);\
\t\treturn err;\
\t}\
\n\tinode_target = d_inode(path_target.dentry);\
\tif (!inode_target) {\
\t\tSUSFS_LOGE("inode_target is NULL\\n");\
\t\terr = -EINVAL;\
\t\tgoto out_path_put_target;\
\t}\
\n\tspin_lock(&inode_target->i_lock);\
\tset_bit(AS_FLAGS_OPEN_REDIRECT_ALL, &inode_target->i_mapping->flags);\
\tspin_unlock(&inode_target->i_lock);\
\nout_path_put_target:\
\tpath_put(&path_target);\
\treturn err;\
}\
\
void susfs_add_open_redirect_all(void __user **user_info) {\
\tstruct st_susfs_open_redirect info = {0};\
\tstruct st_susfs_open_redirect_all_hlist *new_entry;\
\n\tif (copy_from_user(&info, (struct st_susfs_open_redirect __user*)*user_info, sizeof(info))) {\
\t\tinfo.err = -EFAULT;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tnew_entry = kmalloc(sizeof(struct st_susfs_open_redirect_all_hlist), GFP_KERNEL);\
\tif (!new_entry) {\
\t\tinfo.err = -ENOMEM;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tnew_entry->target_ino = info.target_ino;\
\tstrncpy(new_entry->target_pathname, info.target_pathname, SUSFS_MAX_LEN_PATHNAME-1);\
\tnew_entry->target_pathname[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';\
\tstrncpy(new_entry->redirected_pathname, info.redirected_pathname, SUSFS_MAX_LEN_PATHNAME-1);\
\tnew_entry->redirected_pathname[SUSFS_MAX_LEN_PATHNAME-1] = '"'"'\\0'"'"';\
\tif (susfs_update_open_redirect_all_inode(new_entry)) {\
\t\tSUSFS_LOGE("failed adding path '"'"'%s'"'"' to OPEN_REDIRECT_ALL_HLIST\\n", new_entry->target_pathname);\
\t\tkfree(new_entry);\
\t\tinfo.err = -EINVAL;\
\t\tgoto out_copy_to_user;\
\t}\
\n\tspin_lock(&susfs_spin_lock_open_redirect_all);\
\thash_add(OPEN_REDIRECT_ALL_HLIST, &new_entry->node, info.target_ino);\
\tspin_unlock(&susfs_spin_lock_open_redirect_all);\
\tSUSFS_LOGI("target_ino: '"'"'%lu'"'"', target_pathname: '"'"'%s'"'"' redirected_pathname: '"'"'%s'"'"', is successfully added to OPEN_REDIRECT_ALL_HLIST\\n",\
\t\t\tnew_entry->target_ino, new_entry->target_pathname, new_entry->redirected_pathname);\
\tinfo.err = 0;\
out_copy_to_user:\
\tif (copy_to_user(&((struct st_susfs_open_redirect __user*)*user_info)->err, &info.err, sizeof(info.err))) {\
\t\tinfo.err = -EFAULT;\
\t}\
\tSUSFS_LOGI("CMD_SUSFS_ADD_OPEN_REDIRECT_ALL -> ret: %d\\n", info.err);\
}\
\
struct filename* susfs_get_redirected_path_all(unsigned long ino) {\
\tstruct st_susfs_open_redirect_all_hlist *entry;\
\tstruct filename *result = ERR_PTR(-ENOENT);\
\n\tspin_lock(&susfs_spin_lock_open_redirect_all);\
\thash_for_each_possible(OPEN_REDIRECT_ALL_HLIST, entry, node, ino) {\
\t\tif (entry->target_ino == ino) {\
\t\t\tSUSFS_LOGI("Redirect_all for ino: %lu\\n", ino);\
\t\t\tresult = getname_kernel(entry->redirected_pathname);\
\t\t\tbreak;\
\t\t}\
\t}\
\tspin_unlock(&susfs_spin_lock_open_redirect_all);\
\treturn result;\
}
    }' "$SUSFS_C"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'susfs_add_open_redirect_all' "$SUSFS_C"; then
    echo "FATAL: susfs_add_open_redirect_all function injection failed"
    exit 1
fi
if ! grep -q 'susfs_get_redirected_path_all' "$SUSFS_C"; then
    echo "FATAL: susfs_get_redirected_path_all function injection failed"
    exit 1
fi
if ! grep -q 'susfs_update_open_redirect_all_inode' "$SUSFS_C"; then
    echo "FATAL: susfs_update_open_redirect_all_inode function injection failed"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
