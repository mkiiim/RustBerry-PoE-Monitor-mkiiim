use std::error::Error;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{System, Disks, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::{info, debug, trace, error, warn};
use clap::Parser;
use env_logger::{Builder, Env};

mod fan_controller;
use fan_controller::FanController;

mod display;
use display::PoeDisplay;

mod display_types;
mod default_config;

mod network;
use network::InterfaceRotator;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, default_value_t = 60.0)]
    temp_on: f32,

    #[clap(long, default_value_t = 50.0)]
    temp_off: f32,

    #[arg(long, default_value = "/etc/rustberry-poe-monitor/portrait.json")]
    config: String,
}

// How often the interface list is re-read from the system
const INTERFACE_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
// Show the next interface every this many loop iterations
const INTERFACE_ROTATE_EVERY: u64 = 5;

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
    debug!("Using config file: {}", args.config);

    // Initialize display with potential fallback to default config
    let mut poe_disp = match PoeDisplay::new(&args.config) {
        Ok(disp) => {
            info!("Display initialized with configuration from: {}", args.config);
            disp
        },
        Err(e) => {
            error!("Failed to initialize display: {}", e);
            // Box the error to match the return type
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, 
                format!("Display initialization failed: {}", e))));
        }
    };

    // Initialize fan controller with graceful error handling
    let mut fan_controller = match FanController::new(args.temp_on, args.temp_off) {
        Ok(fc) => {
            info!("Fan controller initialized. temp-on: {}, temp-off: {}", 
                  fc.temp_on, fc.temp_off);
            fc
        },
        Err(e) => {
            error!("Failed to initialize fan controller: {}", e);
            // Box the error to match the return type
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, 
                format!("Fan controller initialization failed: {}", e))));
        }
    };

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
    
    if let Err(e) = fan_controller.fan_off() {
        warn!("Failed to turn off fan initially: {}", e);
    }
    
    let mut iteration_count: u64 = 0;
    let mut interfaces = InterfaceRotator::new(INTERFACE_REFRESH_INTERVAL);
    let mut interface = interfaces.next();

    loop {
        iteration_count += 1;
        trace!("main loop iteration: {}", iteration_count);
        
        // Only refresh system info every iteration
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        
        if iteration_count % INTERFACE_ROTATE_EVERY == 0 {
            interface = interfaces.next();
        }

        let cpu_temp = get_cpu_temperature();
        let cpu_temp_str = format!("{:.1}", cpu_temp);
        let cpu_usage = format!("{:.1}", sys.global_cpu_info().cpu_usage());
        let ram_usage = format!("{:.1}", get_ram_usage(&sys));
        
        // Fan control logic
        trace!("Checking fan controller. Fan running: {}", fan_controller.is_running);
        trace!("CPU Temp: {}", cpu_temp);
        
        if fan_controller.is_running {
            if cpu_temp <= fan_controller.temp_off {
                if let Err(e) = fan_controller.fan_off() {
                    warn!("Failed to turn off fan: {}", e);
                }
            }
        } else if cpu_temp >= fan_controller.temp_on {
            if let Err(e) = fan_controller.fan_on() {
                warn!("Failed to turn on fan: {}", e);
            }
        }
        
        // Update disk usage less frequently
        if last_disk_update.elapsed() >= disk_update_interval {
            last_disk_update = Instant::now();
            disk_usage = format!("{:.1}", get_disk_usage());
            info!("Updated disk usage: {}", disk_usage);
        }
        
        let ip_info = (interface.name.clone(), interface.ip.to_string(), interface.ip.octets());
        let (interface_phys, interface_numvlan) = interface.display_parts();
        
        // Log values we're about to display for debugging
        debug!(
            "Display values: ip:{}, interface:{}, phys:{}, vlan:{}, octets:{:?}, cpu:{}, temp:{}, ram:{}, disk:{}",
            ip_info.1, ip_info.0, interface_phys, interface_numvlan, ip_info.2, 
            cpu_usage, cpu_temp_str, ram_usage, disk_usage
        );
        
        // Update the display with consistent error handling
        match poe_disp.update_display(
            &ip_info,
            &ip_info.1,      // IP Address e.g., 192.168.0.1
            &ip_info.0,      // Interface e.g., eth0.99
            &interface_phys,  // Physical interface e.g., eth0
            &interface_numvlan, // VLAN interface e.g., 99
            &ip_info.2,      // IP Octets e.g., [192, 168, 0, 1]
            &cpu_usage,
            &cpu_temp_str,       // CPU temperature
            &ram_usage,
            &disk_usage,
        ) {
            Ok(_) => {
                trace!("Display updated successfully");
                // Slow down the update rate to reduce flickering
                thread::sleep(Duration::from_millis(500));
            },
            Err(e) => {
                error!("Failed to update display: {:?}", e);
                // Sleep even on error to prevent rapid retries
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn get_cpu_temperature() -> f32 {
    match fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
        Ok(temp_contents) => {
            temp_contents.trim().parse::<f32>().unwrap_or(0.0) / 1000.0
        },
        Err(_) => {
            warn!("Failed to read CPU temperature, returning 0.0");
            0.0
        }
    }
}

fn get_ram_usage(sys: &System) -> f64 {
    let total_memory = sys.total_memory();
    if total_memory == 0 {
        return 0.0;
    }
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
