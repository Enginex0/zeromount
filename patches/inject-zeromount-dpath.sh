#!/bin/bash
# inject-zeromount-dpath.sh - Inject ZeroMount hooks into fs/d_path.c
# Part of ZeroMount VFS-level path redirection subsystem
#
# Usage: ./inject-zeromount-dpath.sh <path-to-d_path.c>

set -e

TARGET="${1:-fs/d_path.c}"
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
    echo "  [1/2] Injecting zeromount.h include..."
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

inject_dpath_hook() {
    echo "  [2/2] Injecting d_path() virtual path spoofing hook..."

    # d_path function signature and local vars:
    #   char *d_path(const struct path *path, char *buf, int buflen)
    #   {
    #       char *res = buf + buflen;
    #       struct path root;
    #       int error;
    #
    # We inject after "int error;" to spoof virtual paths for injected files

    awk '
    /^char \*d_path\(const struct path \*path, char \*buf, int buflen\)$/ {
        in_dpath = 1
    }
    in_dpath && /^	int error;$/ {
        print $0
        print ""
        print "#ifdef CONFIG_ZEROMOUNT"
        print "\tif (path->dentry && d_backing_inode(path->dentry)) {"
        print "\t\tchar *v_path = zeromount_get_virtual_path_for_inode(d_backing_inode(path->dentry));"
        print ""
        print "\t\tif (v_path) {"
        print "\t\t\tint len = strlen(v_path);"
        print "\t\t\tif (buflen < len + 1) {"
        print "\t\t\t\tkfree(v_path);"
        print "\t\t\t\treturn ERR_PTR(-ENAMETOOLONG);"
        print "\t\t\t}"
        print "\t\t\t*--res = '"'"'\\0'"'"';"
        print "\t\t\tres -= len;"
        print "\t\t\tmemcpy(res, v_path, len);"
        print ""
        print "\t\t\tkfree(v_path);"
        print "\t\t\treturn res;"
        print "\t\t}"
        print "\t}"
        print "#endif"
        print ""
        in_dpath = 0
        next
    }
    { print }
    ' "$TARGET" > "${TARGET}.tmp" && mv "${TARGET}.tmp" "$TARGET"

    if ! grep -q "zeromount_get_virtual_path_for_inode" "$TARGET"; then
        echo "Error: Failed to inject d_path() hook"
        exit 1
    fi
}

inject_include
inject_dpath_hook

echo "ZeroMount d_path.c hooks injected successfully."
