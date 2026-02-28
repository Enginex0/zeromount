use std::ffi::{c_char, c_void, CString};
use std::process::Command;
use std::thread;
use std::time::Duration;

use tracing::trace;

extern "C" {
    fn __system_property_find(name: *const c_char) -> *const c_void;
    fn __system_property_serial(pi: *const c_void) -> u32;
    fn __system_property_wait(
        pi: *const c_void,
        old_serial: u32,
        new_serial: *mut u32,
        timeout: *const c_void,
    ) -> bool;
}

pub(super) fn enforce_once(props: &[(&str, &str)]) {
    for &(name, value) in props {
        let current = getprop(name);
        if current.as_deref() != Some(value) {
            trace!(prop = name, from = ?current, to = value, "enforce");
            resetprop(name, value);
        }
    }
}

pub(super) fn resetprop(name: &str, value: &str) {
    let _ = Command::new("resetprop")
        .args(["-n", name, value])
        .output();
}

fn getprop(name: &str) -> Option<String> {
    Command::new("getprop")
        .arg(name)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

// Blocks on __system_property_wait — re-enforces the instant the system mutates this prop
pub(super) fn watch_prop(name: &'static str, value: &'static str) {
    thread::spawn(move || {
        resetprop(name, value);
        let cname = CString::new(name).unwrap();

        loop {
            let pi = unsafe { __system_property_find(cname.as_ptr()) };
            if pi.is_null() {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let serial = unsafe { __system_property_serial(pi) };
            let mut new_serial = 0u32;
            unsafe {
                __system_property_wait(pi, serial, &mut new_serial, std::ptr::null());
            }

            let current = getprop(name);
            if current.as_deref() != Some(value) {
                resetprop(name, value);
                trace!(prop = name, "re-enforced");
            }
        }
    });
}
