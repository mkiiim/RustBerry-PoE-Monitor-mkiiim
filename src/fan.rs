use linux_embedded_hal::I2cdev;
use pcf857x::{OutputPin, Pcf8574, SlaveAddr};
use anyhow::{anyhow, Result};
use log::{debug, info};


const I2C_BUS_PATH: &str = "/dev/i2c-1";

/// Switches the PoE HAT fan (PCF8574 pin p0, active low) on and off
/// with hysteresis between `temp_off` and `temp_on`.
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

    /// Turns the fan on or off if `cpu_temp` has crossed a threshold.
    /// On a failed write the state is left unchanged, so the next call retries.
    pub fn update(&mut self, cpu_temp: f32) -> Result<()> {
        match next_fan_state(self.is_running, cpu_temp, self.temp_on, self.temp_off) {
            Some(on) => {
                info!("CPU temp {:.1}°C, turning fan {}", cpu_temp, if on { "on" } else { "off" });
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

/// Returns the state to switch the fan to, or `None` to leave it as it is.
fn next_fan_state(is_running: bool, cpu_temp: f32, temp_on: f32, temp_off: f32) -> Option<bool> {
    if is_running && cpu_temp <= temp_off {
        Some(false)
    } else if !is_running && cpu_temp >= temp_on {
        Some(true)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::next_fan_state;

    const ON: f32 = 60.0;
    const OFF: f32 = 50.0;

    #[test]
    fn turns_on_at_or_above_temp_on() {
        assert_eq!(next_fan_state(false, 59.9, ON, OFF), None);
        assert_eq!(next_fan_state(false, 60.0, ON, OFF), Some(true));
        assert_eq!(next_fan_state(false, 75.0, ON, OFF), Some(true));
    }

    #[test]
    fn turns_off_at_or_below_temp_off() {
        assert_eq!(next_fan_state(true, 50.1, ON, OFF), None);
        assert_eq!(next_fan_state(true, 50.0, ON, OFF), Some(false));
        assert_eq!(next_fan_state(true, 40.0, ON, OFF), Some(false));
    }

    #[test]
    fn holds_state_between_thresholds() {
        assert_eq!(next_fan_state(true, 55.0, ON, OFF), None);
        assert_eq!(next_fan_state(false, 55.0, ON, OFF), None);
    }

    #[test]
    fn no_change_when_already_in_target_state() {
        assert_eq!(next_fan_state(true, 70.0, ON, OFF), None);
        assert_eq!(next_fan_state(false, 40.0, ON, OFF), None);
    }
}
