#!/bin/bash
# inject-zeromount-stat.sh - Inject ZeroMount hooks into fs/stat.c
#
# Hooks vfs_statx() to intercept relative path stat operations for injected directories.
# When a relative path resolves to a ZeroMount rule, redirect stat to the source file.
#
# Usage: ./inject-zeromount-stat.sh <path-to-stat.c>

set -e

STAT_FILE="${1:-fs/stat.c}"
MARKER="CONFIG_ZEROMOUNT"

if [ ! -f "$STAT_FILE" ]; then
    echo "Error: File not found: $STAT_FILE"
    exit 1
fi

echo "Injecting ZeroMount stat hooks into: $STAT_FILE"

if grep -q "$MARKER" "$STAT_FILE"; then
    echo "File already contains ZeroMount hooks ($MARKER found). Skipping."
    exit 0
fi

if ! grep -q '#include <linux/uaccess.h>' "$STAT_FILE"; then
    echo "Error: Cannot find #include <linux/uaccess.h>"
    exit 1
fi

if ! grep -q 'static int vfs_statx' "$STAT_FILE"; then
    echo "Error: Cannot find 'static int vfs_statx' function"
    exit 1
fi

cp "$STAT_FILE" "${STAT_FILE}.bak"

echo "  [1/2] Injecting zeromount.h include..."
sed -i '/#include <linux\/uaccess.h>/a\
#ifdef CONFIG_ZEROMOUNT\
#include <linux/zeromount.h>\
#endif' "$STAT_FILE"

echo "  [2/2] Injecting hook into vfs_statx..."

# Add forward declaration and inline hook function before vfs_statx
sed -i '/^static int vfs_statx(/i\
#ifdef CONFIG_ZEROMOUNT\
/* ZeroMount stat hook for relative path intercept */\
static inline int zeromount_stat_hook(int dfd, const char __user *filename, \
                                      struct kstat *stat, unsigned int request_mask, \
                                      unsigned int flags) {\
    if (filename) {\
        char kname[NAME_MAX + 1];\
        long copied = strncpy_from_user(kname, filename, sizeof(kname));\
        if (copied > 0 && kname[0] != '"'"'/'"'"') {\
            char *abs_path = zeromount_build_absolute_path(dfd, kname);\
            if (abs_path) {\
                char *resolved = zeromount_resolve_path(abs_path);\
                if (resolved) {\
                    struct path zm_path;\
                    int zm_ret = kern_path(resolved, (flags & AT_SYMLINK_NOFOLLOW) ? 0 : LOOKUP_FOLLOW, &zm_path);\
                    kfree(resolved);\
                    kfree(abs_path);\
                    if (zm_ret == 0) {\
                        zm_ret = vfs_getattr(&zm_path, stat, request_mask,\
                                             (flags & AT_SYMLINK_NOFOLLOW) ? AT_SYMLINK_NOFOLLOW : 0);\
                        path_put(&zm_path);\
                        return zm_ret;\
                    }\
                } else {\
                    kfree(abs_path);\
                }\
            }\
        }\
    }\
    return -ENOENT;\
}\
#endif' "$STAT_FILE"

# Also inject the call into vfs_statx after variable declarations
awk '
BEGIN { state = 0; injected = 0 }

# Match start of vfs_statx function
/^static int vfs_statx\(/ { state = 1 }

# Once inside vfs_statx, look for "int error;" line to inject call
state == 1 && /^[[:space:]]*int error;/ && !injected {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\t/* Try ZeroMount hook for relative paths */"
    print "\tif (filename && dfd != AT_FDCWD) {"
    print "\t\tint zm_ret = zeromount_stat_hook(dfd, filename, stat, request_mask, flags);"
    print "\t\tif (zm_ret != -ENOENT)"
    print "\t\t\treturn zm_ret;"
    print "\t}"
    print "#endif"
    print ""
    injected = 1
    next
}

# Exit state on closing brace at start of line (function end)
state == 1 && /^}$/ { state = 0 }

{ print }

END {
    if (!injected) {
        print "INJECTION_FAILED" > "/dev/stderr"
        exit 1
    }
}
' "$STAT_FILE" > "${STAT_FILE}.tmp"

if [ $? -ne 0 ]; then
    echo "Error: awk injection failed"
    mv "${STAT_FILE}.bak" "$STAT_FILE"
    exit 1
fi

mv "${STAT_FILE}.tmp" "$STAT_FILE"

echo ""
echo "Verifying injection..."

ERRORS=0

if ! grep -q '#include <linux/zeromount.h>' "$STAT_FILE"; then
    echo "  [FAIL] zeromount.h include not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zeromount.h include"
fi

if ! grep -q 'zeromount_stat_hook' "$STAT_FILE"; then
    echo "  [FAIL] zeromount_stat_hook function not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zeromount_stat_hook function"
fi

if ! grep -q 'zeromount_build_absolute_path' "$STAT_FILE"; then
    echo "  [FAIL] zeromount_build_absolute_path call not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zeromount_build_absolute_path call"
fi

if ! grep -q 'zeromount_resolve_path' "$STAT_FILE"; then
    echo "  [FAIL] zeromount_resolve_path call not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zeromount_resolve_path call"
fi

echo ""
if [ "$ERRORS" -eq 0 ]; then
    echo "ZeroMount stat hooks injection complete. Backup at ${STAT_FILE}.bak"
    exit 0
else
    echo "Injection completed with $ERRORS verification failures."
    echo "Review the output and ${STAT_FILE}.bak if needed."
    exit 1
fi
