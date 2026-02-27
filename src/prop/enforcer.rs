use std::process::Command;

use tracing::trace;

pub fn enforce_once(props: &[(&str, &str)]) {
    for &(name, value) in props {
        let current = getprop(name);
        if current.as_deref() != Some(value) {
            trace!(prop = name, from = ?current, to = value, "enforce");
            resetprop(name, value);
        }
    }
}

pub fn resetprop(name: &str, value: &str) {
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
