use sysinfo::{Disks, Networks, System};
use std::collections::HashSet;

/// System overview information.
pub struct SystemInfo {
    pub os: String,
    pub kernel: String,
    pub architecture: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu: String,
    pub cpu_cores: u16,
    pub load_average: String,
    pub used_storage: u64,
    pub total_storage: u64,
}

impl SystemInfo {
    /// Gather system information.
    pub fn gather(server_name: &str) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let os = System::name().unwrap_or_else(|| "unknown".to_string());
        let kernel = System::os_version().unwrap_or_else(|| "unknown".to_string());
        let architecture = std::env::consts::ARCH.to_string();
        let hostname = System::host_name().unwrap_or_else(|| server_name.to_string());

        let uptime_secs = System::uptime();
        let uptime = format_uptime(uptime_secs);

        let cpu = sys.cpus().first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let cpu_cores = sys.cpus().len() as u16;

        let load = System::load_average();
        let load_average = format!("{:.2} {:.2} {:.2}", load.one, load.five, load.fifteen);

        let disks = Disks::new_with_refreshed_list();
        let mut used_storage = 0u64;
        let mut total_storage = 0u64;
        for disk in &disks {
            total_storage += disk.total_space();
            used_storage += disk.total_space() - disk.available_space();
        }

        Self {
            os,
            kernel,
            architecture,
            hostname,
            uptime,
            cpu,
            cpu_cores,
            load_average,
            used_storage,
            total_storage,
        }
    }

    /// Compute total storage used by ppdrive data directory.
    pub fn storage_used(root_dir: &std::path::Path) -> u64 {
        let mut size = 0u64;
        let _ = Self::dir_size(root_dir, &mut size);
        size
    }

    fn dir_size(path: &std::path::Path, size: &mut u64) -> anyhow::Result<()> {
        if path.is_file() {
            *size += std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            return Ok(());
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                Self::dir_size(&entry.path(), size)?;
            }
        }
        Ok(())
    }
}

/// Network throughput snapshot.
pub struct NetworkThroughput {
    pub ingres: String,
    pub egres: String,
}

impl NetworkThroughput {
    /// Sample network throughput over a 1-second interval.
    pub fn sample() -> Self {
        let mut networks = Networks::new_with_refreshed_list();
        std::thread::sleep(std::time::Duration::from_millis(500));
        networks.refresh(true);

        let mut total_rx = 0u64;
        let mut total_tx = 0u64;
        for (_name, data) in &networks {
            total_rx += data.total_received();
            total_tx += data.total_transmitted();
        }

        Self {
            ingres: format_bytes_per_sec(total_rx),
            egres: format_bytes_per_sec(total_tx),
        }
    }
}

fn format_bytes_per_sec(bps: u64) -> String {
    if bps < 1024 {
        format!("{bps} B/s")
    } else if bps < 1024 * 1024 {
        format!("{:.1} KB/s", bps as f64 / 1024.0)
    } else if bps < 1024 * 1024 * 1024 {
        format!("{:.1} MB/s", bps as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bps as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Info about a single mounted filesystem.
pub struct MountedDeviceInfo {
    pub mount_path: String,
    pub device: String,
    pub fs_type: String,
    pub total: String,
    pub used: String,
    pub free: String,
}

/// Gather info about all real mounted filesystems.
/// Platform-specific: Linux reads /proc/mounts, macOS uses getmntinfo(),
/// Windows enumerates drive letters.
pub fn mounted_devices() -> Vec<MountedDeviceInfo> {
    mounted_devices_inner()
}

// ─── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn mounted_devices_inner() -> Vec<MountedDeviceInfo> {
    let mounts = parse_proc_mounts();
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for (device, mount_path, fs_type) in mounts {
        if seen.contains(&mount_path) {
            continue;
        }
        seen.insert(mount_path.clone());

        match statvfs(&mount_path) {
            Some((total, available)) => {
                let used = total - available;
                result.push(MountedDeviceInfo {
                    mount_path,
                    device,
                    fs_type,
                    total: format_bytes(total),
                    used: format_bytes(used),
                    free: format_bytes(available),
                });
            }
            None => {
                result.push(MountedDeviceInfo {
                    mount_path,
                    device,
                    fs_type,
                    total: "N/A".into(),
                    used: "N/A".into(),
                    free: "N/A".into(),
                });
            }
        }
    }

    result
}

#[cfg(target_os = "linux")]
fn parse_proc_mounts() -> Vec<(String, String, String)> {
    let Ok(content) = std::fs::read_to_string("/proc/mounts") else {
        return Vec::new();
    };

    let virtual_fs: HashSet<&str> = [
        "proc", "sysfs", "devpts", "tmpfs", "cgroup", "cgroup2",
        "pstore", "securityfs", "debugfs", "tracefs", "fusectl",
        "configfs", "hugetlbfs", "mqueue", "autofs", "overlay",
        "nsfs", "bpf", "rpc_pipefs", "nfsd", "efivarfs",
    ]
    .into_iter()
    .collect();

    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return None;
            }
            let device = parts[0].to_string();
            let mount_path = parts[1].to_string();
            let fs_type = parts[2].to_string();

            if !device.starts_with("/dev/") {
                return None;
            }
            if virtual_fs.contains(fs_type.as_str()) {
                return None;
            }

            Some((device, mount_path, fs_type))
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn statvfs(path: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;

    let c_path = CString::new(path).ok()?;
    let mut buf: libc::statvfs = unsafe { std::mem::zeroed() };

    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut buf) };
    if ret != 0 {
        return None;
    }

    let block_size = buf.f_frsize as u64;
    let total = buf.f_blocks as u64 * block_size;
    let available = buf.f_bavail as u64 * block_size;

    Some((total, available))
}

// ─── macOS / iOS ─────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn mounted_devices_inner() -> Vec<MountedDeviceInfo> {
    use std::ffi::CStr;

    let mut count: libc::c_int = 0;
    let mut mntp: *mut libc::statfs = std::ptr::null_mut();

    let ret = unsafe { libc::getmntinfo(&mut mntp, libc::MNT_NOWAIT) };
    if ret == 0 || mntp.is_null() {
        return Vec::new();
    }
    let count = ret;

    let virtual_fs: HashSet<&str> = [
        "devfs", "fdesc", "nullfs", "specfs", "tun",
        "autofs", "map", "misfs", "sharefs",
    ]
    .into_iter()
    .collect();

    let mut result = Vec::new();
    let mounts = unsafe { std::slice::from_raw_parts(mntp, count as usize) };

    for fs in mounts {
        let device = unsafe { CStr::from_ptr(fs.f_mntfromname.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let mount_path = unsafe { CStr::from_ptr(fs.f_mntonname.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let fs_type = unsafe { CStr::from_ptr(fs.f_fstypename.as_ptr()) }
            .to_string_lossy()
            .to_string();

        if !device.starts_with("/dev/disk") {
            continue;
        }
        if virtual_fs.contains(fs_type.as_str()) {
            continue;
        }

        let block_size = fs.f_bsize as u64;
        let total = fs.f_blocks as u64 * block_size;
        let available = fs.f_bavail as u64 * block_size;
        let used = total - available;

        result.push(MountedDeviceInfo {
            mount_path,
            device,
            fs_type,
            total: format_bytes(total),
            used: format_bytes(used),
            free: format_bytes(available),
        });
    }

    result
}

// ─── FreeBSD / OpenBSD / NetBSD ──────────────────────────────────────────────

#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
fn mounted_devices_inner() -> Vec<MountedDeviceInfo> {
    use std::ffi::CStr;

    let mut count: libc::c_int = 0;
    let mut mntp: *mut libc::statfs = std::ptr::null_mut();

    let ret = unsafe { libc::getmntinfo(&mut mntp, libc::MNT_NOWAIT) };
    if ret == 0 || mntp.is_null() {
        return Vec::new();
    }
    let count = ret;

    let mut result = Vec::new();
    let mounts = unsafe { std::slice::from_raw_parts(mntp, count as usize) };

    for fs in mounts {
        let device = unsafe { CStr::from_ptr(fs.f_mntfromname.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let mount_path = unsafe { CStr::from_ptr(fs.f_mntonname.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let fs_type = unsafe { CStr::from_ptr(fs.f_fstypename.as_ptr()) }
            .to_string_lossy()
            .to_string();

        if !device.starts_with("/dev/") {
            continue;
        }

        let block_size = fs.f_bsize as u64;
        let total = fs.f_blocks as u64 * block_size;
        let available = fs.f_bavail as u64 * block_size;
        let used = total - available;

        result.push(MountedDeviceInfo {
            mount_path,
            device,
            fs_type,
            total: format_bytes(total),
            used: format_bytes(used),
            free: format_bytes(available),
        });
    }

    result
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn mounted_devices_inner() -> Vec<MountedDeviceInfo> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let mut result = Vec::new();
    let drives = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };

    for i in 0..26 {
        if drives & (1 << i) == 0 {
            continue;
        }

        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let root_wide: Vec<u16> = OsStr::new(&root)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Get volume name and fs type
        let mut volume_name_buf = [0u16; 256];
        let mut fs_type_buf = [0u16; 256];
        let mut serial: u32 = 0;
        let mut max_comp: u32 = 0;
        let mut flags: u32 = 0;

        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
                root_wide.as_ptr(),
                volume_name_buf.as_mut_ptr(),
                volume_name_buf.len() as i32,
                Some(&mut serial),
                Some(&mut max_comp),
                Some(&mut flags),
                fs_type_buf.as_mut_ptr(),
                fs_type_buf.len() as i32,
            )
        };

        if ok == 0 {
            continue;
        }

        let device = osstr_from_wide(&volume_name_buf)
            .to_string_lossy()
            .to_string();
        let fs_type = osstr_from_wide(&fs_type_buf)
            .to_string_lossy()
            .to_string();

        if fs_type.is_empty() {
            continue;
        }

        // Get free space
        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut avail_bytes: u64 = 0;

        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                root_wide.as_ptr(),
                &mut avail_bytes,
                &mut total_bytes,
                Some(&mut free_bytes),
            )
        };

        if ok == 0 {
            continue;
        }

        let used = total_bytes - avail_bytes;

        result.push(MountedDeviceInfo {
            mount_path: root,
            device: if device.is_empty() { format!("{letter}:") } else { device },
            fs_type,
            total: format_bytes(total_bytes),
            used: format_bytes(used),
            free: format_bytes(avail_bytes),
        });
    }

    result
}

#[cfg(target_os = "windows")]
fn osstr_from_wide(buf: &[u16]) -> &OsStr {
    // Find null terminator
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsStr::from_wide(&buf[..len])
}

// ─── Fallback (unsupported platforms) ────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "windows")))]
fn mounted_devices_inner() -> Vec<MountedDeviceInfo> {
    Vec::new()
}

// ─── Shared helpers ──────────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024u64 * 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.1} TB", bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0))
    }
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days} day{}", if days == 1 { "" } else { "s" }));
    }
    if hours > 0 {
        parts.push(format!("{hours} hour{}", if hours == 1 { "" } else { "s" }));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{minutes} minute{}", if minutes == 1 { "" } else { "s" }));
    }
    parts.join(", ")
}
