use std::os::unix::io::RawFd;

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

use super::input::TouchDevice;
use super::recovery::FreqGuard;
use super::sysfs;

const MAX_EPOLL_EVENTS: usize = 8;
const EPOLL_TIMEOUT_MS: i32 = 1000;
const INPUT_BUF_SIZE: usize = 512;

const BOOST_DURATION_MS: u64 = 80;
const COOLDOWN_MS: u64 = 60;

pub fn run_boost_loop(
    touch_devices: &[TouchDevice],
    cluster_boosts: &[(String, u64)],
    freq_guard: &FreqGuard,
) -> Result<()> {
    let epoll_fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epoll_fd < 0 {
        return Err(std::io::Error::last_os_error()).context("epoll_create1");
    }
    let _epoll_guard = FdGuard(epoll_fd);

    let timer_fd = unsafe {
        libc::timerfd_create(
            libc::CLOCK_MONOTONIC,
            libc::TFD_NONBLOCK | libc::TFD_CLOEXEC,
        )
    };
    if timer_fd < 0 {
        return Err(std::io::Error::last_os_error()).context("timerfd_create");
    }
    let _timer_guard = FdGuard(timer_fd);

    epoll_add(epoll_fd, timer_fd)?;

    let mut input_fds: Vec<RawFd> = Vec::new();
    for dev in touch_devices {
        match open_input_device(&dev.event_path) {
            Ok(fd) => {
                epoll_add(epoll_fd, fd)?;
                input_fds.push(fd);
                info!(path = %dev.event_path, fd, "input device opened");
            }
            Err(e) => warn!(path = %dev.event_path, %e, "failed to open input device"),
        }
    }

    if input_fds.is_empty() {
        error!("no input devices could be opened, input boost disabled");
        return Ok(());
    }

    info!(
        input_devices = input_fds.len(),
        clusters = cluster_boosts.len(),
        boost_ms = BOOST_DURATION_MS,
        cooldown_ms = COOLDOWN_MS,
        "boost daemon started"
    );

    let mut events = [libc::epoll_event { events: 0, u64: 0 }; MAX_EPOLL_EVENTS];
    let mut last_boost_ns: u64 = 0;
    let cooldown_ns = COOLDOWN_MS * 1_000_000;
    let mut boost_count: u64 = 0;
    let mut last_stats_ns: u64 = monotonic_ns();

    loop {
        if crate::utils::signal::shutdown_requested() {
            info!(total_boosts = boost_count, "shutdown signal received");
            break;
        }

        let n = unsafe {
            libc::epoll_wait(
                epoll_fd,
                events.as_mut_ptr(),
                MAX_EPOLL_EVENTS as i32,
                EPOLL_TIMEOUT_MS,
            )
        };

        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(err).context("epoll_wait");
        }

        // Periodic health log every 5 minutes
        let now_ns = monotonic_ns();
        if now_ns.saturating_sub(last_stats_ns) >= 300_000_000_000 {
            info!(boosts = boost_count, alive_fds = input_fds.len(), "boost daemon alive");
            last_stats_ns = now_ns;
        }

        for i in 0..n as usize {
            let fd = events[i].u64 as RawFd;
            let ev_flags = events[i].events;

            // Handle disconnected input devices
            if (ev_flags & (libc::EPOLLERR | libc::EPOLLHUP) as u32) != 0 && fd != timer_fd {
                warn!(fd, "input device disconnected, removing from epoll");
                unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
                unsafe { libc::close(fd) };
                input_fds.retain(|&f| f != fd);
                if input_fds.is_empty() {
                    warn!("all input devices disconnected, exiting boost loop");
                    freq_guard.restore();
                    return Ok(());
                }
                continue;
            }

            if fd == timer_fd {
                drain_timerfd(timer_fd);
                freq_guard.restore();
                debug!("boost expired, frequencies restored");
                continue;
            }

            drain_input(fd);

            let now_ns = monotonic_ns();
            if now_ns.saturating_sub(last_boost_ns) < cooldown_ns {
                continue;
            }
            last_boost_ns = now_ns;

            apply_boost(cluster_boosts);
            arm_timer(timer_fd, BOOST_DURATION_MS);
            boost_count += 1;
            debug!(count = boost_count, "boost applied");
        }
    }

    freq_guard.restore();

    for fd in input_fds {
        unsafe { libc::close(fd) };
    }

    info!(total_boosts = boost_count, "boost loop exited cleanly");
    Ok(())
}

fn open_input_device(path: &str) -> Result<RawFd> {
    let c_path = std::ffi::CString::new(path)?;
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context(format!("open {path}"));
    }
    Ok(fd)
}

fn epoll_add(epoll_fd: RawFd, fd: RawFd) -> Result<()> {
    let mut ev = libc::epoll_event {
        events: libc::EPOLLIN as u32,
        u64: fd as u64,
    };
    let rc = unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut ev) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error()).context("epoll_ctl ADD");
    }
    Ok(())
}

fn arm_timer(timer_fd: RawFd, duration_ms: u64) {
    let ts = libc::itimerspec {
        it_interval: libc::timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: libc::timespec {
            tv_sec: (duration_ms / 1000) as _,
            tv_nsec: ((duration_ms % 1000) * 1_000_000) as _,
        },
    };
    unsafe { libc::timerfd_settime(timer_fd, 0, &ts, std::ptr::null_mut()) };
}

fn drain_input(fd: RawFd) {
    let mut buf = [0u8; INPUT_BUF_SIZE];
    loop {
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 {
            break;
        }
    }
}

fn drain_timerfd(fd: RawFd) {
    let mut val: u64 = 0;
    unsafe {
        libc::read(
            fd,
            &mut val as *mut u64 as *mut libc::c_void,
            std::mem::size_of::<u64>(),
        );
    }
}

fn apply_boost(cluster_boosts: &[(String, u64)]) {
    for (policy_path, boost_freq) in cluster_boosts {
        let path = format!("{policy_path}/scaling_min_freq");
        let _ = sysfs::sysfs_write(&path, &boost_freq.to_string());
    }
}

fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

struct FdGuard(RawFd);

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe { libc::close(self.0) };
        }
    }
}
