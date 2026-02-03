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

# Use awk to inject the hook after the opening brace and variable declarations of vfs_statx
awk '
BEGIN { state = 0; injected = 0 }

# Match start of vfs_statx function
/^static int vfs_statx\(/ { state = 1 }

# Once inside vfs_statx, look for "int error;" line to inject after declarations
state == 1 && /^[[:space:]]*int error;/ && !injected {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\t/* ZeroMount: Intercept relative paths for injected directories */"
    print "\tif (filename) {"
    print "\t\tchar kname[NAME_MAX + 1];"
    print "\t\tlong copied = strncpy_from_user(kname, filename, sizeof(kname));"
    print "\t\tif (copied > 0 && kname[0] != '"'"'/'"'"') {"
    print "\t\t\tchar *abs_path = zeromount_build_absolute_path(dfd, kname);"
    print "\t\t\tif (abs_path) {"
    print "\t\t\t\tchar *resolved = zeromount_resolve_path(abs_path);"
    print "\t\t\t\tif (resolved) {"
    print "\t\t\t\t\tstruct path zm_path;"
    print "\t\t\t\t\tint zm_ret = kern_path(resolved, (flags & AT_SYMLINK_NOFOLLOW) ? 0 : LOOKUP_FOLLOW, &zm_path);"
    print "\t\t\t\t\tkfree(resolved);"
    print "\t\t\t\t\tkfree(abs_path);"
    print "\t\t\t\t\tif (zm_ret == 0) {"
    print "\t\t\t\t\t\tzm_ret = vfs_getattr(&zm_path, stat, request_mask,"
    print "\t\t\t\t\t\t\t\t(flags & AT_SYMLINK_NOFOLLOW) ? AT_SYMLINK_NOFOLLOW : 0);"
    print "\t\t\t\t\t\tpath_put(&zm_path);"
    print "\t\t\t\t\t\treturn zm_ret;"
    print "\t\t\t\t\t}"
    print "\t\t\t\t} else {"
    print "\t\t\t\t\tkfree(abs_path);"
    print "\t\t\t\t}"
    print "\t\t\t}"
    print "\t\t}"
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

if ! grep -q 'zm_path' "$STAT_FILE"; then
    echo "  [FAIL] zm_path variable not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zm_path variable"
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
