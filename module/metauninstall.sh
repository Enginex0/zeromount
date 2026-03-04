#!/system/bin/sh
MODDIR="${0%/*}"
. "$MODDIR/common.sh"
MODULE_ID="$1"
[ -z "$ABI" ] || [ ! -x "$BIN" ] || [ -z "$MODULE_ID" ] || "$BIN" module scan --cleanup "$MODULE_ID" 2>/dev/null
