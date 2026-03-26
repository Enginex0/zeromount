use std::fs;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::Path;

const FICLONE: libc::c_ulong = 0x4004_9409;

pub fn copy_file(src: &Path, dst: &Path) -> io::Result<u64> {
    let src_file = fs::File::open(src)?;
    let dst_file = fs::File::create(dst)?;

    // SAFETY: Both fds are valid open files. FICLONE is a well-defined ioctl
    // that performs CoW reflink on supporting filesystems (f2fs, btrfs, xfs).
    // On unsupported filesystems it returns EOPNOTSUPP/EXDEV/EINVAL.
    let ret = unsafe { libc::ioctl(dst_file.as_raw_fd(), FICLONE as _, src_file.as_raw_fd()) };

    if ret == 0 {
        let meta = src_file.metadata()?;
        let perms = meta.permissions();
        dst_file.set_permissions(perms)?;
        return Ok(meta.len());
    }

    drop(src_file);
    drop(dst_file);
    fs::copy(src, dst)
}
