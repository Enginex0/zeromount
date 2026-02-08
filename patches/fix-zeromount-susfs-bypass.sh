#!/bin/bash
# fix-zeromount-susfs-bypass.sh
# Adds susfs_is_current_proc_umounted() checks to ZeroMount kernel functions
# to prevent detection apps from seeing VFS redirections

set -e

ZEROMOUNT_C="${1:-fs/zeromount.c}"

if [[ ! -f "$ZEROMOUNT_C" ]]; then
    echo "[-] File not found: $ZEROMOUNT_C"
    exit 1
fi

echo "[*] Fixing ZeroMount SUSFS bypass in: $ZEROMOUNT_C"

if grep -q "susfs_is_current_proc_umounted" "$ZEROMOUNT_C"; then
    echo "[*] SUSFS bypass checks already present"
    exit 0
fi

# Add susfs.h include after zeromount.h
if ! grep -q '#include <linux/susfs.h>' "$ZEROMOUNT_C"; then
    echo "[+] Adding susfs.h include"
    sed -i '/#include <linux\/zeromount.h>/a\
#ifdef CONFIG_KSU_SUSFS\
#include <linux/susfs.h>\
#endif' "$ZEROMOUNT_C"
fi

# Helper: bypass check for umounted processes
BYPASS_CHECK='#ifdef CONFIG_KSU_SUSFS
	if (susfs_is_current_proc_umounted())
		return
#endif'

# Fix zeromount_is_uid_blocked - add early return for umounted
echo "[+] Patching zeromount_is_uid_blocked"
sed -i '/^static bool zeromount_is_uid_blocked(uid_t uid) {$/,/^}$/{
    /if (ZEROMOUNT_DISABLED()) return false;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted()) return true;\
#endif
}' "$ZEROMOUNT_C"

# Fix zeromount_is_traversal_allowed - block umounted processes
echo "[+] Patching zeromount_is_traversal_allowed"
sed -i '/^bool zeromount_is_traversal_allowed(struct inode \*inode, int mask) {$/,/^}$/{
    /if (!inode || ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val)) return false;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted()) return false;\
#endif
}' "$ZEROMOUNT_C"

# Fix zeromount_is_injected_file - hide from umounted
echo "[+] Patching zeromount_is_injected_file"
sed -i '/^bool zeromount_is_injected_file(struct inode \*inode) {$/,/^}$/{
    /if (!inode || !inode->i_sb || ZEROMOUNT_DISABLED())$/,/return false;/{
        /return false;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return false;\
#endif
    }
}' "$ZEROMOUNT_C"

# Fix zeromount_resolve_path - no redirection for umounted
echo "[+] Patching zeromount_resolve_path"
sed -i '/^char \*zeromount_resolve_path(const char \*pathname)$/,/^}$/{
    /if (zeromount_is_critical_process())/,/return NULL;/{
        /return NULL;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return NULL;\
#endif
    }
}' "$ZEROMOUNT_C"

# Fix zeromount_getname_hook - no hook for umounted
echo "[+] Patching zeromount_getname_hook"
sed -i '/^struct filename \*zeromount_getname_hook(struct filename \*name)$/,/^}$/{
    /if (zeromount_is_critical_process())/,/return name;/{
        /return name;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return name;\
#endif
    }
}' "$ZEROMOUNT_C"

# Fix zeromount_inject_dents64 - no injection for umounted
echo "[+] Patching zeromount_inject_dents64"
sed -i '/^void zeromount_inject_dents64(struct file \*file/,/^}$/{
    /if (ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val)) return;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted()) return;\
#endif
}' "$ZEROMOUNT_C"

# Fix zeromount_inject_dents - no injection for umounted
echo "[+] Patching zeromount_inject_dents"
sed -i '/^void zeromount_inject_dents(struct file \*file/,/^}$/{
    /if (ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val)) return;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted()) return;\
#endif
}' "$ZEROMOUNT_C"

# Fix zeromount_spoof_statfs - no spoofing for umounted
echo "[+] Patching zeromount_spoof_statfs"
sed -i '/^int zeromount_spoof_statfs(const char __user \*pathname/,/^}$/{
    /if (ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val))$/,/return 0;/{
        /return 0;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return 0;\
#endif
    }
}' "$ZEROMOUNT_C"

# Fix zeromount_spoof_xattr - no spoofing for umounted
echo "[+] Patching zeromount_spoof_xattr"
sed -i '/^ssize_t zeromount_spoof_xattr(struct dentry \*dentry/,/^}$/{
    /if (ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val))$/,/return -EOPNOTSUPP;/{
        /return -EOPNOTSUPP;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return -EOPNOTSUPP;\
#endif
    }
}' "$ZEROMOUNT_C"

# Fix zeromount_get_virtual_path_for_inode - hide from umounted
echo "[+] Patching zeromount_get_virtual_path_for_inode"
sed -i '/^char \*zeromount_get_virtual_path_for_inode(struct inode \*inode) {$/,/^}$/{
    /if (zeromount_is_uid_blocked(current_uid().val))$/,/return NULL;/{
        /return NULL;/a\
#ifdef CONFIG_KSU_SUSFS\
    if (susfs_is_current_proc_umounted())\
        return NULL;\
#endif
    }
}' "$ZEROMOUNT_C"

# Verify
echo "[*] Verifying patches..."
COUNT=$(grep -c "susfs_is_current_proc_umounted" "$ZEROMOUNT_C" || echo "0")
if [[ "$COUNT" -ge 8 ]]; then
    echo "[+] SUCCESS: $COUNT SUSFS bypass checks added"
else
    echo "[-] WARNING: Only $COUNT checks found (expected 8+)"
    echo "[*] Manual verification recommended"
fi

echo "[*] ZeroMount SUSFS bypass fix complete"
