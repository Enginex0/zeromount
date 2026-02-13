use std::ffi::CString;
use std::io;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::core::types::RuntimeState;

const STATUS_JSON_PATH: &str = "/data/adb/zeromount/.status.json";

// inotify event mask bits (from linux/inotify.h)
const IN_CREATE: u32 = 0x0000_0100;
const IN_DELETE: u32 = 0x0000_0200;
const IN_MODIFY: u32 = 0x0000_0002;
const IN_MOVED_TO: u32 = 0x0000_0080;
const IN_MOVED_FROM: u32 = 0x0000_0040;

// inotify_init1 flags
const IN_NONBLOCK: libc::c_int = libc::O_NONBLOCK;
const IN_CLOEXEC: libc::c_int = libc::O_CLOEXEC;

const WATCH_MASK: u32 = IN_CREATE | IN_DELETE | IN_MODIFY | IN_MOVED_TO | IN_MOVED_FROM;

// inotify_event is variable-length; fixed header is 16 bytes on all arches
const INOTIFY_EVENT_HEADER_SIZE: usize = 16;
const EVENT_BUF_SIZE: usize = 4096;
const DEBOUNCE_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Deleted,
    Modified,
    MovedIn,
    MovedOut,
}

#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub name: Option<String>,
}

pub struct ModuleWatcher {
    inotify_fd: RawFd,
    _watch_fd: i32,
    modules_dir: PathBuf,
}

impl ModuleWatcher {
    pub fn new(modules_dir: &Path) -> Result<Self> {
        let fd = unsafe { libc::inotify_init1(IN_NONBLOCK | IN_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error())
                .context("inotify_init1 failed");
        }

        let c_path = CString::new(modules_dir.as_os_str().as_encoded_bytes())
            .context("modules_dir contains null byte")?;

        let wd = unsafe {
            libc::inotify_add_watch(fd, c_path.as_ptr(), WATCH_MASK)
        };
        if wd < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd); }
            return Err(err).context("inotify_add_watch failed");
        }

        debug!(dir = %modules_dir.display(), "inotify watch established");

        Ok(Self {
            inotify_fd: fd,
            _watch_fd: wd,
            modules_dir: modules_dir.to_path_buf(),
        })
    }

    /// Poll for inotify events with a timeout. Returns empty vec on timeout.
    pub fn poll(&self, timeout_ms: i32) -> Result<Vec<WatchEvent>> {
        let mut pfd = libc::pollfd {
            fd: self.inotify_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if ret < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                return Ok(Vec::new());
            }
            return Err(err).context("poll on inotify fd");
        }
        if ret == 0 || (pfd.revents & libc::POLLIN) == 0 {
            return Ok(Vec::new());
        }

        self.read_events()
    }

    fn read_events(&self) -> Result<Vec<WatchEvent>> {
        let mut buf = [0u8; EVENT_BUF_SIZE];
        let len = unsafe {
            libc::read(
                self.inotify_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };

        if len < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::WouldBlock {
                return Ok(Vec::new());
            }
            return Err(err).context("reading inotify events");
        }

        let len = len as usize;
        let mut events = Vec::new();
        let mut offset = 0;

        while offset + INOTIFY_EVENT_HEADER_SIZE <= len {
            // inotify_event layout: wd(i32) + mask(u32) + cookie(u32) + len(u32)
            let mask = u32::from_ne_bytes([
                buf[offset + 4],
                buf[offset + 5],
                buf[offset + 6],
                buf[offset + 7],
            ]);
            let name_len = u32::from_ne_bytes([
                buf[offset + 12],
                buf[offset + 13],
                buf[offset + 14],
                buf[offset + 15],
            ]) as usize;

            let name = if name_len > 0 && offset + INOTIFY_EVENT_HEADER_SIZE + name_len <= len {
                let name_bytes = &buf[offset + INOTIFY_EVENT_HEADER_SIZE
                    ..offset + INOTIFY_EVENT_HEADER_SIZE + name_len];
                // Name is null-padded
                let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_len);
                String::from_utf8_lossy(&name_bytes[..end]).into_owned().into()
            } else {
                None
            };

            let kind = if mask & IN_CREATE != 0 {
                WatchEventKind::Created
            } else if mask & IN_DELETE != 0 {
                WatchEventKind::Deleted
            } else if mask & IN_MOVED_TO != 0 {
                WatchEventKind::MovedIn
            } else if mask & IN_MOVED_FROM != 0 {
                WatchEventKind::MovedOut
            } else if mask & IN_MODIFY != 0 {
                WatchEventKind::Modified
            } else {
                offset += INOTIFY_EVENT_HEADER_SIZE + name_len;
                continue;
            };

            events.push(WatchEvent { kind, name });
            offset += INOTIFY_EVENT_HEADER_SIZE + name_len;
        }

        Ok(events)
    }

    /// Event loop: poll for changes, invoke callback on each batch.
    /// Runs until the callback returns Err or the process is signaled.
    pub fn run_loop(&self, mut on_change: impl FnMut(Vec<WatchEvent>) -> Result<()>) -> Result<()> {
        debug!(dir = %self.modules_dir.display(), "module watcher loop started");

        loop {
            if crate::utils::signal::shutdown_requested() {
                debug!("shutdown requested, exiting watcher loop");
                return Ok(());
            }
            let events = self.poll(10_000)?;
            if events.is_empty() {
                continue;
            }

            let mut all_events = events;
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(DEBOUNCE_MS);

            loop {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let more = self.poll(remaining.as_millis() as i32)?;
                if more.is_empty() {
                    break;
                }
                all_events.extend(more);
            }

            debug!(count = all_events.len(), "module change events coalesced");
            on_change(all_events)?;
        }
    }
}

impl Drop for ModuleWatcher {
    fn drop(&mut self) {
        unsafe { libc::close(self.inotify_fd); }
    }
}

/// Fallback watcher using directory mtime polling.
/// Used when inotify_init1 fails (ENOSYS on some kernels, or EMFILE).
pub fn start_watcher_fallback(
    modules_dir: &Path,
    interval_secs: u64,
    mut on_change: impl FnMut() -> Result<()>,
) -> Result<()> {
    debug!(
        dir = %modules_dir.display(),
        interval_secs,
        "fallback mtime polling started"
    );

    let mut last_mtime = dir_mtime(modules_dir);

    loop {
        if crate::utils::signal::shutdown_requested() {
            debug!("shutdown requested, exiting fallback watcher");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_secs));

        let current_mtime = dir_mtime(modules_dir);
        if current_mtime != last_mtime {
            debug!("mtime change detected on {}", modules_dir.display());
            last_mtime = current_mtime;
            on_change()?;
        }
    }
}

fn dir_mtime(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e)
        }))
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Update RuntimeState timestamp and write status JSON.
/// Called by the watcher callback after detecting module changes.
pub fn touch_status_timestamp(state: &mut RuntimeState) {
    state.timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let status_path = Path::new(STATUS_JSON_PATH);
    let tmp_path = status_path.with_extension("json.tmp");

    if let Some(parent) = status_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match state.write_status_file(&tmp_path) {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp_path, status_path) {
                warn!("atomic rename failed, trying direct write: {e}");
                let _ = state.write_status_file(status_path);
            }
        }
        Err(e) => {
            warn!("status JSON write failed: {e}");
        }
    }
}

/// Start the module watcher with automatic inotify/polling fallback.
/// Intended to be called from the `mount --post-boot` handler.
pub fn start_module_watcher(
    modules_dir: &Path,
    mut on_change: impl FnMut() -> Result<()>,
) -> Result<()> {
    match ModuleWatcher::new(modules_dir) {
        Ok(watcher) => {
            watcher.run_loop(|events| {
                for ev in &events {
                    let name = ev.name.as_deref().unwrap_or("<unknown>");
                    tracing::debug!(kind = ?ev.kind, name, "module watcher event");
                }
                on_change()
            })
        }
        Err(e) => {
            warn!("inotify init failed, using polling fallback: {e}");
            start_watcher_fallback(modules_dir, 10, on_change)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn watch_event_kinds_distinct() {
        assert_ne!(WatchEventKind::Created, WatchEventKind::Deleted);
        assert_ne!(WatchEventKind::MovedIn, WatchEventKind::MovedOut);
        assert_ne!(WatchEventKind::Modified, WatchEventKind::Created);
    }

    #[test]
    fn dir_mtime_returns_zero_for_missing_path() {
        let mtime = dir_mtime(Path::new("/nonexistent_watcher_test_path"));
        assert_eq!(mtime, 0);
    }

    #[test]
    fn dir_mtime_returns_nonzero_for_real_dir() {
        let mtime = dir_mtime(Path::new("/tmp"));
        assert!(mtime > 0);
    }

    #[test]
    fn watcher_new_succeeds_on_real_dir() {
        let tmpdir = std::env::temp_dir().join("zeromount_watcher_test");
        let _ = fs::create_dir_all(&tmpdir);

        let watcher = ModuleWatcher::new(&tmpdir);
        assert!(watcher.is_ok(), "inotify watch on tmpdir should succeed");

        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[test]
    fn watcher_poll_returns_empty_on_timeout() {
        let tmpdir = std::env::temp_dir().join("zeromount_watcher_poll_test");
        let _ = fs::create_dir_all(&tmpdir);

        let watcher = ModuleWatcher::new(&tmpdir).expect("watcher init");
        let events = watcher.poll(10).expect("poll should succeed");
        assert!(events.is_empty(), "no events expected within 10ms");

        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[test]
    fn watcher_detects_file_creation() {
        let tmpdir = std::env::temp_dir().join("zeromount_watcher_create_test");
        let _ = fs::remove_dir_all(&tmpdir);
        fs::create_dir_all(&tmpdir).expect("create tmpdir");

        let watcher = ModuleWatcher::new(&tmpdir).expect("watcher init");

        // Create a file to trigger an event
        fs::write(tmpdir.join("test_module"), "test").expect("write file");

        // Short delay for inotify delivery
        std::thread::sleep(std::time::Duration::from_millis(50));

        let events = watcher.poll(100).expect("poll");
        assert!(!events.is_empty(), "should detect file creation");
        assert!(
            events.iter().any(|e| e.kind == WatchEventKind::Created),
            "should have a Created event"
        );

        let _ = fs::remove_dir_all(&tmpdir);
    }

    #[test]
    fn touch_status_timestamp_updates_time() {
        let mut state = RuntimeState::default();
        assert_eq!(state.timestamp, 0);

        touch_status_timestamp(&mut state);
        assert!(state.timestamp > 0, "timestamp should be updated");
    }
}
