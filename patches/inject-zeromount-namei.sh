#!/bin/bash
# inject-zeromount-namei.sh - Inject ZeroMount hooks into fs/namei.c
# Part of ZeroMount VFS-level path redirection subsystem
#
# Usage: ./inject-zeromount-namei.sh <path-to-namei.c>

set -e

TARGET="${1:-fs/namei.c}"
MARKER="CONFIG_ZEROMOUNT"

if [[ ! -f "$TARGET" ]]; then
    echo "Error: File not found: $TARGET"
    exit 1
fi

echo "Injecting ZeroMount hooks into: $TARGET"

if grep -q "$MARKER" "$TARGET"; then
    echo "File already contains ZeroMount hooks ($MARKER found). Skipping."
    exit 0
fi

inject_include() {
    echo "  [1/4] Injecting zeromount.h include..."
    sed -i '/#include "mount.h"/a\
\
#ifdef CONFIG_ZEROMOUNT\
#include <linux/zeromount.h>\
#endif' "$TARGET"

    if ! grep -q "zeromount.h" "$TARGET"; then
        echo "Error: Failed to inject include directive"
        exit 1
    fi
}

inject_getname_hook() {
    echo "  [2/4] Injecting getname_flags() hook..."

    sed -i '/audit_getname(result);/{
N
/\n[[:space:]]*return result;/s/audit_getname(result);/audit_getname(result);\
\
#ifdef CONFIG_ZEROMOUNT\
	if (!IS_ERR(result)) {\
		result = zeromount_getname_hook(result);\
	}\
#endif\
/
}' "$TARGET"

    if ! grep -q "zeromount_getname_hook" "$TARGET"; then
        echo "Error: Failed to inject getname_flags() hook"
        exit 1
    fi
}

inject_generic_permission_hook() {
    echo "  [3/4] Injecting generic_permission() hook..."

    sed -i '/^int generic_permission(struct inode \*inode, int mask)$/,/^}$/{
/^{$/,/int ret;/{
/int ret;/a\
\
#ifdef CONFIG_ZEROMOUNT\
	if (zeromount_is_injected_file(inode)) {\
		if (mask \& MAY_WRITE)\
			return -EACCES;\
		return 0;\
	}\
\
	if (S_ISDIR(inode->i_mode) \&\& zeromount_is_traversal_allowed(inode, mask)) {\
		return 0;\
	}\
#endif
}
}' "$TARGET"

    if ! grep -A20 "^int generic_permission" "$TARGET" | grep -q "zeromount_is_injected_file"; then
        echo "Error: Failed to inject generic_permission() hook"
        exit 1
    fi
}

inject_inode_permission_hook() {
    echo "  [4/4] Injecting inode_permission() hook..."

    sed -i '/^int inode_permission(struct inode \*inode, int mask)$/,/^}$/{
/^{$/,/int retval;/{
/int retval;/a\
\
#ifdef CONFIG_ZEROMOUNT\
	if (zeromount_is_injected_file(inode)) {\
		if (mask \& MAY_WRITE)\
			return -EACCES;\
		return 0;\
	}\
\
	if (S_ISDIR(inode->i_mode) \&\& zeromount_is_traversal_allowed(inode, mask)) {\
		return 0;\
	}\
#endif
}
}' "$TARGET"

    if ! grep -A20 "^int inode_permission" "$TARGET" | grep -q "zeromount_is_injected_file"; then
        echo "Error: Failed to inject inode_permission() hook"
        exit 1
    fi
}

inject_include
inject_getname_hook
inject_generic_permission_hook
inject_inode_permission_hook

echo "ZeroMount namei.c hooks injected successfully."
