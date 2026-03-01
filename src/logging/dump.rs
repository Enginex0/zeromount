use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Write};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rand::Rng;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const DUMP_DIR: &str = "/sdcard/Download";
const LOCK_PATH: &str = "/data/adb/zeromount/.dump_lock";
const DUMP_PATH_FILE: &str = "/data/adb/zeromount/.dump_path";
const DMESG_SIZE_LIMIT: usize = 2 * 1024 * 1024;

pub fn execute_dump() -> Result<()> {
    let lock = acquire_flock(LOCK_PATH)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let config = crate::core::config::ZeroMountConfig::load(None).unwrap_or_default();
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buf);
    let mut manifest_files: Vec<serde_json::Value> = Vec::new();

    add_log_files(&mut zip, opts, &config.logging.log_dir, &mut manifest_files)?;
    add_entry(&mut zip, opts, "dmesg-zeromount.log", &collect_dmesg("zeromount")?, &mut manifest_files)?;
    add_entry(&mut zip, opts, "dmesg-susfs.log", &collect_dmesg("susfs")?, &mut manifest_files)?;
    add_entry(&mut zip, opts, "logcat-zeromount.log", &collect_logcat(), &mut manifest_files)?;
    add_entry(&mut zip, opts, "zygisk-status.txt",
        &fs::read("/data/adb/zeromount/flags/zygisk_status").unwrap_or_default(),
        &mut manifest_files)?;
    add_entry(&mut zip, opts, "config.toml", toml::to_string_pretty(&config)?.as_bytes(), &mut manifest_files)?;
    add_entry(&mut zip, opts, "sysfs-level.txt", collect_sysfs_level().as_bytes(), &mut manifest_files)?;
    add_entry(&mut zip, opts, "susfs-probe.txt", collect_susfs_probe().as_bytes(), &mut manifest_files)?;
    add_entry(&mut zip, opts, "device-info.txt", collect_device_info().as_bytes(), &mut manifest_files)?;

    let manifest = serde_json::json!({
        "timestamp": timestamp,
        "version": env!("CARGO_PKG_VERSION"),
        "files": manifest_files,
    });
    let manifest_str = serde_json::to_string_pretty(&manifest)?;
    add_entry(&mut zip, opts, "manifest.json", manifest_str.as_bytes(), &mut manifest_files)?;

    zip.finish()?;

    let zip_bytes = buf.into_inner();
    let zip_name = format!("{}.zip", random_name(8));
    let zip_path = Path::new(DUMP_DIR).join(&zip_name);
    fs::create_dir_all(DUMP_DIR).context("cannot create Download dir")?;
    fs::write(&zip_path, &zip_bytes).with_context(|| format!("cannot write {}", zip_path.display()))?;

    let zip_path_str = zip_path.to_string_lossy().to_string();
    fs::write(DUMP_PATH_FILE, &zip_path_str).context("cannot write .dump_path")?;

    println!("{}", serde_json::json!({
        "zip": zip_path_str,
        "size": zip_bytes.len(),
        "files": manifest_files.len(),
    }));

    drop(lock);
    Ok(())
}

fn add_entry(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    opts: SimpleFileOptions,
    name: &str,
    data: &[u8],
    manifest: &mut Vec<serde_json::Value>,
) -> Result<()> {
    zip.start_file(name, opts)?;
    zip.write_all(data)?;
    manifest.push(serde_json::json!({ "file": name, "bytes": data.len() }));
    Ok(())
}

fn add_log_files(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    opts: SimpleFileOptions,
    log_dir: &Path,
    manifest: &mut Vec<serde_json::Value>,
) -> Result<()> {
    for name in ["zeromount.log", "zeromount.log.1", "zeromount.log.2", "zeromount.log.3", "zeromount.log.4"] {
        let src = log_dir.join(name);
        if let Ok(data) = fs::read(&src) {
            add_entry(zip, opts, name, &data, manifest)?;
        }
    }
    Ok(())
}

fn collect_dmesg(filter: &str) -> Result<Vec<u8>> {
    let output = Command::new("dmesg").output().context("dmesg failed")?;
    let filter_bytes = filter.as_bytes();
    let mut buf = Vec::new();
    for line in output.stdout.split(|&b| b == b'\n') {
        let lower = line.to_ascii_lowercase();
        if lower.windows(filter_bytes.len()).any(|w| w == filter_bytes) {
            buf.extend_from_slice(line);
            buf.push(b'\n');
            if buf.len() >= DMESG_SIZE_LIMIT { break; }
        }
    }
    Ok(buf)
}

fn collect_logcat() -> Vec<u8> {
    Command::new("logcat")
        .args(["-d", "-s", "ZeroMount-Settings:*"])
        .output()
        .map(|o| {
            let mut buf = o.stdout;
            buf.truncate(DMESG_SIZE_LIMIT);
            buf
        })
        .unwrap_or_default()
}

fn collect_sysfs_level() -> String {
    match super::sysfs::read_kernel_debug_level() {
        Ok(level) => format!("kernel_debug_level={level}\n"),
        Err(e) => format!("kernel_debug_level=unavailable ({e})\n"),
    }
}

fn collect_susfs_probe() -> String {
    match crate::susfs::SusfsClient::probe() {
        Ok(client) => {
            let f = client.features();
            format!(
                "available={}\nversion={}\nkstat={}\npath={}\nmaps={}\nkstat_redirect={}\n",
                client.is_available(),
                client.version().unwrap_or("unknown"),
                f.kstat, f.path, f.maps, f.kstat_redirect,
            )
        }
        Err(e) => format!("probe_error={e}\n"),
    }
}

fn collect_device_info() -> String {
    let run = |cmd: &str, args: &[&str]| -> String {
        Command::new(cmd).args(args).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };

    let uname = run("uname", &["-a"]);
    let device = run("getprop", &["ro.product.device"]);
    let build = run("getprop", &["ro.build.display.id"]);
    let android = run("getprop", &["ro.build.version.release"]);
    let ksu_ver = fs::read_to_string("/data/adb/ksu/version").unwrap_or_default();
    let module_ver = fs::read_to_string("/data/adb/modules/meta-zeromount/module.prop")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("version="))
        .map(|l| l.trim_start_matches("version=").to_string())
        .unwrap_or_default();

    format!(
        "uname={uname}\ndevice={device}\nbuild={build}\nandroid={android}\nksu={}\nmodule={module_ver}\n",
        ksu_ver.trim(),
    )
}

fn random_name(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..36u8);
            (if idx < 10 { b'0' + idx } else { b'a' + idx - 10 }) as char
        })
        .collect()
}

struct FlockGuard {
    #[allow(dead_code)]
    file: File,
}

fn acquire_flock(path: &str) -> Result<FlockGuard> {
    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .with_context(|| format!("cannot open lock file {path}"))?;

    let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
    let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if ret != 0 {
        anyhow::bail!("zm log dump already running (flock on {path} failed)");
    }
    Ok(FlockGuard { file })
}
