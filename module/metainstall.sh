#!/system/bin/sh
# Runs when ANOTHER module is installed via KSU/APatch
MODDIR="${0%/*}"

KSU_HAS_METAMODULE=true
KSU_METAMODULE=meta-zeromount
export KSU_HAS_METAMODULE KSU_METAMODULE

# KSU install_module calls this callback — stub to suppress default partition handling
handle_partition() { : ; }

install_module

# KSU sets system_file during its own install flow, but the metamodule
# installer can race with restorecon. Belt-and-suspenders: force correct
# context so font/overlay modules work without reinstall.
if [ -d "$MODPATH/system" ] && command -v chcon >/dev/null 2>&1; then
    chcon -R u:object_r:system_file:s0 "$MODPATH/system" 2>/dev/null
fi
