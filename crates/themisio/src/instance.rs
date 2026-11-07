//! One Themisio window: a launch that only names files (an Explorer double-click, Open With) hands
//! them to the Themisio already running, which opens them and comes to the front.
//!
//! The running Themisio listens on a loopback TCP port and writes the port and a random token to
//! `%LOCALAPPDATA%\Themisio\instance.json` (owner's profile). A later launch sends
//! `{"token": …, "files": [...]}`; a wrong token is ignored.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: String,
    pid: u32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    token: String,
    files: Vec<PathBuf>,
}

fn endpoint_file() -> Option<PathBuf> {
    let base = if cfg!(windows) { std::env::var_os("LOCALAPPDATA").map(PathBuf::from) } else { None };
    base.map(|b| b.join("Themisio")).or_else(crate::settings::dir).map(|d| d.join("instance.json"))
}

fn token() -> String {
    use std::hash::{BuildHasher, RandomState};
    // `RandomState` is keyed from the operating system's random source.
    let a = RandomState::new().hash_one(std::process::id());
    let b = RandomState::new().hash_one(std::time::SystemTime::now());
    format!("{a:016x}{b:016x}")
}

/// Try to give `files` to a running Themisio. `true`: it took them, this launch can exit.
pub fn hand_off(files: &[PathBuf]) -> bool {
    let Some(path) = endpoint_file() else { return false };
    let Ok(bytes) = std::fs::read(&path) else { return false };
    let Ok(ep) = serde_json::from_slice::<Endpoint>(&bytes) else { return false };
    let addr = (Ipv4Addr::LOCALHOST, ep.port).into();
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else { return false };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let message = Message { token: ep.token, files: files.to_vec() };
    let Ok(mut line) = serde_json::to_vec(&message) else { return false };
    line.push(b'\n');
    if stream.write_all(&line).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

/// Become the running Themisio: listen for later launches. Files arrive on the receiver; `wake`
/// is called after each delivery (to repaint and focus the window).
pub fn serve() -> Option<(Receiver<Vec<PathBuf>>, std::sync::Arc<std::sync::Mutex<Option<egui::Context>>>)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let token = token();
    let path = endpoint_file()?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let ep = Endpoint { port, token: token.clone(), pid: std::process::id() };
    std::fs::write(&path, serde_json::to_vec(&ep).ok()?).ok()?;
    let (tx, rx): (Sender<Vec<PathBuf>>, Receiver<Vec<PathBuf>>) = channel();
    let ctx_slot: std::sync::Arc<std::sync::Mutex<Option<egui::Context>>> = Default::default();
    let slot = ctx_slot.clone();
    let spawned = std::thread::Builder::new().name("themisio-instance".into()).spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            // One message per connection, capped at 1 MiB.
            if reader.by_ref().take(1 << 20).read_line(&mut line).is_err() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Message>(&line) else { continue };
            if msg.token != token {
                continue;
            }
            let _ = (&stream).write_all(b"ok\n");
            if tx.send(msg.files).is_err() {
                break;
            }
            if let Ok(guard) = slot.lock()
                && let Some(ctx) = guard.as_ref()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.request_repaint();
            }
        }
    });
    spawned.ok()?;
    Some((rx, ctx_slot))
}

/// The running Themisio is going away: later launches start their own.
pub fn release() {
    if let Some(path) = endpoint_file()
        && let Ok(bytes) = std::fs::read(&path)
        && let Ok(ep) = serde_json::from_slice::<Endpoint>(&bytes)
        && ep.pid == std::process::id()
    {
        let _ = std::fs::remove_file(path);
    }
}

use std::io::Read as _;
