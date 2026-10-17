use crate::metrics::Metrics;
use crate::network::InterfaceAddr;

/// The values a config can refer to by name, formatted for display.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayValues {
    /// Interface name, e.g. `eth0.20`
    pub interface: String,
    /// First display part of the interface name, e.g. `eth`
    pub interface_phys: String,
    /// Second display part of the interface name, e.g. `0.20`
    pub interface_numvlan: String,
    pub ip: String,
    pub ip_octets: [u8; 4],
    pub cpu_usage: String,
    pub cpu_temp: String,
    pub ram_usage: String,
    pub disk_usage: String,
    pub hostname: String,
}

impl DisplayValues {
    pub fn new(interface: &InterfaceAddr, metrics: &Metrics, hostname: &str) -> Self {
        let (interface_phys, interface_numvlan) = interface.display_parts();
        DisplayValues {
            interface: interface.name.clone(),
            interface_phys,
            interface_numvlan,
            ip: interface.ip.to_string(),
            ip_octets: interface.ip.octets(),
            cpu_usage: format_reading(metrics.cpu_usage.into()),
            cpu_temp: metrics.cpu_temp.map_or_else(|| "--".to_string(), |t| format_reading(t.into())),
            ram_usage: format_reading(metrics.ram_usage),
            disk_usage: format_reading(metrics.disk_usage),
            hostname: hostname.to_string(),
        }
    }

    /// Fixed values for `--render --sample`, so renders can be compared.
    pub fn sample() -> Self {
        DisplayValues {
            interface: "eth0.20".to_string(),
            interface_phys: "eth".to_string(),
            interface_numvlan: "0.20".to_string(),
            ip: "10.0.20.133".to_string(),
            ip_octets: [10, 0, 20, 133],
            cpu_usage: "12.5".to_string(),
            cpu_temp: "48.2".to_string(),
            ram_usage: "7.0".to_string(),
            disk_usage: "21.4".to_string(),
            hostname: "rustberry".to_string(),
        }
    }

    /// The widest values each name normally takes, for `--check-config`.
    /// The hostname has no useful worst case, so it is a typical one.
    pub fn worst_case() -> Self {
        DisplayValues {
            interface: "eth0.999".to_string(),
            interface_phys: "eth".to_string(),
            interface_numvlan: "0.999".to_string(),
            ip: "255.255.255.255".to_string(),
            ip_octets: [255; 4],
            cpu_usage: "99.9".to_string(),
            cpu_temp: "99.9".to_string(),
            ram_usage: "99.9".to_string(),
            disk_usage: "99.9".to_string(),
            hostname: "rustberry".to_string(),
        }
    }

    pub fn get(&self, name: ValueName) -> String {
        match name {
            ValueName::Interface => self.interface.clone(),
            ValueName::InterfacePhys => self.interface_phys.clone(),
            ValueName::InterfaceNumVlan => self.interface_numvlan.clone(),
            ValueName::Ip => self.ip.clone(),
            ValueName::IpOctet(i) => self.ip_octets[i].to_string(),
            ValueName::CpuUsage => self.cpu_usage.clone(),
            ValueName::CpuTemp => self.cpu_temp.clone(),
            ValueName::RamUsage => self.ram_usage.clone(),
            ValueName::DiskUsage => self.disk_usage.clone(),
            ValueName::Hostname => self.hostname.clone(),
        }
    }
}

/// The value names a config can use in a template, e.g. `{cpu_temp}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueName {
    Interface,
    InterfacePhys,
    InterfaceNumVlan,
    Ip,
    /// One of the four octets of `ip`, 0 to 3 from the left
    IpOctet(usize),
    CpuUsage,
    CpuTemp,
    RamUsage,
    DiskUsage,
    Hostname,
}

impl ValueName {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "interface" => ValueName::Interface,
            "interface_phys" => ValueName::InterfacePhys,
            "interface_numvlan" => ValueName::InterfaceNumVlan,
            "ip" => ValueName::Ip,
            "ip_octet0" => ValueName::IpOctet(0),
            "ip_octet1" => ValueName::IpOctet(1),
            "ip_octet2" => ValueName::IpOctet(2),
            "ip_octet3" => ValueName::IpOctet(3),
            "cpu_usage" => ValueName::CpuUsage,
            "cpu_temp" => ValueName::CpuTemp,
            "ram_usage" => ValueName::RamUsage,
            "disk_usage" => ValueName::DiskUsage,
            "hostname" => ValueName::Hostname,
            _ => return None,
        })
    }
}

/// Formats a reading with one decimal, dropping it from 100 up so the value
/// stays four characters wide, e.g. "99.9" then "100".
fn format_reading(value: f64) -> String {
    if value >= 99.95 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_values_from_readings() {
        let interface = InterfaceAddr { name: "eth0.20".to_string(), ip: "10.0.20.133".parse().unwrap() };
        let metrics = Metrics { cpu_usage: 12.54, cpu_temp: Some(48.16), ram_usage: 7.0, disk_usage: 21.44 };
        assert_eq!(DisplayValues::new(&interface, &metrics, "rustberry"), DisplayValues::sample());

        let no_temp = Metrics { cpu_temp: None, ..metrics };
        assert_eq!(DisplayValues::new(&interface, &no_temp, "rustberry").cpu_temp, "--");
    }

    #[test]
    fn readings_stay_four_characters_wide() {
        assert_eq!(format_reading(0.0), "0.0");
        assert_eq!(format_reading(99.94), "99.9");
        assert_eq!(format_reading(99.95), "100");
        assert_eq!(format_reading(100.0), "100");
    }

    #[test]
    fn looks_up_values_by_name() {
        let values = DisplayValues::sample();
        let get = |name: &str| values.get(ValueName::parse(name).unwrap());
        assert_eq!(get("interface"), "eth0.20");
        assert_eq!(get("ip"), "10.0.20.133");
        assert_eq!(get("ip_octet3"), "133");
        assert_eq!(get("hostname"), "rustberry");
        assert_eq!(ValueName::parse("ip_octet4"), None);
        assert_eq!(ValueName::parse("ip_info.0"), None);
    }
}
