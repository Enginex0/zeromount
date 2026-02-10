use std::ffi::CString;
use std::path::Path;

// Best-effort SELinux context copy. Falls back to u:object_r:system_file:s0.
pub fn mirror_selinux_context(source: &Path, dest: &Path) {
    let src_c = match CString::new(source.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let dst_c = match CString::new(dest.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };

    let attr = b"security.selinux\0";
    let attr_ptr = attr.as_ptr() as *const libc::c_char;

    if source.exists() {
        unsafe {
            let size = libc::getxattr(src_c.as_ptr(), attr_ptr, std::ptr::null_mut(), 0);
            if size > 0 {
                let mut buf = vec![0u8; size as usize];
                let read = libc::getxattr(
                    src_c.as_ptr(),
                    attr_ptr,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                );
                if read > 0 {
                    libc::setxattr(
                        dst_c.as_ptr(),
                        attr_ptr,
                        buf.as_ptr() as *const libc::c_void,
                        read as usize,
                        0,
                    );
                    return;
                }
            }
        }
    }

    let context = b"u:object_r:system_file:s0\0";
    unsafe {
        libc::setxattr(
            dst_c.as_ptr(),
            attr_ptr,
            context.as_ptr() as *const libc::c_void,
            context.len() - 1,
            0,
        );
    }
}
