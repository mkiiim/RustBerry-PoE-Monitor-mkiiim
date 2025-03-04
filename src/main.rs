use std::error::Error;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{System, Disks, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use log::{info, debug, trace, error, warn};
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
mod default_config;

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


lazy_static! {
    static ref IP_ADDRESSES: Mutex<Vec<(String, String, [u8; 4])>> = Mutex::new(Vec::new());
    static ref CURRENT_INDEX: Mutex<usize> = Mutex::new(0);
    static ref LAST_IP_REFRESH: Mutex<Instant> = Mutex::new(Instant::now());
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
    
    let mut iteration_count = 0;
    let mut ip_info = get_local_ip();
    
    // Add a way to detect network changes
    let mut previous_ip_info = get_local_ip();
    let mut network_check_counter = 0;

    loop {
        iteration_count += 1;
        info!("main loop iteration: {}", iteration_count);
        
        // Only refresh system info every iteration
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        
        // Only update IP info every 5 iterations (or similar interval)
        if iteration_count % 5 == 0 {
            network_check_counter += 1;
            
            // Every 12 network checks (about 1 minute if each loop is 1 second), 
            // perform a more thorough check to see if the network has changed
            if network_check_counter >= 15 {
                network_check_counter = 0;
                
                // Force a network refresh by clearing the IP_ADDRESSES cache
                {
                    let mut addresses = IP_ADDRESSES.lock().unwrap();
                    if !addresses.is_empty() {
                        info!("Periodic network check: Clearing IP cache to force refresh");
                        addresses.clear();
                    }
                }
            }
            
            ip_info = get_local_ip();
            
            // Check if IP info has changed, and log it clearly if it has
            if ip_info.1 != previous_ip_info.1 || ip_info.0 != previous_ip_info.0 {
                info!("IP information changed. Old: {:?}, New: {:?}", previous_ip_info, ip_info);
                previous_ip_info = ip_info.clone();
            }
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
        
        let (interface_phys, interface_numvlan) = split_interface(&ip_info.0);
        
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

fn collect_interface_ips() -> Vec<(String, String, [u8; 4])> {
    info!("Starting to collect interface IPs...");
    
    let output = match Command::new("ip").args(&["addr"]).output() {
        Ok(output) => output,
        Err(e) => {
            error!("Failed to execute ip command: {}", e);
            return vec![("NoInterface".to_string(), "0.0.0.0".to_string(), [0, 0, 0, 0])];
        }
    };

    let output_str = String::from_utf8_lossy(&output.stdout);
    debug!("Raw 'ip addr' output: \n{}", output_str);
    
    let mut ips = Vec::new();
    let mut current_interface = String::new();

    info!("Parsing interfaces from ip command output...");
    
    for line in output_str.lines() {
        debug!("Processing line: {}", line);
        
        if line.starts_with(char::is_numeric) {
            if let Some(interface) = line.split(": ").nth(1)
                .map(|s| s.split(' ').next().unwrap()
                .trim_end_matches(':')
                .split('@').next().unwrap()) {
                current_interface = interface.to_string();
                debug!("Found interface: {}", current_interface);
            }
        } else if line.contains("inet ") && current_interface.starts_with("eth0") {
            debug!("Found inet line for {}: {}", current_interface, line);
            
            if let Some(ip) = line
                .split_whitespace()
                .find(|s| s.contains("/"))
                .map(|s| s.split('/').next().unwrap().to_string())
            {
                debug!("Extracted IP: {}", ip);
                
                // Parse IP into [u8;4] octets
                let octs: Vec<u8> = ip
                    .split('.')
                    .map(|num| num.parse().unwrap_or(0))
                    .collect();
                if octs.len() == 4 {
                    info!("Adding interface: {}, IP: {}, octets: {:?}", 
                          current_interface, ip, [octs[0], octs[1], octs[2], octs[3]]);
                    ips.push((current_interface.clone(), ip, [octs[0], octs[1], octs[2], octs[3]]));
                } else {
                    warn!("Invalid IP format for {}: {}", current_interface, ip);
                }
            }
        }
    }
    
    if ips.is_empty() {
        warn!("No interfaces and IPs were found matching criteria");
        // Return a dummy entry so we have something to display
        ips.push(("NoInterface".to_string(), "0.0.0.0".to_string(), [0, 0, 0, 0]));
    } else {
        info!("Successfully collected {} interface IPs: {:?}", ips.len(), ips);
    }
    
    ips
}

fn get_local_ip() -> (String, String, [u8; 4]) {
    // Use a result pattern to handle potential errors while obtaining locks
    let result = (|| -> Result<(String, String, [u8; 4]), Box<dyn std::error::Error>> {
        let mut addresses = IP_ADDRESSES.lock().unwrap();
        let mut index = CURRENT_INDEX.lock().unwrap();
        let mut last_refresh = LAST_IP_REFRESH.lock().unwrap();
        
        // Force a refresh of IP addresses every 5 minutes (300 seconds)
        let refresh_interval = Duration::from_secs(300);
        let should_refresh = addresses.is_empty() || last_refresh.elapsed() >= refresh_interval;

        // Log current state
        info!(
            "get_local_ip called. Current addresses: {:?}, index: {}, time since last refresh: {:?}, should refresh: {}",
            addresses, *index, last_refresh.elapsed(), should_refresh
        );
        
        // Refresh if needed
        if should_refresh {
            info!("Refreshing IP addresses...");
            *addresses = collect_interface_ips();
            *last_refresh = Instant::now();
            
            // Reset index when we refresh
            *index = 0;
        }

        // Safely get an address or return a default
        if addresses.is_empty() {
            warn!("No IP addresses found, returning dummy record");
            return Ok(("NoInterface".to_string(), "0.0.0.0".to_string(), [0, 0, 0, 0]));
        }
        
        if *index >= addresses.len() {
            info!("Index {} is out of bounds, resetting to 0", *index);
            *index = 0; // Reset if out of bounds
        }
        
        let (iface, ip, ip_octets) = addresses[*index].clone();
        *index = (*index + 1) % addresses.len();
        
        info!("Returning IP info: interface={}, ip={}, octets={:?}, next index will be {}", 
              iface, ip, ip_octets, *index);
        Ok((iface, ip, ip_octets))
    })();
    
    // Handle any potential errors with mutex locks
    match result {
        Ok(info) => info,
        Err(e) => {
            error!("Error in get_local_ip: {}. Returning default values.", e);
            ("NoInterface".to_string(), "0.0.0.0".to_string(), [0, 0, 0, 0])
        }
    }
}

fn split_interface(interface: &str) -> (String, String) {
    let parts: Vec<&str> = interface.split('.').collect();
    if parts.len() == 2 {
        // Handle potential edge case where interface name is too short
        if parts[0].is_empty() {
            return (interface.to_string(), String::new());
        }
        
        // Safely extract the last character and handle potential string slicing issues
        let phys = parts[0].to_string();
        if phys.is_empty() {
            return (interface.to_string(), String::new());
        }
        
        // Safely get the last character
        if let Some(last_char) = phys.chars().last() {
            let numvlan = format!("{}.{}", last_char, parts[1]);
            // Safely create the physical interface name without the last character
            if phys.len() > 1 {
                return (phys[..phys.len() - 1].to_string(), numvlan);
            } else {
                return (phys, numvlan);
            }
        }
    }
    
    // Default fallback
    (interface.to_string(), String::new())
}