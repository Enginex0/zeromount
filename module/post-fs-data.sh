#!/system/bin/sh
MODDIR="${0%/*}"
. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 1
[ -x "$BIN" ] || exit 1

"$BIN" detect
