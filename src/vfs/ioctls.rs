use std::ffi::CString;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::{IoctlData, IoctlError, VfsRule, VfsStatus};

const DEVICE_PATH: &str = "/dev/zeromount";

// -- Ioctl number computation --
//
// Linux ioctl encoding: direction(2) | size(14) | type(8) | nr(8)
// _IO:  direction = 0 (none)
// _IOW: direction = 1 (write from userspace to kernel)
// _IOR: direction = 2 (read from kernel to userspace)

const IOC_NRBITS: u32 = 8;
const IOC_TYPEBITS: u32 = 8;
const IOC_SIZEBITS: u32 = 14;

const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

const ZEROMOUNT_MAGIC: u32 = 0x5A; // ASCII 'Z'

const fn ioc(dir: u32, typ: u32, nr: u32, size: u32) -> u32 {
    (dir << IOC_DIRSHIFT) | (typ << IOC_TYPESHIFT) | (nr << IOC_NRSHIFT) | (size << IOC_SIZESHIFT)
}

const fn io(typ: u32, nr: u32) -> u32 {
    ioc(IOC_NONE, typ, nr, 0)
}

const fn iow(typ: u32, nr: u32, size: u32) -> u32 {
    ioc(IOC_WRITE, typ, nr, size)
}

const fn ior(typ: u32, nr: u32, size: u32) -> u32 {
    ioc(IOC_READ, typ, nr, size)
}

// struct zeromount_ioctl_data size is pointer-width-dependent
const IOCTL_DATA_SIZE: u32 = std::mem::size_of::<IoctlData>() as u32;

// All 10 kernel-defined ioctl commands + proposed GET_STATUS
pub const IOCTL_ADD_RULE: u32 = iow(ZEROMOUNT_MAGIC, 1, IOCTL_DATA_SIZE);
pub const IOCTL_DEL_RULE: u32 = iow(ZEROMOUNT_MAGIC, 2, IOCTL_DATA_SIZE);
pub const IOCTL_CLEAR_ALL: u32 = io(ZEROMOUNT_MAGIC, 3);
pub const IOCTL_GET_VERSION: u32 = ior(ZEROMOUNT_MAGIC, 4, 4); // _IOR(..., int) = 4 bytes
pub const IOCTL_ADD_UID: u32 = iow(ZEROMOUNT_MAGIC, 5, 4); // _IOW(..., unsigned int)
pub const IOCTL_DEL_UID: u32 = iow(ZEROMOUNT_MAGIC, 6, 4);
pub const IOCTL_GET_LIST: u32 = ior(ZEROMOUNT_MAGIC, 7, 4); // _IOR(..., int)
pub const IOCTL_ENABLE: u32 = io(ZEROMOUNT_MAGIC, 8);
pub const IOCTL_DISABLE: u32 = io(ZEROMOUNT_MAGIC, 9);
pub const IOCTL_REFRESH: u32 = io(ZEROMOUNT_MAGIC, 10);
pub const IOCTL_GET_STATUS: u32 = ior(ZEROMOUNT_MAGIC, 11, 4); // proposed, may not exist

// Compile-time verification of ioctl numbers for ARM64
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(IOCTL_ADD_RULE == 0x40185A01);
    assert!(IOCTL_DEL_RULE == 0x40185A02);
    assert!(IOCTL_CLEAR_ALL == 0x5A03);
    assert!(IOCTL_GET_VERSION == 0x80045A04);
    assert!(IOCTL_ADD_UID == 0x40045A05);
    assert!(IOCTL_DEL_UID == 0x40045A06);
    assert!(IOCTL_GET_LIST == 0x80045A07);
    assert!(IOCTL_ENABLE == 0x5A08);
    assert!(IOCTL_DISABLE == 0x5A09);
    assert!(IOCTL_REFRESH == 0x5A0A);
    assert!(IOCTL_GET_STATUS == 0x80045A0B);
};

// Compile-time verification for ARM32
#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(IOCTL_ADD_RULE == 0x400C5A01);
    assert!(IOCTL_DEL_RULE == 0x400C5A02);
    assert!(IOCTL_CLEAR_ALL == 0x5A03);
    assert!(IOCTL_GET_VERSION == 0x80045A04);
    assert!(IOCTL_ADD_UID == 0x40045A05);
    assert!(IOCTL_DEL_UID == 0x40045A06);
    assert!(IOCTL_GET_LIST == 0x80045A07);
    assert!(IOCTL_ENABLE == 0x5A08);
    assert!(IOCTL_DISABLE == 0x5A09);
    assert!(IOCTL_REFRESH == 0x5A0A);
    assert!(IOCTL_GET_STATUS == 0x80045A0B);
};

/// Raw ioctl wrapper that returns the kernel's return value or an IoctlError.
unsafe fn raw_ioctl(fd: i32, request: u32, arg: *mut libc::c_void) -> Result<i32, IoctlError> {
    let ret = libc::ioctl(fd, request as libc::Ioctl, arg);
    if ret < 0 {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
        Err(IoctlError::IoctlFailed {
            name: ioctl_name(request),
            msg: std::io::Error::from_raw_os_error(errno).to_string(),
            errno,
        })
    } else {
        Ok(ret)
    }
}

fn ioctl_name(code: u32) -> &'static str {
    match code {
        c if c == IOCTL_ADD_RULE => "ADD_RULE",
        c if c == IOCTL_DEL_RULE => "DEL_RULE",
        c if c == IOCTL_CLEAR_ALL => "CLEAR_ALL",
        c if c == IOCTL_GET_VERSION => "GET_VERSION",
        c if c == IOCTL_ADD_UID => "ADD_UID",
        c if c == IOCTL_DEL_UID => "DEL_UID",
        c if c == IOCTL_GET_LIST => "GET_LIST",
        c if c == IOCTL_ENABLE => "ENABLE",
        c if c == IOCTL_DISABLE => "DISABLE",
        c if c == IOCTL_REFRESH => "REFRESH",
        c if c == IOCTL_GET_STATUS => "GET_STATUS",
        _ => "UNKNOWN",
    }
}

// -- VfsDriver --

pub struct VfsDriver {
    fd: std::fs::File,
}

impl VfsDriver {
    pub fn open() -> Result<Self, IoctlError> {
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open(DEVICE_PATH)
            .map_err(|e| {
                let errno = e.raw_os_error().unwrap_or(0);
                IoctlError::OpenFailed(e.to_string(), errno)
            })?;
        Ok(Self { fd })
    }

    fn raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    /// Inject a VFS redirection rule into the kernel driver.
    pub fn add_rule(&self, source: &Path, target: &Path, is_dir: bool) -> Result<()> {
        let rule = VfsRule::new(source, target, is_dir)?;
        let mut data = rule.as_ioctl_data();
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_ADD_RULE,
                &mut data as *mut IoctlData as *mut libc::c_void,
            )?;
        }
        Ok(())
    }

    /// Delete a VFS rule by virtual path.
    pub fn del_rule(&self, source: &Path, target: &Path) -> Result<()> {
        let vp = CString::new(
            source
                .to_str()
                .ok_or_else(|| IoctlError::InvalidPath(source.display().to_string()))?,
        )
        .map_err(|_| IoctlError::InvalidPath(source.display().to_string()))?;

        let rp = CString::new(
            target
                .to_str()
                .ok_or_else(|| IoctlError::InvalidPath(target.display().to_string()))?,
        )
        .map_err(|_| IoctlError::InvalidPath(target.display().to_string()))?;

        let mut data = IoctlData {
            virtual_path: vp.as_ptr(),
            real_path: rp.as_ptr(),
            flags: 0,
            #[cfg(target_pointer_width = "64")]
            _pad: 0,
        };
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_DEL_RULE,
                &mut data as *mut IoctlData as *mut libc::c_void,
            )?;
        }
        Ok(())
    }

    /// Clear all rules. NOTE: leaks dirs_ht entries per CO01.
    pub fn clear_all(&self) -> Result<()> {
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_CLEAR_ALL,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    /// Query driver version. Only ioctl NOT requiring CAP_SYS_ADMIN.
    pub fn get_version(&self) -> Result<u32> {
        let mut version: i32 = 0;
        let ret = unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_GET_VERSION,
                &mut version as *mut i32 as *mut libc::c_void,
            )?
        };
        // Kernel returns version as ioctl return value, not buffer
        let ver = if ret > 0 { ret as u32 } else { version as u32 };
        Ok(ver)
    }

    /// Exclude a UID from VFS redirection.
    pub fn add_uid(&self, uid: u32) -> Result<()> {
        let mut uid_val = uid;
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_ADD_UID,
                &mut uid_val as *mut u32 as *mut libc::c_void,
            )?;
        }
        Ok(())
    }

    /// Re-include a previously excluded UID.
    pub fn del_uid(&self, uid: u32) -> Result<()> {
        let mut uid_val = uid;
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_DEL_UID,
                &mut uid_val as *mut u32 as *mut libc::c_void,
            )?;
        }
        Ok(())
    }

    /// List current VFS rules. Returns raw text from kernel.
    pub fn get_list(&self) -> Result<String> {
        // Kernel writes rule list into the provided buffer and returns byte count
        let mut buf = vec![0u8; 65536];
        let ret = unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_GET_LIST,
                buf.as_mut_ptr() as *mut libc::c_void,
            )?
        };
        let len = ret as usize;
        buf.truncate(len);
        String::from_utf8(buf).context("kernel returned non-UTF8 rule list")
    }

    /// Enable the VFS engine.
    pub fn enable(&self) -> Result<()> {
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_ENABLE,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    /// Disable the VFS engine.
    pub fn disable(&self) -> Result<()> {
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_DISABLE,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    /// Force dcache refresh after rule changes. Was missing from zm.c (BUG-M1).
    pub fn refresh(&self) -> Result<()> {
        unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_REFRESH,
                std::ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    /// Query engine status. Returns None if kernel lacks GET_STATUS (old kernel).
    /// Backward-compatible: ENOTTY or EINVAL means the ioctl doesn't exist.
    pub fn get_status(&self) -> Result<Option<VfsStatus>> {
        let mut status_val: i32 = 0;
        let result = unsafe {
            raw_ioctl(
                self.raw_fd(),
                IOCTL_GET_STATUS,
                &mut status_val as *mut i32 as *mut libc::c_void,
            )
        };
        match result {
            Ok(ret) => Ok(Some(VfsStatus {
                enabled: ret != 0,
                rule_count: status_val as u32,
            })),
            Err(IoctlError::IoctlFailed { errno, .. })
                if errno == libc::ENOTTY || errno == libc::EINVAL =>
            {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_data_size_arm64() {
        // On the host (likely x86_64, same pointer width as ARM64)
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<IoctlData>(), 24);
    }

    #[test]
    fn ioctl_numbers_no_data() {
        assert_eq!(IOCTL_CLEAR_ALL, 0x5A03);
        assert_eq!(IOCTL_ENABLE, 0x5A08);
        assert_eq!(IOCTL_DISABLE, 0x5A09);
        assert_eq!(IOCTL_REFRESH, 0x5A0A);
    }

    #[test]
    fn ioctl_numbers_with_int() {
        assert_eq!(IOCTL_GET_VERSION, 0x80045A04);
        assert_eq!(IOCTL_ADD_UID, 0x40045A05);
        assert_eq!(IOCTL_DEL_UID, 0x40045A06);
        assert_eq!(IOCTL_GET_LIST, 0x80045A07);
        assert_eq!(IOCTL_GET_STATUS, 0x80045A0B);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn ioctl_numbers_with_struct_arm64() {
        assert_eq!(IOCTL_ADD_RULE, 0x40185A01);
        assert_eq!(IOCTL_DEL_RULE, 0x40185A02);
    }
}
