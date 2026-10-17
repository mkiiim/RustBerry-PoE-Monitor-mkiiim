use std::error::Error;
use std::thread;
use std::time::Duration;
use log::{info, debug, trace, error, warn};
use clap::Parser;
use env_logger::{Builder, Env};

mod fan;
use fan::FanController;

mod display;
use display::{DisplayValues, PoeDisplay};

mod metrics;
use metrics::MetricsCollector;

mod network;
use network::InterfaceRotator;

mod wake;
use wake::WakeControl;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, default_value_t = 40.0)]
    temp_on: f32,

    #[clap(long, default_value_t = 30.0)]
    temp_off: f32,

    #[arg(long, default_value = "/etc/rustberry-poe-monitor/portrait.json")]
    config: String,

    /// Keep the display switched off unless woken; fan control still runs
    #[arg(long)]
    screen_off: bool,

    /// Serve /wake, /sleep and /status on this address, e.g. 0.0.0.0:8590
    #[arg(long)]
    listen: Option<String>,

    /// How long /wake keeps the display on, in seconds
    #[arg(long, default_value_t = 300)]
    wake_seconds: u64,

    /// Check the --config file and exit, without touching the display or fan
    #[arg(long)]
    check_config: bool,
}

// How often the interface list is re-read from the system
const INTERFACE_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
// Show the next interface every this many loop iterations
const INTERFACE_ROTATE_EVERY: u64 = 5;
// How often disk usage is re-read
const DISK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

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

    if args.check_config {
        match display::check_config(&args.config) {
            Ok(()) => {
                println!("{}: OK", args.config);
                return Ok(());
            },
            Err(e) => {
                eprintln!("{}: {}", args.config, e);
                std::process::exit(1);
            }
        }
    }

    // Initialize display; a bad config shows "CONFIG ERROR" instead of failing
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
    let mut fan = match FanController::new(args.temp_on, args.temp_off) {
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

    let mut metrics = MetricsCollector::new(DISK_REFRESH_INTERVAL);
    info!("Starting main loop");
    
    if let Err(e) = fan.start(metrics::cpu_temperature()) {
        warn!("Failed to set initial fan state: {}", e);
    }
    
    let wake = WakeControl::new(Duration::from_secs(args.wake_seconds));
    if let Some(addr) = &args.listen {
        wake::spawn_server(addr, wake.clone());
    }

    let mut display_powered_off = false;
    let mut iteration_count: u64 = 0;
    let mut interfaces = InterfaceRotator::new(INTERFACE_REFRESH_INTERVAL);
    let mut interface = interfaces.next();

    loop {
        iteration_count += 1;
        trace!("main loop iteration: {}", iteration_count);
        
        let m = metrics.sample();

        if iteration_count % INTERFACE_ROTATE_EVERY == 0 {
            interface = interfaces.next();
        }

        trace!("CPU Temp: {:?}, fan running: {}", m.cpu_temp, fan.is_running());
        if let Err(e) = fan.update(m.cpu_temp) {
            warn!("{}", e);
        }

        if args.screen_off && !wake.is_awake() {
            // Retried every loop until it succeeds
            if !display_powered_off {
                match poe_disp.power_off() {
                    Ok(()) => {
                        info!("Display switched off");
                        display_powered_off = true;
                    },
                    Err(e) => error!("Failed to switch display off: {:?}", e),
                }
            }
            thread::sleep(Duration::from_millis(500));
            continue;
        }
        
        let values = DisplayValues::new(&interface, &m);
        debug!("Display values: {:?}", values);

        match poe_disp.update_display(&values) {
            Ok(_) => {
                trace!("Display updated successfully");
                // Switched on only after drawing, so the old frame is never shown
                if display_powered_off {
                    match poe_disp.power_on() {
                        Ok(()) => {
                            info!("Display switched on");
                            display_powered_off = false;
                        },
                        Err(e) => error!("Failed to switch display on: {:?}", e),
                    }
                }
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
