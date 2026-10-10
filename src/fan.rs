use linux_embedded_hal::I2cdev;
use pcf857x::{OutputPin, Pcf8574, SlaveAddr};
use anyhow::{anyhow, Result};
use log::{debug, info};


const I2C_BUS_PATH: &str = "/dev/i2c-1";

/// Switches the PoE HAT fan (PCF8574 pin p0, active low) on and off
/// with hysteresis between `temp_off` and `temp_on`. The HAT's FAN switch
/// must be set to P0; in the EN position the fan always runs.
///
/// The fan is kept on whenever the CPU temperature is unknown.
pub struct FanController {
    expander: Pcf8574<I2cdev>,
    is_running: bool,
    pub temp_on: f32,
    pub temp_off: f32,
}

impl FanController {
    pub fn new(temp_on: f32, temp_off: f32) -> Result<Self> {
        debug!("Initializing FanController");
        if temp_off <= 0.0 || temp_on <= 0.0 {
            return Err(anyhow!("Temperatures must be greater than 0"));
        }
        if temp_on <= temp_off {
            return Err(anyhow!("temp_on must be greater than temp_off"));
        }

        let i2c = I2cdev::new(I2C_BUS_PATH)?;
        debug!("I2C device initialized");
        let expander = Pcf8574::new(i2c, SlaveAddr::default());
        debug!("pcf8574 IO Expander initialized");

        Ok(FanController {
            expander,
            is_running: false,
            temp_off,
            temp_on,
        })
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Sets the fan's initial state: on unless the CPU is already at or below `temp_off`.
    pub fn start(&mut self, cpu_temp: Option<f32>) -> Result<()> {
        let on = initial_fan_state(cpu_temp, self.temp_off);
        info!("CPU temp {}, starting with fan {}", format_temp(cpu_temp), if on { "on" } else { "off" });
        self.set(on)
    }

    /// Turns the fan on or off if `cpu_temp` has crossed a threshold.
    /// On a failed write the state is left unchanged, so the next call retries.
    pub fn update(&mut self, cpu_temp: Option<f32>) -> Result<()> {
        match next_fan_state(self.is_running, cpu_temp, self.temp_on, self.temp_off) {
            Some(on) => {
                info!("CPU temp {}, turning fan {}", format_temp(cpu_temp), if on { "on" } else { "off" });
                self.set(on)
            }
            None => Ok(()),
        }
    }

    pub fn set(&mut self, on: bool) -> Result<()> {
        debug!("Sending fan {} signal [p0: {}]", if on { "on" } else { "off" }, if on { "low" } else { "high" });
        let mut parts = self.expander.split();
        let result = if on { parts.p0.set_low() } else { parts.p0.set_high() };
        result.map_err(|e| anyhow!("Failed to set fan {}: {:?}", if on { "on" } else { "off" }, e))?;
        self.is_running = on;
        Ok(())
    }
}

fn format_temp(cpu_temp: Option<f32>) -> String {
    cpu_temp.map_or("unknown".to_string(), |t| format!("{:.1}°C", t))
}

/// The fan starts on unless the CPU is known to be at or below `temp_off`.
fn initial_fan_state(cpu_temp: Option<f32>, temp_off: f32) -> bool {
    cpu_temp.map_or(true, |t| t > temp_off)
}

/// Returns the state to switch the fan to, or `None` to leave it as it is.
fn next_fan_state(is_running: bool, cpu_temp: Option<f32>, temp_on: f32, temp_off: f32) -> Option<bool> {
    match cpu_temp {
        None if !is_running => Some(true),
        Some(t) if is_running && t <= temp_off => Some(false),
        Some(t) if !is_running && t >= temp_on => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{initial_fan_state, next_fan_state};

    const ON: f32 = 40.0;
    const OFF: f32 = 30.0;

    #[test]
    fn turns_on_at_or_above_temp_on() {
        assert_eq!(next_fan_state(false, Some(39.9), ON, OFF), None);
        assert_eq!(next_fan_state(false, Some(40.0), ON, OFF), Some(true));
        assert_eq!(next_fan_state(false, Some(75.0), ON, OFF), Some(true));
    }

    #[test]
    fn turns_off_at_or_below_temp_off() {
        assert_eq!(next_fan_state(true, Some(30.1), ON, OFF), None);
        assert_eq!(next_fan_state(true, Some(30.0), ON, OFF), Some(false));
        assert_eq!(next_fan_state(true, Some(20.0), ON, OFF), Some(false));
    }

    #[test]
    fn holds_state_between_thresholds() {
        assert_eq!(next_fan_state(true, Some(35.0), ON, OFF), None);
        assert_eq!(next_fan_state(false, Some(35.0), ON, OFF), None);
    }

    #[test]
    fn no_change_when_already_in_target_state() {
        assert_eq!(next_fan_state(true, Some(70.0), ON, OFF), None);
        assert_eq!(next_fan_state(false, Some(20.0), ON, OFF), None);
    }

    #[test]
    fn unknown_temperature_keeps_fan_on() {
        assert_eq!(next_fan_state(false, None, ON, OFF), Some(true));
        assert_eq!(next_fan_state(true, None, ON, OFF), None);
    }

    #[test]
    fn starts_on_unless_at_or_below_temp_off() {
        assert!(initial_fan_state(Some(45.0), OFF));
        assert!(initial_fan_state(Some(35.0), OFF));
        assert!(initial_fan_state(Some(30.1), OFF));
        assert!(!initial_fan_state(Some(30.0), OFF));
        assert!(!initial_fan_state(Some(20.0), OFF));
        assert!(initial_fan_state(None, OFF));
    }
}
