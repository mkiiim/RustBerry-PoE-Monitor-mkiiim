use std::fs;
use std::time::{Duration, Instant};
use sysinfo::{System, Disks, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::{debug, info, warn};

const CPU_TEMP_PATH: &str = "/sys/class/thermal/thermal_zone0/temp";

/// One reading of the system stats shown on the display.
#[derive(Clone, Debug)]
pub struct Metrics {
    /// Overall CPU usage in percent.
    pub cpu_usage: f32,
    /// CPU temperature in °C, or `None` if it could not be read.
    pub cpu_temp: Option<f32>,
    /// Used RAM in percent.
    pub ram_usage: f64,
    /// Used disk space in percent, refreshed every `disk_interval`.
    pub disk_usage: f64,
}

/// Samples system stats. Disk usage is slower to read and changes slowly,
/// so it is only refreshed every `disk_interval`.
pub struct MetricsCollector {
    sys: System,
    disk_interval: Duration,
    last_disk_update: Option<Instant>,
    disk_usage: f64,
}

impl MetricsCollector {
    pub fn new(disk_interval: Duration) -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(MemoryRefreshKind::new().with_ram()),
        );

        debug!("System initialized. System info:");
        debug!("================================");
        debug!("System name:             {}", System::name().unwrap_or_default());
        debug!("System kernel version:   {}", System::kernel_version().unwrap_or_default());
        debug!("System OS version:       {}", System::os_version().unwrap_or_default());

        MetricsCollector { sys, disk_interval, last_disk_update: None, disk_usage: 0.0 }
    }

    pub fn sample(&mut self) -> Metrics {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();

        let disk_due = self.last_disk_update.is_none_or(|t| t.elapsed() >= self.disk_interval);
        if disk_due {
            self.last_disk_update = Some(Instant::now());
            self.disk_usage = disk_usage();
            info!("Updated disk usage: {:.1}", self.disk_usage);
        }

        Metrics {
            cpu_usage: self.sys.global_cpu_info().cpu_usage(),
            cpu_temp: cpu_temperature(),
            ram_usage: percent(self.sys.used_memory(), self.sys.total_memory()),
            disk_usage: self.disk_usage,
        }
    }
}

/// Reads the CPU temperature in °C.
pub fn cpu_temperature() -> Option<f32> {
    let temp = fs::read_to_string(CPU_TEMP_PATH)
        .ok()
        .and_then(|contents| parse_millidegrees(&contents));
    if temp.is_none() {
        warn!("Failed to read CPU temperature");
    }
    temp
}

fn parse_millidegrees(contents: &str) -> Option<f32> {
    contents.trim().parse::<f32>().ok().map(|millidegrees| millidegrees / 1000.0)
}

/// Used space on the first disk sysinfo reports, in percent.
fn disk_usage() -> f64 {
    let mut disks = Disks::new_with_refreshed_list();
    match disks.first_mut() {
        Some(disk) => {
            disk.refresh();
            let total = disk.total_space();
            percent(total.saturating_sub(disk.available_space()), total)
        }
        None => 0.0,
    }
}

fn percent(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    used as f64 / total as f64 * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_thermal_zone_contents() {
        assert_eq!(parse_millidegrees("45764\n"), Some(45.764));
        assert_eq!(parse_millidegrees(""), None);
        assert_eq!(parse_millidegrees("n/a"), None);
    }

    #[test]
    fn computes_percent() {
        assert_eq!(percent(25, 100), 25.0);
        assert_eq!(percent(0, 100), 0.0);
        assert_eq!(percent(5, 0), 0.0);
    }
}
