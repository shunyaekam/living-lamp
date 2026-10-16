//! The motor path: the organism's calls to lampbus's HTTP API (power, and effects it was asked to relay), and a poll of the
//! lamp's power state. Both run on their own threads so the 20 Hz loop never waits on a socket.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::Duration;

/// What the poll last saw of the lamp's power.
pub const UNKNOWN: u8 = 0;
pub const OFF: u8 = 1;
pub const ON: u8 = 2;
/// lampbus has not been told anything about the lamp since it started (`"known":false`), so its idea of the power is empty.
pub const UNTOUCHED: u8 = 3;

/// The motor: send control objects to lampbus.
#[derive(Clone)]
pub struct Motor {
    tx: Sender<String>,
    /// The lamp's power as last polled.
    pub lamp: Arc<AtomicU8>,
}

fn request(
    addr: &str,
    token: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<String, String> {
    let sock = addr
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("no address")?;
    let mut s =
        TcpStream::connect_timeout(&sock, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let status: u16 = raw
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .ok_or("malformed reply")?;
    let body = raw
        .split_once("\r\n\r\n")
        .map_or("", |(_, b)| b)
        .to_string();
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(format!("lampbus answered {status}: {}", body.trim()))
    }
}

/// The first lamp's power in a `/api/state` reply (`"on":true` or `"on":false`, as lampbus writes it).
pub fn power_in(state: &str) -> u8 {
    let first_on = state.find("\"on\":").unwrap_or(usize::MAX);
    if state.find("\"known\":false").is_some_and(|k| k < first_on) {
        return UNTOUCHED;
    }
    match (state.find("\"on\":true"), state.find("\"on\":false")) {
        (Some(a), Some(b)) if a < b => ON,
        (Some(_), None) => ON,
        (_, Some(_)) => OFF,
        _ => UNKNOWN,
    }
}

/// Put `"motor":true` into a control object so lampbus executes it instead of forwarding it back.
pub fn with_motor(json: &str) -> String {
    let inner = json
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    if inner.is_empty() {
        "{\"motor\":true}".into()
    } else {
        format!("{{\"motor\":true,{inner}}}")
    }
}

/// Start the motor and the power poll for lampbus's API at `addr`.
pub fn spawn(addr: String, token: String) -> Motor {
    let (tx, rx) = channel::<String>();
    let lamp = Arc::new(AtomicU8::new(UNKNOWN));
    let (a, t) = (addr.clone(), token.clone());
    std::thread::spawn(move || {
        for body in rx {
            if let Err(e) = request(&a, &t, "POST", "/api/control", &body) {
                eprintln!("living-lamp: motor: {e}");
            }
        }
    });
    let l = Arc::clone(&lamp);
    std::thread::spawn(move || loop {
        match request(&addr, &token, "GET", "/api/state", "") {
            Ok(state) => l.store(power_in(&state), Ordering::Relaxed),
            Err(_) => l.store(UNKNOWN, Ordering::Relaxed),
        }
        std::thread::sleep(Duration::from_secs(3));
    });
    Motor { tx, lamp }
}

impl Motor {
    /// Send a control object to lampbus (marked as the motor's own).
    pub fn control(&self, json: &str) {
        let _ = self.tx.send(with_motor(json));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_motor_mark_goes_into_any_object() {
        assert_eq!(with_motor("{}"), "{\"motor\":true}");
        assert_eq!(
            with_motor("{\"power\":false}"),
            "{\"motor\":true,\"power\":false}"
        );
        assert_eq!(
            with_motor(" { \"effect\": \"rainbow\" } "),
            "{\"motor\":true,\"effect\": \"rainbow\"}"
        );
    }

    #[test]
    fn power_is_read_from_the_state_reply() {
        assert_eq!(power_in("{\"lamps\":[{\"on\":true,\"x\":1}]}"), ON);
        assert_eq!(power_in("{\"lamps\":[{\"on\":false}]}"), OFF);
        assert_eq!(power_in("{\"lamps\":[]}"), UNKNOWN);
        assert_eq!(
            power_in("{\"lamps\":[{\"known\":false,\"on\":false}]}"),
            UNTOUCHED,
            "a restarted lampbus knows nothing yet"
        );
        assert_eq!(power_in("{\"lamps\":[{\"known\":true,\"on\":false}]}"), OFF);
        assert_eq!(
            power_in("{\"lamps\":[{\"on\":true},{\"on\":false}]}"),
            ON,
            "the first lamp decides"
        );
    }
}
