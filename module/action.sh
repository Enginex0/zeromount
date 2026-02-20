#!/system/bin/sh
# Triggered via KSU app "Action" button — resets config to defaults
CONFIG="/data/adb/zeromount/config.toml"
BACKUP="/data/adb/zeromount/config.toml.bak"

echo "🧹 ZeroMount Config Reset"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ -f "$CONFIG" ] || [ -f "$BACKUP" ]; then
    rm -f "$CONFIG" "$BACKUP"
    echo "✅ Config cleared — fresh defaults on next boot"
    echo ""
    echo "🔄 Reboot to apply"
else
    echo "ℹ️  Already using defaults — nothing to reset"
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "👻 GHOST"
