use sysinfo::{System, Disks, Networks};

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

        // Convert bytes/s to human-readable
        Self {
            ingres: format_bytes_per_sec(total_rx),
            egres: format_bytes_per_sec(total_tx),
        }
    }
}

/// Format bytes per second into a human-readable string.
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

/// Format seconds into a human-readable uptime string like "47 days, 3 hours, 22 minutes".
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
