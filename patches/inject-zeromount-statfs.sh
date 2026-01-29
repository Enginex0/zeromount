#!/bin/bash
# inject-zeromount-statfs.sh - Inject ZeroMount statfs spoofing hooks into fs/statfs.c

set -e

TARGET="${1:-fs/statfs.c}"

echo "[INFO] ZeroMount statfs hook injection"
echo "[INFO] Target: $TARGET"

if [ ! -f "$TARGET" ]; then
    echo "[ERROR] Target file not found: $TARGET"
    exit 1
fi

if grep -q "zeromount_spoof_statfs" "$TARGET"; then
    echo "[INFO] Hooks already present - skipping"
    exit 0
fi

cp "$TARGET" "${TARGET}.orig"

echo "[INFO] Injecting include..."
sed -i '/#include "internal\.h"/a\
#ifdef CONFIG_ZEROMOUNT\
#include <linux/zeromount.h>\
#endif' "$TARGET"

if ! grep -q '#include <linux/zeromount.h>' "$TARGET"; then
    echo "[ERROR] Failed to inject include"
    mv "${TARGET}.orig" "$TARGET"
    exit 1
fi
echo "[OK] Include injected"

echo "[INFO] Injecting user_statfs hook..."

# awk state machine: only inject inside user_statfs() function
awk '
BEGIN { in_user_statfs = 0; var_injected = 0; call_injected = 0 }

# Enter user_statfs function (matches "int user_statfs(" at function definition)
/^int user_statfs\(/ { in_user_statfs = 1 }

# Exit on next function definition or closing brace at column 0
in_user_statfs && /^[a-z].*\(/ && !/^int user_statfs/ { in_user_statfs = 0 }
in_user_statfs && /^}$/ { in_user_statfs = 0 }

# Inject variable declaration after "if (!error) {" inside user_statfs only
in_user_statfs && /if \(!error\) \{/ && !var_injected {
    print
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\t\tint spoofed;"
    print "#endif"
    var_injected = 1
    next
}

# Inject call after "error = vfs_statfs(&path, st);" inside user_statfs only
in_user_statfs && /error = vfs_statfs\(&path, st\);/ && !call_injected {
    print
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\t\tspoofed = zeromount_spoof_statfs(pathname, st);"
    print "\t\t(void)spoofed;"
    print "#endif"
    call_injected = 1
    next
}

{ print }
' "$TARGET" > "${TARGET}.tmp" && mv "${TARGET}.tmp" "$TARGET"

if ! grep -q 'zeromount_spoof_statfs' "$TARGET"; then
    echo "[ERROR] Failed to inject user_statfs hook"
    mv "${TARGET}.orig" "$TARGET"
    exit 1
fi
echo "[OK] user_statfs hook injected"

rm -f "${TARGET}.orig"

echo "[SUCCESS] ZeroMount statfs hooks injected"
echo "  - Include: <linux/zeromount.h>"
echo "  - Hook: user_statfs() -> zeromount_spoof_statfs(pathname, st)"
exit 0
