#!/bin/bash
# inject-susfs-unicode-filter-func.sh
# Injects the susfs_check_unicode_bypass() function body and its declaration
# into upstream SUSFS source, guarded by CONFIG_KSU_SUSFS_UNICODE_FILTER.
#
# Usage: ./inject-susfs-unicode-filter-func.sh <SUSFS_KERNEL_PATCHES_DIR>

set -e

SUSFS_DIR="$1"

if [ -z "$SUSFS_DIR" ]; then
    echo "Usage: $0 <SUSFS_KERNEL_PATCHES_DIR>"
    exit 1
fi

SUSFS_H="$SUSFS_DIR/include/linux/susfs.h"
SUSFS_C="$SUSFS_DIR/fs/susfs.c"

for f in "$SUSFS_H" "$SUSFS_C"; do
    if [ ! -f "$f" ]; then
        echo "FATAL: missing $f"
        exit 1
    fi
done

echo "=== inject-susfs-unicode-filter-func ==="
inject_count=0

# --- 1. #include <linux/limits.h> in susfs.c ---
if grep -q '#include <linux/limits.h>' "$SUSFS_C"; then
    echo "[=] #include <linux/limits.h> already present in susfs.c"
else
    echo "[+] Injecting #include <linux/limits.h> into susfs.c"
    sed -i '/#include <linux\/susfs.h>/a #include <linux/limits.h>' "$SUSFS_C"
    ((inject_count++)) || true
fi

if ! grep -q '#include <linux/limits.h>' "$SUSFS_C"; then
    echo "FATAL: #include <linux/limits.h> injection failed"
    exit 1
fi

# --- 2. Unicode filter function body in susfs.c ---
# Anchor: after the SUSFS_LOGE macro line, before susfs_starts_with.
# The block goes between the #endif of the log macros and the next function.
if grep -q 'susfs_check_unicode_bypass' "$SUSFS_C"; then
    echo "[=] susfs_check_unicode_bypass already present in susfs.c"
else
    echo "[+] Injecting susfs_check_unicode_bypass function into susfs.c"
    # Anchor on the SUSFS_LOGE macro (last line of the log config block)
    sed -i '/^#define SUSFS_LOGE/,/^#endif/ {
        /^#endif/ a\
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
\
static const unsigned char PAT_RTL_OVERRIDE[]   = {0xE2, 0x80, 0xAE};\
static const unsigned char PAT_LTR_OVERRIDE[]   = {0xE2, 0x80, 0xAD};\
static const unsigned char PAT_RTL_EMBED[]      = {0xE2, 0x80, 0xAB};\
static const unsigned char PAT_LTR_EMBED[]      = {0xE2, 0x80, 0xAA};\
static const unsigned char PAT_ZWSP[]           = {0xE2, 0x80, 0x8B};\
static const unsigned char PAT_ZWNJ[]           = {0xE2, 0x80, 0x8C};\
static const unsigned char PAT_ZWJ[]            = {0xE2, 0x80, 0x8D};\
static const unsigned char PAT_BOM[]            = {0xEF, 0xBB, 0xBF};\
\
bool susfs_check_unicode_bypass(const char __user *filename)\
{\
\tchar *buf;\
\tunsigned int uid;\
\tbool blocked = false;\
\tlong len;\
\tint i;\
\
\tif (!filename)\
\t\treturn false;\
\
\tuid = current_uid().val;\
\tif (uid == 0 || uid == 1000)\
\t\treturn false;\
\
\tbuf = kmalloc(PATH_MAX, GFP_KERNEL);\
\tif (!buf)\
\t\treturn false;\
\
\tlen = strncpy_from_user(buf, filename, PATH_MAX - 1);\
\tif (len <= 0) {\
\t\tkfree(buf);\
\t\treturn false;\
\t}\
\tbuf[len] = '"'"'\\0'"'"';\
\
\tfor (i = 0; i < len; i++) {\
\t\tunsigned char c = (unsigned char)buf[i];\
\
\t\tif (c <= 127)\
\t\t\tcontinue;\
\
\t\tif (i + 2 < len) {\
\t\t\tif (memcmp(&buf[i], PAT_RTL_OVERRIDE, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_LTR_OVERRIDE, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_RTL_EMBED, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_LTR_EMBED, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_ZWSP, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_ZWNJ, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_ZWJ, 3) == 0 ||\
\t\t\t    memcmp(&buf[i], PAT_BOM, 3) == 0) {\
\t\t\t\tSUSFS_LOGI("unicode: blocked pattern uid=%u\\n", uid);\
\t\t\t\tblocked = true;\
\t\t\t\tbreak;\
\t\t\t}\
\t\t}\
\
\t\tif (c == 0xD0 || c == 0xD1) {\
\t\t\tSUSFS_LOGI("unicode: blocked cyrillic uid=%u\\n", uid);\
\t\t\tblocked = true;\
\t\t\tbreak;\
\t\t}\
\
\t\tif (c == 0xCC || (c == 0xCD && i + 1 < len && (unsigned char)buf[i+1] <= 0xAF)) {\
\t\t\tSUSFS_LOGI("unicode: blocked diacritical uid=%u\\n", uid);\
\t\t\tblocked = true;\
\t\t\tbreak;\
\t\t}\
\
\t\tSUSFS_LOGI("unicode: blocked byte 0x%02x uid=%u\\n", c, uid);\
\t\tblocked = true;\
\t\tbreak;\
\t}\
\tkfree(buf);\
\treturn blocked;\
}\
#endif
    }' "$SUSFS_C"
    ((inject_count++)) || true
fi

if ! grep -q 'susfs_check_unicode_bypass' "$SUSFS_C"; then
    echo "FATAL: susfs_check_unicode_bypass function injection failed"
    exit 1
fi

# --- 3. Declaration in susfs.h ---
# Anchor: before the final #endif that closes the header guard
if grep -q 'susfs_check_unicode_bypass' "$SUSFS_H"; then
    echo "[=] susfs_check_unicode_bypass declaration already present in susfs.h"
else
    echo "[+] Injecting susfs_check_unicode_bypass declaration into susfs.h"
    # Insert before susfs_init declaration as a reliable anchor near the end
    sed -i '/^void susfs_init(void);/a \
\
#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER\
bool susfs_check_unicode_bypass(const char __user *filename);\
#endif' "$SUSFS_H"
    ((inject_count++)) || true
fi

if ! grep -q 'susfs_check_unicode_bypass' "$SUSFS_H"; then
    echo "FATAL: susfs_check_unicode_bypass declaration injection failed"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
