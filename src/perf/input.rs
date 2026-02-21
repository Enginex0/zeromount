use std::fs;
use std::path::Path;

use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct TouchDevice {
    pub event_path: String,
    pub name: String,
}

const TOUCH_PATTERNS: &[&str] = &[
    "touch", "screen", "panel", "_ts",
    "synaptics", "focaltech", "goodix", "himax",
    "novatek", "ilitek", "mxt", "atmel",
    "nvt", "fts", "sec_e",
];

pub fn detect_touchscreens() -> Vec<TouchDevice> {
    let mut devices = Vec::new();

    let Ok(entries) = fs::read_dir("/sys/class/input") else {
        warn!("cannot read /sys/class/input");
        return devices;
    };

    for entry in entries.flatten() {
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        if !dir_name.starts_with("input") {
            continue;
        }

        let input_path = entry.path();
        let name_path = input_path.join("name");
        let Some(name) = fs::read_to_string(&name_path).ok().map(|s| s.trim().to_string()) else {
            continue;
        };

        if !is_touchscreen(&name) {
            continue;
        }

        if let Some(event_path) = resolve_event_device(&input_path) {
            info!(name = %name, event = %event_path, "detected touchscreen");
            devices.push(TouchDevice { event_path, name });
        } else {
            debug!(name = %name, "touchscreen found but no event device");
        }
    }

    if devices.is_empty() {
        if let Some(dev) = fallback_proc_scan() {
            info!(name = %dev.name, event = %dev.event_path, "touchscreen via /proc fallback");
            devices.push(dev);
        }
    }

    devices
}

fn is_touchscreen(name: &str) -> bool {
    let lower = name.to_lowercase();
    TOUCH_PATTERNS.iter().any(|p| lower.contains(p))
}

fn resolve_event_device(input_path: &Path) -> Option<String> {
    let entries = fs::read_dir(input_path).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("event") {
            let event_num: u32 = name.strip_prefix("event")?.parse().ok()?;
            let dev_path = format!("/dev/input/event{event_num}");
            if Path::new(&dev_path).exists() {
                return Some(dev_path);
            }
        }
    }

    let dir_name = input_path.file_name()?.to_string_lossy();
    let input_num: u32 = dir_name.strip_prefix("input")?.parse().ok()?;
    let dev_path = format!("/dev/input/event{input_num}");
    if Path::new(&dev_path).exists() {
        return Some(dev_path);
    }

    None
}

fn fallback_proc_scan() -> Option<TouchDevice> {
    let content = fs::read_to_string("/proc/bus/input/devices").ok()?;

    let mut current_name = String::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("N: Name=\"") {
            current_name = rest.trim_end_matches('"').to_string();
        } else if line.starts_with("H: Handlers=") {
            if is_touchscreen(&current_name) {
                let handlers = line.strip_prefix("H: Handlers=")?;
                for handler in handlers.split_whitespace() {
                    if handler.starts_with("event") {
                        let dev_path = format!("/dev/input/{handler}");
                        if Path::new(&dev_path).exists() {
                            return Some(TouchDevice {
                                event_path: dev_path,
                                name: current_name,
                            });
                        }
                    }
                }
            }
            current_name.clear();
        }
    }

    None
}
