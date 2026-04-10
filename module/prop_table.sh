#!/system/bin/sh
# Prop spoofing tables — direct port from src/prop/table.rs
# Sourced by service.sh. Uses newline-delimited strings for mksh compatibility.

# 29 stealth props (table.rs GENERAL) — format: name=value
STEALTH_PROPS="ro.debuggable=0
ro.secure=1
ro.build.type=user
ro.build.tags=release-keys
ro.boot.vbmeta.device_state=locked
ro.boot.verifiedbootstate=green
ro.boot.flash.locked=1
ro.boot.veritymode=enforcing
ro.adb.secure=1
ro.crypto.state=encrypted
ro.force.debuggable=0
ro.kernel.qemu=
ro.secureboot.lockstate=locked
ro.is_ever_orange=0
ro.bootmode=normal
ro.bootimage.build.tags=release-keys
vendor.boot.vbmeta.device_state=locked
vendor.boot.verifiedbootstate=green
ro.boot.realme.lockstate=1
ro.boot.realmebootstate=green
ro.boot.verifiedbooterror=
ro.boot.veritymode.managed=yes
ro.boot.vbmeta.hash_alg=sha256
ro.boot.vbmeta.avb_version=1.3
ro.boot.vbmeta.invalidate_on_error=yes
sys.oem_unlock_allowed=0
ro.vendor.boot.warranty_bit=0
ro.vendor.warranty_bit=0
ro.boot.warranty_bit=0
ro.warranty_bit=0"

# 8 PIF-leaking props to nuke (table.rs NUKE_PIF)
NUKE_PIF="persist.sys.pihooks.status
persist.sys.pihooks
ro.pihooks.enable
persist.pihooks.mainline_update
persist.sys.pixelprops.pi
persist.sys.pixelprops.gms
persist.sys.pixelprops.gphotos
persist.sys.pixelprops.netflix"

# 13 custom ROM identity props to nuke (table.rs NUKE_CUSTOM_ROM)
NUKE_CUSTOM_ROM="ro.lineage.build.version
ro.lineage.build.version.plat_sdk
ro.lineage.version
ro.lineage.display.version
ro.lineage.releasetype
ro.lineageaudio.version
ro.crdroid.build.version
ro.crdroid.version
ro.crdroid.display.version
ro.modversion
ro.romversion
ro.rom.build.display.id
ro.custom.build.version"
