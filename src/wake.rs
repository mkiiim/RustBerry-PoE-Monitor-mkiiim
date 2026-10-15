use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};
use tiny_http::{Header, Response, Server};

/// Longest wake that can be requested with `?seconds=`.
const MAX_WAKE: Duration = Duration::from_secs(3600);

/// Tracks whether the screen has been woken and until when. Cloned handles
/// share the same state, so the HTTP thread and the main loop see the same wake.
#[derive(Clone)]
pub struct WakeControl {
    wake_until: Arc<Mutex<Option<Instant>>>,
    default_duration: Duration,
}

impl WakeControl {
    pub fn new(default_duration: Duration) -> Self {
        WakeControl { wake_until: Arc::new(Mutex::new(None)), default_duration }
    }

    /// Keeps the screen on for `duration` from now, replacing any earlier wake.
    pub fn wake(&self, duration: Duration) {
        *self.lock() = Some(Instant::now() + duration.min(MAX_WAKE));
    }

    pub fn sleep(&self) {
        *self.lock() = None;
    }

    /// Time left before the screen goes back to sleep, or `None` if it is asleep.
    pub fn remaining(&self) -> Option<Duration> {
        self.lock()
            .and_then(|until| until.checked_duration_since(Instant::now()))
            .filter(|left| !left.is_zero())
    }

    pub fn is_awake(&self) -> bool {
        self.remaining().is_some()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Instant>> {
        // The state is a plain timestamp, so a poisoned lock is still usable
        self.wake_until.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn status_json(&self) -> String {
        let remaining = self.remaining().map_or(0, |left| left.as_secs());
        format!("{{\"on\":{},\"remaining\":{}}}", self.is_awake(), remaining)
    }
}

/// Starts the HTTP endpoint on `addr` in a background thread:
///
/// - `/wake` turns the screen on for the default time, or `?seconds=N` (max 3600)
/// - `/sleep` turns it off
/// - `/status` reports `{"on":true,"remaining":295}`
///
/// GET and POST are both accepted. Failing to bind only disables the endpoint.
pub fn spawn_server(addr: &str, control: WakeControl) {
    let server = match Server::http(addr) {
        Ok(server) => server,
        Err(e) => {
            error!("Failed to start wake endpoint on {}: {}", addr, e);
            return;
        }
    };
    info!("Wake endpoint listening on {}", addr);

    thread::spawn(move || {
        let json = Header::from_bytes("Content-Type", "application/json").expect("valid header");
        for request in server.incoming_requests() {
            let (status, body) = handle(request.method().as_str(), request.url(), &control);
            debug!("{} {} -> {} {}", request.method(), request.url(), status, body);
            let response = Response::from_string(body).with_status_code(status).with_header(json.clone());
            if let Err(e) = request.respond(response) {
                warn!("Failed to send wake endpoint response: {}", e);
            }
        }
    });
}

/// Routes one request and returns the status code and JSON body.
fn handle(method: &str, url: &str, control: &WakeControl) -> (u16, String) {
    if method != "GET" && method != "POST" {
        return (405, "{\"error\":\"method not allowed\"}".to_string());
    }
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    match path {
        "/wake" => match wake_duration(query, control.default_duration) {
            Some(duration) => {
                control.wake(duration);
                info!("Screen woken for {}s", duration.min(MAX_WAKE).as_secs());
                (200, control.status_json())
            }
            None => (400, "{\"error\":\"seconds must be a whole number\"}".to_string()),
        },
        "/sleep" => {
            control.sleep();
            info!("Screen put to sleep");
            (200, control.status_json())
        }
        "/status" => (200, control.status_json()),
        _ => (404, "{\"error\":\"not found\"}".to_string()),
    }
}

/// Reads `seconds=N` from the query string, falling back to `default`.
fn wake_duration(query: &str, default: Duration) -> Option<Duration> {
    match query.split('&').find_map(|pair| pair.strip_prefix("seconds=")) {
        Some(seconds) => seconds.parse().ok().map(Duration::from_secs),
        None => Some(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> WakeControl {
        WakeControl::new(Duration::from_secs(300))
    }

    #[test]
    fn starts_asleep() {
        assert!(!control().is_awake());
        assert_eq!(handle("GET", "/status", &control()), (200, "{\"on\":false,\"remaining\":0}".to_string()));
    }

    #[test]
    fn wakes_for_default_and_requested_time() {
        let c = control();
        assert_eq!(handle("GET", "/wake", &c).0, 200);
        let left = c.remaining().unwrap().as_secs();
        assert!((298..=300).contains(&left), "{}", left);

        assert_eq!(handle("POST", "/wake?seconds=60", &c).0, 200);
        let left = c.remaining().unwrap().as_secs();
        assert!((58..=60).contains(&left), "{}", left);
    }

    #[test]
    fn caps_wake_time() {
        let c = control();
        handle("GET", "/wake?seconds=999999", &c);
        assert!(c.remaining().unwrap() <= MAX_WAKE);
    }

    #[test]
    fn sleep_ends_wake() {
        let c = control();
        handle("GET", "/wake", &c);
        assert_eq!(handle("GET", "/sleep", &c), (200, "{\"on\":false,\"remaining\":0}".to_string()));
        assert!(!c.is_awake());
    }

    #[test]
    fn zero_seconds_leaves_screen_asleep() {
        let c = control();
        handle("GET", "/wake?seconds=0", &c);
        assert!(!c.is_awake());
    }

    #[test]
    fn rejects_bad_requests() {
        let c = control();
        assert_eq!(handle("GET", "/wake?seconds=abc", &c).0, 400);
        assert_eq!(handle("GET", "/wake?seconds=-5", &c).0, 400);
        assert_eq!(handle("DELETE", "/wake", &c).0, 405);
        assert_eq!(handle("GET", "/", &c).0, 404);
        assert!(!c.is_awake());
    }

    #[test]
    fn reads_seconds_from_query() {
        let default = Duration::from_secs(300);
        assert_eq!(wake_duration("", default), Some(default));
        assert_eq!(wake_duration("other=1", default), Some(default));
        assert_eq!(wake_duration("a=1&seconds=30", default), Some(Duration::from_secs(30)));
        assert_eq!(wake_duration("seconds=", default), None);
    }
}
