use std::error::Error;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{System, Disks, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::{info, debug, trace};
use clap::Parser;
use env_logger::{Builder, Env};

use lazy_static::lazy_static;
use std::process::Command;
use std::sync::Mutex;

mod fan_controller;
use fan_controller::FanController;

mod display;
use display::PoeDisplay;

mod display_types;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, default_value_t = 60.0)]
    temp_on: f32,

    #[clap(long, default_value_t = 50.0)]
    temp_off: f32,

    #[clap(long, default_value = "landscape")]
    display: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let env = Env::default().default_filter_or("info");
    Builder::from_env(env).init();

    let version = env!("CARGO_PKG_VERSION");

    debug!("Binary info:");
    debug!("================================");
    debug!("rustberry-poe-monitor:   {}", version);
    debug!("Target OS:               {}", std::env::consts::OS);
    debug!("Target Family:           {}", std::env::consts::FAMILY);
    debug!("Target Architecture:     {}", std::env::consts::ARCH);

    let args = Args::parse();
    let display_orientation = args.display.clone();
    debug!("Display orientation: {}", display_orientation);

    let mut poe_disp = PoeDisplay::new(&args.display)?;
    info!("Display initialized");

    let mut fan_controller = FanController::new(args.temp_on, args.temp_off)?;
    info!("Fan controller initialized. temp-on: {}, temp-off: {}", fan_controller.temp_on, fan_controller.temp_off);

    let mut sys: System = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::new().with_cpu_usage())
            .with_memory(MemoryRefreshKind::new().with_ram()),
    );

    debug!("System initialized. System info:");
    debug!("================================");
    debug!("System name:             {}", System::name().unwrap_or_default());
    debug!("System kernel version:   {}", System::kernel_version().unwrap_or_default());
    debug!("System OS version:       {}", System::os_version().unwrap_or_default());

    let mut disk_usage = String::new();
    let disk_update_interval = Duration::from_secs(60);
    let mut last_disk_update = Instant::now() - disk_update_interval;
    info!("Starting main loop");

    fan_controller.fan_off()?;

    let mut iteration_count = 0;
    let mut ip_info = get_local_ip();

    loop {
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        
        if iteration_count % 5 == 0 {
            ip_info = get_local_ip();
        }

        let temp = get_cpu_temperature();
        let temp_str = format!("{:.1}", temp);
        let cpu_usage = format!("{:.1}", sys.global_cpu_info().cpu_usage());
        let ram_usage = format!("{:.1}", get_ram_usage(&sys));

        trace!("Checking fan controller. Fan running: {}", fan_controller.is_running);
        trace!("CPU Temp: {}", temp);

        if fan_controller.is_running {
            if temp <= fan_controller.temp_off {
                fan_controller.fan_off()?;
            }
        } else if temp >= fan_controller.temp_on {
            fan_controller.fan_on()?;
        }

        if last_disk_update.elapsed() >= disk_update_interval {
            last_disk_update = Instant::now();
            disk_usage = format!("{:.1}", get_disk_usage());
        }

        let (interface_phys, interface_numvlan) = split_interface(&ip_info.0);

        poe_disp.update_display(
            &ip_info.1,
            &ip_info.0,
            &interface_phys,
            &interface_numvlan,
            ip_info.2,
            cpu_usage, temp_str,
            ram_usage, &disk_usage,
            &display_orientation
        ).unwrap();
        thread::sleep(Duration::from_secs(1));
        
        iteration_count += 1;
    }
}

fn get_cpu_temperature() -> f32 {
    let temp_contents = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").unwrap();
    temp_contents.trim().parse::<f32>().unwrap() / 1000.0
}

fn get_ram_usage(sys: &System) -> f64 {
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    (used_memory as f64 / total_memory as f64) * 100.0
}

fn get_disk_usage() -> f64 {
    let mut disks = Disks::new_with_refreshed_list();
    if let Some(disk) = disks.first_mut() {
        disk.refresh();
        let total_space = disk.total_space();
        let available_space = disk.available_space();
        if total_space > 0 {
            (1.0 - (available_space as f64 / total_space as f64)) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    }
}

lazy_static! {
    static ref IP_ADDRESSES: Mutex<Vec<(String, String, [u8; 4])>> = Mutex::new(Vec::new());
    static ref CURRENT_INDEX: Mutex<usize> = Mutex::new(0);
}

fn collect_interface_ips() -> Vec<(String, String, [u8; 4])> {
    let output = Command::new("ip")
        .args(&["addr"])
        .output()
        .expect("Failed to execute ip command");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut ips = Vec::new();
    let mut current_interface = String::new();

    for line in output_str.lines() {
        if line.starts_with(char::is_numeric) {
            if let Some(interface) = line.split(": ").nth(1)
                .map(|s| s.split(' ').next().unwrap()
                .trim_end_matches(':')
                .split('@').next().unwrap()) {
                current_interface = interface.to_string();
            }
        } else if line.contains("inet ") && current_interface.starts_with("eth0") {
            if let Some(ip) = line
                .split_whitespace()
                .find(|s| s.contains("/"))
                .map(|s| s.split('/').next().unwrap().to_string())
            {
                // Parse IP into [u8;4] octets
                let octs: Vec<u8> = ip
                    .split('.')
                    .map(|num| num.parse().unwrap_or(0))
                    .collect();
                if octs.len() == 4 {
                    ips.push((current_interface.clone(), ip, [octs[0], octs[1], octs[2], octs[3]]));
                }
            }
        }
    }
    ips
}

fn get_local_ip() -> (String, String, [u8; 4]) {
    let mut addresses = IP_ADDRESSES.lock().unwrap();
    let mut index = CURRENT_INDEX.lock().unwrap();

    // If empty, collect new interface data
    if addresses.is_empty() {
        *addresses = collect_interface_ips();
        if addresses.is_empty() {
            // Return a dummy record with a "No IP" sentinel
            return ("NoInterface".to_string(), "No IP".to_string(), [0,0,0,0]);
        }
    }

    let (iface, ip, ip_octets) = addresses[*index].clone();
    *index = (*index + 1) % addresses.len();

    (iface, ip, ip_octets)
}

fn split_interface(interface: &str) -> (String, String) {
    let parts: Vec<&str> = interface.split('.').collect();
    if parts.len() == 2 {
        let phys = parts[0].to_string();
        let numvlan = format!("{}.{}", &phys[phys.len() - 1..], parts[1]);
        (phys[..phys.len() - 1].to_string(), numvlan)
    } else {
        (interface.to_string(), String::new())
    }
}
