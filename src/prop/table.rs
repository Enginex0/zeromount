pub(super) struct PropEntry {
    pub name: &'static str,
    pub value: &'static str,
}

pub(super) static HIDE_DEBUG: &[PropEntry] = &[
    PropEntry { name: "ro.debuggable", value: "0" },
    PropEntry { name: "persist.sys.debuggable", value: "0" },
    PropEntry { name: "persist.service.debuggerd.enable", value: "0" },
    PropEntry { name: "dalvik.vm.checkjni", value: "false" },
    PropEntry { name: "ro.kernel.android.checkjni", value: "0" },
    PropEntry { name: "ro.boot.vbmeta.device_state", value: "locked" },
    PropEntry { name: "ro.boot.verifiedbootstate", value: "green" },
    PropEntry { name: "ro.boot.flash.locked", value: "1" },
    PropEntry { name: "ro.boot.warranty_bit", value: "0" },
    PropEntry { name: "ro.warranty_bit", value: "0" },
    PropEntry { name: "ro.boot.mode", value: "normal" },
    PropEntry { name: "ro.bootmode", value: "normal" },
];

pub(super) static GENERAL: &[PropEntry] = &[
    PropEntry { name: "ro.debuggable", value: "0" },
    PropEntry { name: "ro.secure", value: "1" },
    PropEntry { name: "ro.build.type", value: "user" },
    PropEntry { name: "ro.build.tags", value: "release-keys" },
    PropEntry { name: "ro.boot.vbmeta.device_state", value: "locked" },
    PropEntry { name: "ro.boot.verifiedbootstate", value: "green" },
    PropEntry { name: "ro.boot.flash.locked", value: "1" },
    PropEntry { name: "ro.boot.veritymode", value: "enforcing" },
    PropEntry { name: "ro.adb.secure", value: "1" },
    PropEntry { name: "ro.crypto.state", value: "encrypted" },
    PropEntry { name: "ro.force.debuggable", value: "0" },
    PropEntry { name: "ro.kernel.qemu", value: "" },
    PropEntry { name: "ro.secureboot.lockstate", value: "locked" },
    PropEntry { name: "ro.is_ever_orange", value: "0" },
    PropEntry { name: "ro.bootmode", value: "normal" },
    PropEntry { name: "ro.bootimage.build.tags", value: "release-keys" },
    PropEntry { name: "vendor.boot.vbmeta.device_state", value: "locked" },
    PropEntry { name: "vendor.boot.verifiedbootstate", value: "green" },
    PropEntry { name: "ro.boot.realme.lockstate", value: "1" },
    PropEntry { name: "ro.boot.realmebootstate", value: "green" },
    PropEntry { name: "ro.boot.verifiedbooterror", value: "" },
    PropEntry { name: "ro.boot.veritymode.managed", value: "yes" },
    PropEntry { name: "ro.boot.vbmeta.hash_alg", value: "sha256" },
    PropEntry { name: "ro.boot.vbmeta.avb_version", value: "1.3" },
    PropEntry { name: "ro.boot.vbmeta.invalidate_on_error", value: "yes" },
    PropEntry { name: "sys.oem_unlock_allowed", value: "0" },
    PropEntry { name: "ro.vendor.boot.warranty_bit", value: "0" },
    PropEntry { name: "ro.vendor.warranty_bit", value: "0" },
    PropEntry { name: "ro.boot.warranty_bit", value: "0" },
    PropEntry { name: "ro.warranty_bit", value: "0" },
];

// Dynamic props the system resets — need persistent __system_property_wait enforcement
pub(super) static DYNAMIC_USB: &[PropEntry] = &[
    PropEntry { name: "init.svc.adbd", value: "stopped" },
    PropEntry { name: "sys.usb.config", value: "mtp" },
    PropEntry { name: "sys.usb.state", value: "mtp" },
    PropEntry { name: "sys.usb.ffs.ready", value: "0" },
    PropEntry { name: "sys.usb.ffs.adb.ready", value: "0" },
    PropEntry { name: "persist.sys.usb.config", value: "mtp" },
    PropEntry { name: "persist.sys.usb.reboot.func", value: "mtp" },
    PropEntry { name: "service.adb.root", value: "0" },
    PropEntry { name: "service.adb.tcp.port", value: "-1" },
    PropEntry { name: "persist.service.adb.enable", value: "0" },
    PropEntry { name: "persist.vendor.usb.config", value: "none" },
    PropEntry { name: "vendor.usb.config", value: "none" },
];

pub(super) static BUILD_PROP_PATHS: &[&str] = &[
    "/default.prop",
    "/system/build.prop",
    "/vendor/build.prop",
    "/product/build.prop",
    "/vendor/odm/etc/build.prop",
    "/system/system/build.prop",
    "/system_ext/build.prop",
];
