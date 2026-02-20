use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;

const LOCK_PATH: &str = "/data/adb/zeromount/.lock";

// Returns Some(file) if lock acquired, None if another instance holds it.
// The lock auto-releases when the File is dropped or the process exits.
pub fn acquire_instance_lock() -> anyhow::Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(LOCK_PATH)?;

    // SAFETY: fd is a valid open file descriptor from OpenOptions::open above.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        Ok(Some(file))
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
            Ok(None)
        } else {
            Err(err.into())
        }
    }
}
