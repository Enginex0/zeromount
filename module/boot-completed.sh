#!/system/bin/sh
MODDIR="${0%/*}"

. "$MODDIR/common.sh"
[ -z "$ABI" ] && exit 0
[ -x "$BIN" ] || exit 0

rm -f /data/adb/zeromount/.bootcount

SUSFS_VER=$("$BIN" mount --show-version 2>/dev/null || echo "")
BASE_DESC="ZeroMount - SUSFS/KSU metamodule"
if [ -n "$SUSFS_VER" ]; then
    STATUS="[SUSFS: ${SUSFS_VER}, Active]"
else
    STATUS="[SUSFS: not detected]"
fi
sed -i "s|^description=.*|description=${STATUS} ${BASE_DESC}|" "$MODDIR/module.prop"

# Ensure KSU unmounts module overlays from app namespaces
KSU_BIN=""
[ -x /data/adb/ksu/bin/ksud ] && KSU_BIN=/data/adb/ksu/bin/ksud
[ -x /data/adb/ap/bin/ksud ] && KSU_BIN=/data/adb/ap/bin/ksud

if [ -n "$KSU_BIN" ]; then
    KUMOUNT=$("$BIN" config get brene.kernel_umount 2>/dev/null)
    if [ "$KUMOUNT" = "true" ]; then
        "$KSU_BIN" feature set kernel_umount 1
    else
        "$KSU_BIN" feature set kernel_umount 0
    fi
    "$KSU_BIN" feature save
fi

# Emoji app overrides need pm (package manager), only available post-boot
"$BIN" emoji apply-apps 2>/dev/null || true
