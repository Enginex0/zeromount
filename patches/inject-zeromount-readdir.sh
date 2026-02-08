#!/bin/bash
#
# inject-zeromount-readdir.sh - Inject ZeroMount hooks into fs/readdir.c
#
# Hooks getdents, getdents64, and compat_getdents syscalls to inject
# virtual directory entries via zeromount_inject_dents[64]().
#
# Usage: ./inject-zeromount-readdir.sh <path-to-readdir.c>
#

set -e

READDIR_FILE="${1:-fs/readdir.c}"

if [ ! -f "$READDIR_FILE" ]; then
    echo "Error: File not found: $READDIR_FILE"
    exit 1
fi

echo "Injecting ZeroMount readdir hooks into: $READDIR_FILE"

if grep -q "CONFIG_ZEROMOUNT" "$READDIR_FILE"; then
    echo "File already contains ZeroMount hooks. Skipping."
    exit 0
fi

if ! grep -q '#include <linux/uaccess.h>' "$READDIR_FILE"; then
    echo "Error: Cannot find #include <linux/uaccess.h>"
    exit 1
fi

if ! grep -q 'SYSCALL_DEFINE3(getdents64,' "$READDIR_FILE"; then
    echo "Error: Cannot find SYSCALL_DEFINE3(getdents64,"
    exit 1
fi

cp "$READDIR_FILE" "${READDIR_FILE}.bak"

echo "  [1/4] Injecting zeromount.h include..."
sed -i '/#include <linux\/uaccess.h>/a\
#ifdef CONFIG_ZEROMOUNT\
#include <linux/zeromount.h>\
#endif' "$READDIR_FILE"

echo "  [2/4] Injecting hooks into getdents..."
awk '
BEGIN { state = 0 }

/^SYSCALL_DEFINE3\(getdents,/ { state = 1 }
state == 1 && /^SYSCALL_DEFINE3\(getdents64,/ { state = 0 }

state == 1 && /^[[:space:]]*int error;[[:space:]]*$/ && !var_done {
    print
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tint initial_count = count;"
    print "#endif"
    var_done = 1
    next
}

state == 1 && /return -EBADF;/ && !skip_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tif (f.file->f_pos >= ZEROMOUNT_MAGIC_POS) {"
    print "\t\terror = 0;"
    print "\t\tgoto skip_real_iterate;"
    print "\t}"
    print "#endif"
    skip_done = 1
    next
}

state == 1 && /error = buf\.error;/ && !inject_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "skip_real_iterate:"
    print "\tif (error >= 0 && !signal_pending(current)) {"
    print "\t\tzeromount_inject_dents(f.file, (void __user **)&dirent, &count, &f.file->f_pos);"
    print "\t\terror = initial_count - count;"
    print "\t\tgoto zm_out;"
    print "\t}"
    print "#endif"
    inject_done = 1
    next
}

# Place label before fdput_pos so zeromount can skip the original epilogue
state == 1 && /fdput_pos\(f\);/ && !out_done {
    print "#ifdef CONFIG_ZEROMOUNT"
    print "zm_out:"
    print "#endif"
    print
    out_done = 1
    next
}

{ print }
' "$READDIR_FILE" > "${READDIR_FILE}.tmp" && mv "${READDIR_FILE}.tmp" "$READDIR_FILE"

echo "  [3/4] Injecting hooks into getdents64..."
awk '
BEGIN { state = 0 }

/^SYSCALL_DEFINE3\(getdents64,/ { state = 1 }
state == 1 && /^COMPAT_SYSCALL_DEFINE3\(getdents,/ { state = 0 }
state == 1 && /^}$/ { state = 0 }

state == 1 && /^[[:space:]]*int error;[[:space:]]*$/ && !var_done {
    print
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tint initial_count = count;"
    print "#endif"
    var_done = 1
    next
}

state == 1 && /return -EBADF;/ && !skip_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tif (f.file->f_pos >= ZEROMOUNT_MAGIC_POS) {"
    print "\t\terror = 0;"
    print "\t\tgoto skip_real_iterate;"
    print "\t}"
    print "#endif"
    skip_done = 1
    next
}

state == 1 && /error = buf\.error;/ && !inject_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "skip_real_iterate:"
    print "\tif (error >= 0 && !signal_pending(current)) {"
    print "\t\tzeromount_inject_dents64(f.file, (void __user **)&dirent, &count, &f.file->f_pos);"
    print "\t\terror = initial_count - count;"
    print "\t\tgoto zm_out;"
    print "\t}"
    print "#endif"
    inject_done = 1
    next
}

state == 1 && /fdput_pos\(f\);/ && !out_done {
    print "#ifdef CONFIG_ZEROMOUNT"
    print "zm_out:"
    print "#endif"
    print
    out_done = 1
    next
}

{ print }
' "$READDIR_FILE" > "${READDIR_FILE}.tmp" && mv "${READDIR_FILE}.tmp" "$READDIR_FILE"

echo "  [4/4] Injecting hooks into compat_getdents..."
awk '
BEGIN { state = 0 }

/^COMPAT_SYSCALL_DEFINE3\(getdents,/ { state = 1 }
state == 1 && /^}$/ { state = 0 }

state == 1 && /^[[:space:]]*int error;[[:space:]]*$/ && !var_done {
    print
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tint initial_count = count;"
    print "#endif"
    var_done = 1
    next
}

state == 1 && /return -EBADF;/ && !skip_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "\tif (f.file->f_pos >= ZEROMOUNT_MAGIC_POS) {"
    print "\t\terror = 0;"
    print "\t\tgoto skip_real_iterate;"
    print "\t}"
    print "#endif"
    skip_done = 1
    next
}

state == 1 && /error = buf\.error;/ && !inject_done {
    print
    print ""
    print "#ifdef CONFIG_ZEROMOUNT"
    print "skip_real_iterate:"
    print "\tif (error >= 0 && !signal_pending(current)) {"
    print "\t\tzeromount_inject_dents(f.file, (void __user **)&dirent, &count, &f.file->f_pos);"
    print "\t\terror = initial_count - count;"
    print "\t\tgoto zm_out;"
    print "\t}"
    print "#endif"
    inject_done = 1
    next
}

state == 1 && /fdput_pos\(f\);/ && !out_done {
    print "#ifdef CONFIG_ZEROMOUNT"
    print "zm_out:"
    print "#endif"
    print
    out_done = 1
    next
}

{ print }
' "$READDIR_FILE" > "${READDIR_FILE}.tmp" && mv "${READDIR_FILE}.tmp" "$READDIR_FILE"

echo ""
echo "Verifying injection..."

ERRORS=0

if ! grep -q '#include <linux/zeromount.h>' "$READDIR_FILE"; then
    echo "  [FAIL] zeromount.h include not found"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] zeromount.h include"
fi

INITIAL_COUNT=$(grep -c 'int initial_count = count;' "$READDIR_FILE" || true)
if [ "$INITIAL_COUNT" -ne 3 ]; then
    echo "  [FAIL] Expected 3 initial_count declarations, found $INITIAL_COUNT"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 3 initial_count declarations"
fi

SKIP_LABELS=$(grep -c '^skip_real_iterate:' "$READDIR_FILE" || true)
if [ "$SKIP_LABELS" -ne 3 ]; then
    echo "  [FAIL] Expected 3 skip_real_iterate labels, found $SKIP_LABELS"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 3 skip_real_iterate labels"
fi

INJECT64=$(grep -c 'zeromount_inject_dents64' "$READDIR_FILE" || true)
if [ "$INJECT64" -ne 1 ]; then
    echo "  [FAIL] Expected 1 zeromount_inject_dents64 call, found $INJECT64"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 1 zeromount_inject_dents64 call"
fi

INJECT32=$(grep -c 'zeromount_inject_dents(' "$READDIR_FILE" || true)
if [ "$INJECT32" -ne 2 ]; then
    echo "  [FAIL] Expected 2 zeromount_inject_dents calls, found $INJECT32"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 2 zeromount_inject_dents calls"
fi

MAGIC_POS=$(grep -c 'ZEROMOUNT_MAGIC_POS' "$READDIR_FILE" || true)
if [ "$MAGIC_POS" -ne 3 ]; then
    echo "  [FAIL] Expected 3 ZEROMOUNT_MAGIC_POS checks, found $MAGIC_POS"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 3 ZEROMOUNT_MAGIC_POS checks"
fi

ZM_OUT_LABELS=$(grep -c '^zm_out:' "$READDIR_FILE" || true)
if [ "$ZM_OUT_LABELS" -ne 3 ]; then
    echo "  [FAIL] Expected 3 zm_out labels, found $ZM_OUT_LABELS"
    ERRORS=$((ERRORS + 1))
else
    echo "  [OK] 3 zm_out labels"
fi

echo ""
if [ "$ERRORS" -eq 0 ]; then
    echo "ZeroMount readdir hooks injection complete. Backup at ${READDIR_FILE}.bak"
    exit 0
else
    echo "Injection completed with $ERRORS verification failures."
    echo "Review the output and ${READDIR_FILE}.bak if needed."
    exit 1
fi
