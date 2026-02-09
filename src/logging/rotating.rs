use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub struct RotatingFileLayer {
    state: Mutex<RotatingState>,
}

struct RotatingState {
    dir: PathBuf,
    current: Option<std::fs::File>,
    current_size: u64,
    max_size: u64,
    max_files: usize,
}

impl RotatingFileLayer {
    pub fn new(dir: &str, max_size: u64, max_files: usize) -> Self {
        let dir = PathBuf::from(dir);
        let _ = fs::create_dir_all(&dir);

        let (file, size) = open_current_log(&dir);

        Self {
            state: Mutex::new(RotatingState {
                dir,
                current: file,
                current_size: size,
                max_size,
                max_files,
            }),
        }
    }
}

fn open_current_log(dir: &Path) -> (Option<std::fs::File>, u64) {
    let path = dir.join("zeromount.log");
    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    (file, size)
}

impl RotatingState {
    fn rotate(&mut self) {
        // Close current handle before renaming
        self.current.take();

        // Shift existing logs: .2 -> .3, .1 -> .2, current -> .1
        for i in (1..self.max_files).rev() {
            let src = if i == 1 {
                self.dir.join("zeromount.log")
            } else {
                self.dir.join(format!("zeromount.log.{}", i - 1))
            };
            let dst = self.dir.join(format!("zeromount.log.{i}"));
            let _ = fs::rename(&src, &dst);
        }

        let (file, size) = open_current_log(&self.dir);
        self.current = file;
        self.current_size = size;
    }

    fn write_line(&mut self, line: &str) {
        if self.current_size >= self.max_size {
            self.rotate();
        }

        let file = match self.current.as_mut() {
            Some(f) => f,
            None => return,
        };

        let bytes = line.as_bytes();
        if file.write_all(bytes).is_ok() {
            let _ = file.write_all(b"\n");
            self.current_size += bytes.len() as u64 + 1;
        }
    }
}

struct LogVisitor {
    message: String,
    fields: String,
}

impl LogVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: String::new(),
        }
    }
}

impl Visit for LogVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            if !self.fields.is_empty() { self.fields.push(' '); }
            self.fields.push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let raw = format!("{:?}", value);
            // Strip surrounding debug quotes from string values
            self.message = raw.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(&raw)
                .to_string();
        } else {
            if !self.fields.is_empty() { self.fields.push(' '); }
            self.fields.push_str(&format!("{}={:?}", field.name(), value));
        }
    }
}

fn timestamp() -> String {
    // Seconds since epoch, good enough for boot-time logs on Android
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

impl<S: Subscriber> Layer<S> for RotatingFileLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut guard = match self.state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);

        let level = event.metadata().level();
        let target = event.metadata().target();
        let ts = timestamp();
        let line = if visitor.fields.is_empty() {
            format!("{ts} [{level}] {target}: {}", visitor.message)
        } else {
            format!("{ts} [{level}] {target}: {} {}", visitor.message, visitor.fields)
        };
        guard.write_line(&line);
    }
}
