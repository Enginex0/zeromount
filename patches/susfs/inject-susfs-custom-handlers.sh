#!/bin/bash
# inject-susfs-custom-handlers.sh
# Injects fork-specific SUSFS handlers into KernelSU supercalls.c
# and adds required Kconfig/defconfig entries.
#
# Usage: ./inject-susfs-custom-handlers.sh SUPERCALLS SUSFS_SOURCE KCONFIG DEFCONFIG

set -e

SUPERCALLS="$1"
SUSFS_SOURCE="$2"
KCONFIG="$3"
DEFCONFIG="$4"

[ -z "$SUPERCALLS" ] || [ -z "$SUSFS_SOURCE" ] && {
    echo "Usage: $0 SUPERCALLS SUSFS_SOURCE [KCONFIG] [DEFCONFIG]"
    exit 1
}

echo "=== SUSFS Custom Handler Injection ==="

# Handler definitions: function_name|cmd_name|anchor_cmd|anchor_endif|handler_call
HANDLERS=(
    "susfs_add_sus_kstat_redirect|CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT|CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY|CONFIG_KSU_SUSFS_SUS_KSTAT|susfs_add_sus_kstat_redirect(arg)"
    "susfs_add_open_redirect_all|CMD_SUSFS_ADD_OPEN_REDIRECT_ALL|CMD_SUSFS_ADD_OPEN_REDIRECT|CONFIG_KSU_SUSFS_OPEN_REDIRECT|susfs_add_open_redirect_all(arg)"
)

# Kconfig definitions: function_name|config_name|description|help_text
KCONFIGS=(
    "susfs_check_unicode_bypass|KSU_SUSFS_UNICODE_FILTER|Unicode Filter (blocks scoped storage bypass)|Blocks filesystem path attacks using unicode characters."
)

inject_count=0
kconfig_count=0

# Inject handlers into supercalls.c
for entry in "${HANDLERS[@]}"; do
    IFS='|' read -r func cmd anchor_cmd anchor_endif handler_call <<< "$entry"

    if grep -q "$func" "$SUSFS_SOURCE" 2>/dev/null; then
        if [ -f "$SUPERCALLS" ] && ! grep -q "$cmd" "$SUPERCALLS"; then
            echo "[+] Injecting $cmd"
            sed -i "/$anchor_cmd/,/#endif.*$anchor_endif/ {
                /#endif.*$anchor_endif/ i\\
        if (cmd == $cmd) {\\
            $handler_call;\\
            return 0;\\
        }
            }" "$SUPERCALLS"
            ((inject_count++)) || true
        else
            echo "[=] $cmd already present"
        fi
    else
        echo "[-] $func not in source, skipping"
    fi
done

# Add Kconfig entries
if [ -n "$KCONFIG" ] && [ -f "$KCONFIG" ]; then
    for entry in "${KCONFIGS[@]}"; do
        IFS='|' read -r func config desc help_text <<< "$entry"

        if grep -q "$func" "$SUSFS_SOURCE" 2>/dev/null; then
            if ! grep -q "$config" "$KCONFIG"; then
                echo "[+] Adding $config to Kconfig"
                printf '\nconfig %s\n    bool "%s"\n    depends on KSU_SUSFS\n    default y\n    help\n      %s\n' \
                    "$config" "$desc" "$help_text" >> "$KCONFIG"
                ((kconfig_count++)) || true

                # Add to defconfig if provided
                if [ -n "$DEFCONFIG" ] && [ -f "$DEFCONFIG" ]; then
                    if ! grep -q "CONFIG_$config" "$DEFCONFIG"; then
                        echo "CONFIG_$config=y" >> "$DEFCONFIG"
                        echo "[+] Added CONFIG_$config to defconfig"
                    fi
                fi
            else
                echo "[=] $config already in Kconfig"
            fi
        else
            echo "[-] $func not in source, skipping Kconfig"
        fi
    done
fi

echo "=== Done: $inject_count handlers injected, $kconfig_count Kconfig entries added ==="
