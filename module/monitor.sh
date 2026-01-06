MODDIR=${0%/*}
PROP_FILE="$MODDIR/module.prop"
MODULE_COUNT="$1"
BASE_DESC="A metamodule that replaces OverlayFS/MagicMount with VFS path redirection."

if [ -z "$MODULE_COUNT" ]; then
    MODULE_COUNT=0
fi
if [ "$MODULE_COUNT" -gt 0 ]; then
    STATUS="[✅Working: $MODULE_COUNT modules active] \\\\n"
else
    STATUS="[⚠️Idle: No modules found] \\\\n"
fi

sed -i "s|^description=.*|description=$STATUS$BASE_DESC|" "$PROP_FILE"
