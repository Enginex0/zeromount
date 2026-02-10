#!/system/bin/sh
MODDIR="${0%/*}"
. "$MODDIR/common.sh"
[ -z "$ABI" ] || [ ! -x "$BIN" ] || "$BIN" module scan --cleanup "$1" 2>/dev/null
