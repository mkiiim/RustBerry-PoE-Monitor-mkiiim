use std::cmp::Ordering;
use std::net::Ipv4Addr;
use std::process::Command;
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};

/// An IPv4 address assigned to a network interface, e.g. `eth0.20` / `10.0.20.133`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceAddr {
    pub name: String,
    pub ip: Ipv4Addr,
}

impl InterfaceAddr {
    /// Placeholder shown when no matching interface has an address.
    pub fn none() -> Self {
        InterfaceAddr { name: "NoInterface".to_string(), ip: Ipv4Addr::UNSPECIFIED }
    }

    /// Splits the interface name for the two-line display, see [`split_interface`].
    pub fn display_parts(&self) -> (String, String) {
        split_interface(&self.name)
    }
}

/// Cycles through the host's interface addresses, one per call to [`next`](Self::next),
/// re-reading them from the system every `refresh_interval`.
pub struct InterfaceRotator {
    addrs: Vec<InterfaceAddr>,
    index: usize,
    last_refresh: Option<Instant>,
    refresh_interval: Duration,
}

impl InterfaceRotator {
    pub fn new(refresh_interval: Duration) -> Self {
        InterfaceRotator { addrs: Vec::new(), index: 0, last_refresh: None, refresh_interval }
    }

    /// Returns the next address to display, refreshing the list first if it is stale.
    pub fn next(&mut self) -> InterfaceAddr {
        let stale = self.last_refresh.map_or(true, |t| t.elapsed() >= self.refresh_interval);
        if stale {
            self.set_addrs(collect_interface_addrs());
            self.last_refresh = Some(Instant::now());
        }
        self.advance()
    }

    /// Replaces the address list. The rotation position is kept unless the list changed.
    fn set_addrs(&mut self, addrs: Vec<InterfaceAddr>) {
        if addrs != self.addrs {
            info!("Interface addresses changed: {:?} -> {:?}", self.addrs, addrs);
            self.addrs = addrs;
            self.index = 0;
        }
    }

    fn advance(&mut self) -> InterfaceAddr {
        if self.addrs.is_empty() {
            return InterfaceAddr::none();
        }
        if self.index >= self.addrs.len() {
            self.index = 0;
        }
        let addr = self.addrs[self.index].clone();
        self.index = (self.index + 1) % self.addrs.len();
        addr
    }
}

/// Runs `ip addr` and returns the sorted eth/wlan IPv4 addresses, or a single
/// placeholder entry if there are none.
pub fn collect_interface_addrs() -> Vec<InterfaceAddr> {
    let output = match Command::new("ip").arg("addr").output() {
        Ok(output) => output,
        Err(e) => {
            error!("Failed to execute ip command: {}", e);
            return vec![InterfaceAddr::none()];
        }
    };
    let output_str = String::from_utf8_lossy(&output.stdout);
    debug!("Raw 'ip addr' output: \n{}", output_str);

    let mut addrs = parse_ip_addr(&output_str);
    if addrs.is_empty() {
        warn!("No interfaces and IPs were found matching criteria");
        return vec![InterfaceAddr::none()];
    }
    addrs.sort_by(compare_interfaces);
    debug!("Sorted interface addresses: {:?}", addrs);
    addrs
}

/// Extracts IPv4 addresses of `eth*` and `wlan*` interfaces from `ip addr` output.
fn parse_ip_addr(output: &str) -> Vec<InterfaceAddr> {
    let mut addrs = Vec::new();
    let mut current_interface = String::new();

    for line in output.lines() {
        if line.starts_with(char::is_numeric) {
            // e.g. "5: eth0.20@eth0: <BROADCAST,...> mtu 1500 ..."
            if let Some(name) = line.split(": ").nth(1) {
                let name = name.split(' ').next().unwrap_or_default();
                let name = name.trim_end_matches(':').split('@').next().unwrap_or_default();
                current_interface = name.to_string();
            }
        } else if line.contains("inet ")
            && (current_interface.starts_with("eth") || current_interface.starts_with("wlan"))
        {
            // e.g. "    inet 10.0.20.133/24 brd 10.0.20.255 scope global eth0.20"
            let Some(cidr) = line.split_whitespace().find(|s| s.contains('/')) else {
                continue;
            };
            let ip_str = cidr.split('/').next().unwrap_or_default();
            match ip_str.parse::<Ipv4Addr>() {
                Ok(ip) => addrs.push(InterfaceAddr { name: current_interface.clone(), ip }),
                Err(_) => warn!("Invalid IP format for {}: {}", current_interface, ip_str),
            }
        }
    }
    addrs
}

/// Orders eth before wlan, then by interface number, then plain before VLANs,
/// then by VLAN id: eth0, eth0.20, eth0.30, eth1, wlan0.
fn compare_interfaces(a: &InterfaceAddr, b: &InterfaceAddr) -> Ordering {
    sort_key(&a.name).cmp(&sort_key(&b.name))
}

fn sort_key(name: &str) -> (String, u32, Option<u32>) {
    let leading_number = |s: &str| -> u32 {
        let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().unwrap_or(0)
    };

    if let Some(rest) = name.strip_prefix("eth") {
        match rest.split_once('.') {
            Some((num, vlan)) => ("eth".to_string(), leading_number(num), Some(vlan.parse().unwrap_or(0))),
            None => ("eth".to_string(), leading_number(rest), None),
        }
    } else if let Some(rest) = name.strip_prefix("wlan") {
        ("wlan".to_string(), leading_number(rest), None)
    } else {
        (name.to_string(), 999, None)
    }
}

/// Splits an interface name into the two strings the layouts display.
/// VLAN interfaces move the interface's last character to the second part
/// ("eth0.99" -> ("eth", "0.99")); anything else is returned whole.
pub fn split_interface(interface: &str) -> (String, String) {
    let parts: Vec<&str> = interface.split('.').collect();
    if let [phys, vlan] = parts[..] {
        if let Some(last_char) = phys.chars().last() {
            let numvlan = format!("{}.{}", last_char, vlan);
            let phys = if phys.chars().count() > 1 {
                &phys[..phys.len() - last_char.len_utf8()]
            } else {
                phys
            };
            return (phys.to_string(), numvlan);
        }
    }
    (interface.to_string(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP_ADDR_OUTPUT: &str = "\
1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN group default qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
    inet 127.0.0.1/8 scope host lo
2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default qlen 1000
    link/ether dc:a6:32:00:00:01 brd ff:ff:ff:ff:ff:ff
    inet 192.168.0.133/24 brd 192.168.0.255 scope global dynamic eth0
    inet6 fe80::1/64 scope link
3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc pfifo_fast state UP group default qlen 1000
    inet 192.168.1.50/24 brd 192.168.1.255 scope global wlan0
4: eth0.30@eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc noqueue state UP group default qlen 1000
    inet 10.0.30.133/24 brd 10.0.30.255 scope global eth0.30
5: eth0.20@eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc noqueue state UP group default qlen 1000
    inet 10.0.20.133/24 brd 10.0.20.255 scope global eth0.20
6: docker0: <NO-CARRIER,BROADCAST,MULTICAST,UP> mtu 1500 qdisc noqueue state DOWN group default
    inet 172.17.0.1/16 brd 172.17.255.255 scope global docker0
";

    fn addr(name: &str, ip: &str) -> InterfaceAddr {
        InterfaceAddr { name: name.to_string(), ip: ip.parse().unwrap() }
    }

    #[test]
    fn parses_only_eth_and_wlan_ipv4() {
        let addrs = parse_ip_addr(IP_ADDR_OUTPUT);
        assert_eq!(addrs, vec![
            addr("eth0", "192.168.0.133"),
            addr("wlan0", "192.168.1.50"),
            addr("eth0.30", "10.0.30.133"),
            addr("eth0.20", "10.0.20.133"),
        ]);
    }

    #[test]
    fn sorts_eth_then_vlans_then_wlan() {
        let mut addrs = parse_ip_addr(IP_ADDR_OUTPUT);
        addrs.sort_by(compare_interfaces);
        let names: Vec<&str> = addrs.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["eth0", "eth0.20", "eth0.30", "wlan0"]);
    }

    #[test]
    fn sorts_by_interface_number() {
        let mut addrs = vec![addr("wlan1", "1.1.1.1"), addr("eth1", "1.1.1.2"),
                             addr("wlan0", "1.1.1.3"), addr("eth0.5", "1.1.1.4")];
        addrs.sort_by(compare_interfaces);
        let names: Vec<&str> = addrs.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["eth0.5", "eth1", "wlan0", "wlan1"]);
    }

    #[test]
    fn splits_interface_names() {
        assert_eq!(split_interface("eth0.99"), ("eth".to_string(), "0.99".to_string()));
        assert_eq!(split_interface("eth0"), ("eth0".to_string(), String::new()));
        assert_eq!(split_interface("wlan0"), ("wlan0".to_string(), String::new()));
        assert_eq!(split_interface("x.5"), ("x".to_string(), "x.5".to_string()));
        assert_eq!(split_interface(".5"), (".5".to_string(), String::new()));
        assert_eq!(split_interface("a.b.c"), ("a.b.c".to_string(), String::new()));
    }

    #[test]
    fn rotates_through_addresses() {
        let mut rotator = InterfaceRotator::new(Duration::from_secs(60));
        rotator.set_addrs(vec![addr("eth0", "1.1.1.1"), addr("eth0.20", "1.1.1.2")]);
        assert_eq!(rotator.advance().name, "eth0");
        assert_eq!(rotator.advance().name, "eth0.20");
        assert_eq!(rotator.advance().name, "eth0");
    }

    #[test]
    fn keeps_position_when_list_unchanged() {
        let list = vec![addr("eth0", "1.1.1.1"), addr("eth0.20", "1.1.1.2"), addr("wlan0", "1.1.1.3")];
        let mut rotator = InterfaceRotator::new(Duration::from_secs(60));
        rotator.set_addrs(list.clone());
        rotator.advance();
        rotator.set_addrs(list);
        assert_eq!(rotator.advance().name, "eth0.20");
    }

    #[test]
    fn restarts_when_list_changes() {
        let mut rotator = InterfaceRotator::new(Duration::from_secs(60));
        rotator.set_addrs(vec![addr("eth0", "1.1.1.1"), addr("eth0.20", "1.1.1.2")]);
        rotator.advance();
        rotator.set_addrs(vec![addr("eth0", "1.1.1.1"), addr("eth0.30", "1.1.1.3")]);
        assert_eq!(rotator.advance().name, "eth0");
    }

    #[test]
    fn empty_list_yields_placeholder() {
        let mut rotator = InterfaceRotator::new(Duration::from_secs(60));
        assert_eq!(rotator.advance(), InterfaceAddr::none());
    }
}
