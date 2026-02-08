#!/bin/bash
# inject-susfs-mount-display.sh
# Modifies the GKI SUSFS patch file to add zeromount UID exclusion checks
# in the 3 show_* functions (show_vfsmnt, show_mountinfo, show_vfsstat).
#
# NOTE: This operates on a .patch file, not a .c file directly. The sed
# patterns match patch-format lines (starting with +).
#
# Usage: ./inject-susfs-mount-display.sh <SUSFS_KERNEL_PATCHES_DIR>

set -e

SUSFS_DIR="$1"

if [ -z "$SUSFS_DIR" ]; then
    echo "Usage: $0 <SUSFS_KERNEL_PATCHES_DIR>"
    exit 1
fi

# Find the GKI patch file (version-specific name)
GKI_PATCH=$(find "$SUSFS_DIR" -maxdepth 1 -name '50_add_susfs_in_gki-*.patch' -print -quit)

if [ -z "$GKI_PATCH" ] || [ ! -f "$GKI_PATCH" ]; then
    echo "FATAL: no 50_add_susfs_in_gki-*.patch found in $SUSFS_DIR"
    exit 1
fi

echo "=== inject-susfs-mount-display ==="
echo "    Target: $(basename "$GKI_PATCH")"
inject_count=0

# Check if zeromount guards are already present
if grep -q 'CONFIG_ZEROMOUNT' "$GKI_PATCH"; then
    echo "[=] CONFIG_ZEROMOUNT guards already present in GKI patch"
else
    echo "[+] Injecting CONFIG_ZEROMOUNT guards into GKI patch"

    # 1. Add extern declaration after EVERY "+extern bool susfs_is_current_ksu_domain(void);"
    # The patch has multiple #ifdef blocks (one per patched .c file), each needs the extern.
    sed -i '/^+extern bool susfs_is_current_ksu_domain(void);/a +#ifdef CONFIG_ZEROMOUNT\n+extern bool susfs_is_uid_zeromount_excluded(uid_t uid);\n+#endif' "$GKI_PATCH"

    # 2. Inject zeromount condition check into each show_* block.
    # The upstream pattern in the patch is:
    #   +		!susfs_is_current_ksu_domain()
    #   +#endif
    #   +		)
    #
    # We transform it to:
    #   +		!susfs_is_current_ksu_domain()
    #   +#ifdef CONFIG_ZEROMOUNT
    #   +		&& !susfs_is_uid_zeromount_excluded(current_uid().val)
    #   +#endif
    #   +#endif
    #   +		)
    #
    # Wait -- that would add an extra #endif. Looking at the fork version,
    # the structure is actually:
    #   +		!susfs_is_current_ksu_domain()
    #   +#ifdef CONFIG_ZEROMOUNT
    #   +		&& !susfs_is_uid_zeromount_excluded(current_uid().val)
    #   +#endif
    #   +		)
    # The original +#endif (which was between ksu_domain and ')') is REMOVED
    # and replaced with the zeromount ifdef/endif block. The outer #endif
    # for CONFIG_KSU_SUSFS_SUS_MOUNT is the one after the closing '}'.
    #
    # So we replace the line "+#endif" that comes right after
    # "+		!susfs_is_current_ksu_domain()" with the zeromount block.

    # Use awk for multi-line pattern replacement (more reliable than sed for this)
    awk '
    /^\+\t\t!susfs_is_current_ksu_domain\(\)$/ {
        print
        # Read next line - should be +#endif
        if (getline nextline > 0) {
            if (nextline == "+#endif") {
                # Replace with zeromount guard
                print "+#ifdef CONFIG_ZEROMOUNT"
                print "+\t\t&& !susfs_is_uid_zeromount_excluded(current_uid().val)"
                print "+#endif"
            } else {
                # Not the expected pattern, keep original
                print nextline
            }
        }
        next
    }
    { print }
    ' "$GKI_PATCH" > "$GKI_PATCH.tmp" && mv "$GKI_PATCH.tmp" "$GKI_PATCH"

    ((inject_count++)) || true
fi

# Validate: should have zeromount references
# 1 extern declaration + 3 inline uses = 4 minimum
count=$(grep -c 'susfs_is_uid_zeromount_excluded' "$GKI_PATCH" || true)
if [ "$count" -lt 4 ]; then
    echo "FATAL: expected at least 4 zeromount references in GKI patch, found $count"
    exit 1
fi

echo "=== Done: $inject_count injections applied ==="
