#!/bin/bash
# inject-susfs-supercall-dispatch.sh
# Adds CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT and CMD_SUSFS_ADD_OPEN_REDIRECT_ALL
# case handlers into the KSU supercalls dispatch in 10_enable_susfs_for_ksu.patch.
#
# NOTE: Currently unused in the metamodule build pipeline. The build uses
# KernelSU-Next's dev_susfs branch (which has SUSFS pre-integrated), so
# 10_enable_susfs_for_ksu.patch is never applied via git-apply. The same
# handler injection is performed directly on supercalls.c by
# inject-susfs-custom-handlers.sh instead. This script is retained for
# compatibility with build flows that apply the GKI .patch file on a
# non-dev_susfs KernelSU branch.
#
# Usage: ./inject-susfs-supercall-dispatch.sh <SUSFS_KERNEL_PATCHES_DIR>

set -e

SUSFS_DIR="$1"

if [ -z "$SUSFS_DIR" ]; then
    echo "Usage: $0 <SUSFS_KERNEL_PATCHES_DIR>"
    exit 1
fi

KSU_PATCH="$SUSFS_DIR/KernelSU/10_enable_susfs_for_ksu.patch"

if [ ! -f "$KSU_PATCH" ]; then
    echo "FATAL: missing $KSU_PATCH"
    exit 1
fi

echo "=== inject-susfs-supercall-dispatch ==="
inject_count=0

# --- 1. kstat_redirect handler ---
if grep -q 'CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT' "$KSU_PATCH"; then
    echo "[=] CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT handler already present"
else
    echo "[+] Injecting CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT handler"
    # Anchor: after CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY handler block
    # The pattern in the patch is:
    #   +        if (cmd == CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY) {
    #   +            susfs_add_sus_kstat(arg);
    #   +            return 0;
    #   +        }
    # Insert after the closing +        }
    sed -i '/CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY/,/+        }/ {
        /+        }/ a\
+        if (cmd == CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT) {\
+            susfs_add_sus_kstat_redirect(arg);\
+            return 0;\
+        }
    }' "$KSU_PATCH"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT' "$KSU_PATCH"; then
    echo "FATAL: CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT handler injection failed"
    exit 1
fi

# --- 2. open_redirect_all handler ---
if grep -q 'CMD_SUSFS_ADD_OPEN_REDIRECT_ALL' "$KSU_PATCH"; then
    echo "[=] CMD_SUSFS_ADD_OPEN_REDIRECT_ALL handler already present"
else
    echo "[+] Injecting CMD_SUSFS_ADD_OPEN_REDIRECT_ALL handler"
    # Anchor: after CMD_SUSFS_ADD_OPEN_REDIRECT handler block
    sed -i '/CMD_SUSFS_ADD_OPEN_REDIRECT)/,/+        }/ {
        /+        }/ a\
+        if (cmd == CMD_SUSFS_ADD_OPEN_REDIRECT_ALL) {\
+            susfs_add_open_redirect_all(arg);\
+            return 0;\
+        }
    }' "$KSU_PATCH"
    ((inject_count++)) || true
fi

# Validate
if ! grep -q 'CMD_SUSFS_ADD_OPEN_REDIRECT_ALL' "$KSU_PATCH"; then
    echo "FATAL: CMD_SUSFS_ADD_OPEN_REDIRECT_ALL handler injection failed"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
