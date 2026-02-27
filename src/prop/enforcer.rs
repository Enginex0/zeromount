use std::ffi::{c_char, c_void, CStr, CString};
use std::process::Command;
use std::ptr;

use tracing::{debug, info, trace};

use super::ffi::{self, PropInfo};

struct WatchedProp {
    cname: CString,
    display: &'static str,
    target: &'static str,
    pi: *const PropInfo,
    serial: u32,
}

unsafe impl Send for WatchedProp {}

pub fn enforce_once(props: &[(&str, &str)]) {
    for &(name, value) in props {
        let current = getprop(name);
        if current.as_deref() != Some(value) {
            resetprop(name, value);
        }
    }
}

pub fn resetprop(name: &str, value: &str) {
    let _ = Command::new("resetprop")
        .args(["-n", name, value])
        .output();
}

pub fn watch_loop(props: &[(&'static str, &'static str)]) -> ! {
    let mut watched: Vec<WatchedProp> = props
        .iter()
        .filter_map(|&(name, value)| {
            let cname = CString::new(name).ok()?;
            let pi = unsafe { ffi::__system_property_find(cname.as_ptr()) };
            let serial = if pi.is_null() { 0 } else {
                unsafe { ffi::__system_property_serial(pi) }
            };
            Some(WatchedProp { cname, display: name, target: value, pi, serial })
        })
        .collect();

    for wp in &watched {
        let current = read_prop_value(wp.pi);
        if current.as_deref() != Some(wp.target) {
            trace!(prop = wp.display, from = ?current, to = wp.target, "initial enforce");
            resetprop(wp.display, wp.target);
        }
    }

    refresh_serials(&mut watched);
    let mut global_serial = current_global_serial();

    info!(count = watched.len(), "property watch loop active");

    loop {
        let mut new_serial = 0u32;
        unsafe {
            ffi::__system_property_wait(ptr::null(), global_serial, &mut new_serial, ptr::null());
        }
        global_serial = new_serial;

        let mut reverted = 0u32;
        for wp in &mut watched {
            if wp.pi.is_null() {
                wp.pi = unsafe { ffi::__system_property_find(wp.cname.as_ptr()) };
                if wp.pi.is_null() { continue; }
            }

            let serial = unsafe { ffi::__system_property_serial(wp.pi) };
            if serial == wp.serial { continue; }
            wp.serial = serial;

            let current = read_prop_value(wp.pi);
            if current.as_deref() != Some(wp.target) {
                trace!(prop = wp.display, from = ?current, to = wp.target, "reverted");
                resetprop(wp.display, wp.target);
                reverted += 1;
            }
        }

        if reverted > 0 {
            debug!(reverted, "properties reverted");
            refresh_serials(&mut watched);
            global_serial = current_global_serial();
        }
    }
}

fn refresh_serials(watched: &mut [WatchedProp]) {
    for wp in watched.iter_mut() {
        if !wp.pi.is_null() {
            wp.serial = unsafe { ffi::__system_property_serial(wp.pi) };
        }
    }
}

fn current_global_serial() -> u32 {
    let mut serial = 0u32;
    let timeout = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
        ffi::__system_property_wait(ptr::null(), 0, &mut serial, &timeout);
    }
    serial
}

fn read_prop_value(pi: *const PropInfo) -> Option<String> {
    if pi.is_null() { return None; }
    let mut buf = [0u8; 92];
    unsafe {
        ffi::__system_property_read_callback(pi, prop_cb, buf.as_mut_ptr() as *mut c_void);
        let cstr = CStr::from_ptr(buf.as_ptr() as *const c_char);
        Some(cstr.to_string_lossy().into_owned())
    }
}

unsafe extern "C" fn prop_cb(
    cookie: *mut c_void,
    _name: *const c_char,
    value: *const c_char,
    _serial: u32,
) {
    let buf = cookie as *mut u8;
    let len = libc::strlen(value).min(91);
    ptr::copy_nonoverlapping(value as *const u8, buf, len);
    *buf.add(len) = 0;
}

fn getprop(name: &str) -> Option<String> {
    Command::new("getprop")
        .arg(name)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}
