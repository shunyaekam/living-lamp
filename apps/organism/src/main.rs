//! `living-lamp`: the organism. Every input (a person's commands, the screen, an agent) arrives as a message; the organism
//! is the only thing that drives the lamp, by streaming colour to lampbus. See `docs/MODEL.md`.
//!
//! ```text
//! living-lamp [--listen 127.0.0.1:8791] [--screen 127.0.0.1:8792] [--to 127.0.0.1:8789] [--lampbus 127.0.0.1:8787]
//!             [--state FILE] [--fps 20] [--seed N]       run the organism
//! living-lamp send "person look 255 0 0 50"              send a message to a running organism and print its reply
//! ```

mod messages;
mod motor;
mod organism;

use messages::{parse, screen_frame};
use organism::Organism;
use std::io::ErrorKind;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const HELP: &str = "living-lamp: the organism that drives the lamp through lampbus

USAGE
  living-lamp [options]               run the organism
  living-lamp send <message...>       send one message to a running organism and print its reply

OPTIONS
  --listen host:port    where messages arrive (default 127.0.0.1:8791; set lampbus's \"controller\" to this)
  --screen host:port    where the screen app's frames arrive (default 127.0.0.1:8792)
  --to host:port        lampbus's UDP stream input (default 127.0.0.1:8789)
  --lampbus host:port   lampbus's HTTP API, for power and relayed effects (default 127.0.0.1:8787)
  --state FILE          where what it has learned is kept (default ~/.local/state/living-lamp/model)
  --fps N               frames per second (default 40; lampbus writes at most 40 a second)
  --seed N              random seed (default: from the clock)

MESSAGES  person look R G B [PERCENT] | person brightness PERCENT | person power on|off|toggle | person act JSON
          agent working|done|needs-you|error|idle | lively 0..4 | release | sleep | wake | status";

struct Opts {
    listen: String,
    screen: String,
    to: String,
    lampbus: String,
    state: PathBuf,
    fps: f64,
    seed: u64,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn token() -> Result<String, String> {
    if let Ok(t) = std::env::var("LAMPBUS_TOKEN") {
        return Ok(t);
    }
    let dir = std::env::var_os("LAMPBUS_HOME")
        .map_or_else(|| home().join(".config").join("lampbus"), PathBuf::from);
    std::fs::read_to_string(dir.join("token"))
        .map(|t| t.trim().to_string())
        .map_err(|e| {
            format!(
                "cannot read lampbus's token ({}): {e}",
                dir.join("token").display()
            )
        })
}

fn parse_opts(args: Vec<String>) -> Result<Option<Opts>, String> {
    let mut o = Opts {
        listen: "127.0.0.1:8791".into(),
        screen: "127.0.0.1:8792".into(),
        to: "127.0.0.1:8789".into(),
        lampbus: "127.0.0.1:8787".into(),
        state: home().join(".local/state/living-lamp/model"),
        fps: 40.0,
        seed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "-h" | "--help" => return Ok(None),
            "--listen" => o.listen = value("--listen")?,
            "--screen" => o.screen = value("--screen")?,
            "--to" => o.to = value("--to")?,
            "--lampbus" => o.lampbus = value("--lampbus")?,
            "--state" => o.state = PathBuf::from(value("--state")?),
            "--fps" => {
                o.fps = value("--fps")?
                    .parse::<f64>()
                    .ok()
                    .filter(|f| (1.0..=60.0).contains(f))
                    .ok_or("--fps must be 1 to 60")?
            }
            "--seed" => {
                o.seed = value("--seed")?
                    .parse()
                    .map_err(|_| "--seed must be a number")?
            }
            other => return Err(format!("unknown option {other:?}; try --help")),
        }
    }
    Ok(Some(o))
}

/// `living-lamp send ...`: one message to a running organism.
fn send(args: &[String]) -> Result<(), String> {
    let line = args.join(" ");
    let sock = UdpSocket::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    sock.send_to(line.as_bytes(), "127.0.0.1:8791")
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 512];
    let n = sock
        .recv(&mut buf)
        .map_err(|_| "no answer (is it running?)".to_string())?;
    println!(
        "{}",
        String::from_utf8_lossy(buf.get(..n).unwrap_or_default()).trim()
    );
    Ok(())
}

fn bind(addr: &str) -> Result<UdpSocket, String> {
    let s = UdpSocket::bind(addr)
        .map_err(|e| format!("cannot listen on {addr} (is another one running?): {e}"))?;
    s.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok(s)
}

/// The wire frame for lampbus's UDP input: `r g b hi lo`, the level in hundredths of a percent.
fn wire(l: lamp_model::light::Light) -> [u8; 5] {
    let [hi, lo] = ((l.level * 100.0).round().clamp(0.0, 10_000.0) as u16).to_be_bytes();
    [l.rgb[0], l.rgb[1], l.rgb[2], hi, lo]
}

fn run(o: &Opts) -> Result<(), String> {
    let control = bind(&o.listen)?;
    let screen = bind(&o.screen)?;
    // send from the loopback address when lampbus is local, so nothing is bound to the network by accident
    let local = o.to.starts_with("127.") || o.to.starts_with("localhost");
    let out = UdpSocket::bind(if local { "127.0.0.1:0" } else { "0.0.0.0:0" })
        .map_err(|e| e.to_string())?;
    let motor = motor::spawn(o.lampbus.clone(), token()?);
    let mut organism = Organism::new(o.seed);
    organism.start_asleep();
    match std::fs::read_to_string(&o.state) {
        Ok(text) => match organism.load(&text) {
            Ok(()) => eprintln!(
                "living-lamp: restored what it had learned from {}",
                o.state.display()
            ),
            Err(e) => eprintln!("living-lamp: ignoring {}: {e}", o.state.display()),
        },
        Err(_) => eprintln!("living-lamp: newborn (nothing learned yet)"),
    }
    eprintln!(
        "living-lamp: alive; messages on {}, screen on {}, streaming to {}",
        o.listen, o.screen, o.to
    );
    let (start, period) = (Instant::now(), Duration::from_secs_f64(1.0 / o.fps));
    let dt = 1.0 / o.fps;
    let (mut last_sent, mut last_frame, mut last_save) = (Instant::now(), None, Instant::now());
    let mut buf = [0u8; 4096];
    loop {
        let tick = Instant::now();
        let now = start.elapsed().as_secs_f64();
        loop {
            match control.recv_from(&mut buf) {
                Ok((n, from)) => {
                    let text =
                        String::from_utf8_lossy(buf.get(..n).unwrap_or_default()).to_string();
                    let mut reply = "ok".to_string();
                    for line in text.lines().filter(|l| !l.trim().is_empty()) {
                        if line.trim() != "status" {
                            eprintln!("living-lamp: message {:?} from {from}", line.trim());
                        }
                        match parse(line).and_then(|m| organism.handle(m, now)) {
                            Ok(r) if reply == "ok" => reply = r,
                            Ok(_) => {}
                            Err(e) => {
                                reply = format!("err {e}");
                                break;
                            }
                        }
                    }
                    let _ = control.send_to(reply.as_bytes(), from);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        while let Ok((n, _)) = screen.recv_from(&mut buf) {
            if let Some((rgb, percent)) = buf.get(..n).and_then(screen_frame) {
                organism.screen(rgb, percent, now);
            }
        }
        organism.resync(motor.lamp.load(std::sync::atomic::Ordering::Relaxed), now);
        let frame = organism.tick(dt, now);
        for c in organism.take_motor() {
            eprintln!("living-lamp: motor {c}");
            motor.control(&c);
        }
        if let Some(l) = frame {
            let w = wire(l);
            // repeat a still frame: lampbus drops a stream that has been quiet for two seconds
            if last_frame != Some(w) || last_sent.elapsed() >= Duration::from_millis(500) {
                let _ = out.send_to(&w, &o.to);
                last_sent = Instant::now();
                last_frame = Some(w);
            }
        }
        if last_save.elapsed() >= Duration::from_secs(300) {
            last_save = Instant::now();
            if let Some(dir) = o.state.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(&o.state, organism.save()) {
                eprintln!("living-lamp: cannot save to {}: {e}", o.state.display());
            }
        }
        if let Some(rest) = period.checked_sub(tick.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("send") => send(args.get(1..).unwrap_or_default()),
        _ => match parse_opts(args) {
            Ok(None) => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            Ok(Some(o)) => run(&o),
            Err(e) => Err(e),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("living-lamp: {e}");
            ExitCode::FAILURE
        }
    }
}
