//! Log records from Themisio and the four apps go to standard error and
//! `%LOCALAPPDATA%\Themisio\logs\themisio.log` (the previous run's log is kept as `themisio.old.log`).
//! `RUST_LOG=debug` (or `trace`, `warn`…) sets the level; the default is `info`.

use std::io::Write;
use std::sync::Mutex;

struct Logger {
    level: log::LevelFilter,
    file: Mutex<Option<std::fs::File>>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // Busy third-party crates stay at warnings unless asked.
        let noisy = ["wgpu", "naga", "winit", "zbus", "tracing", "calloop", "sctk", "polling", "async_io", "accesskit"];
        let target = metadata.target();
        if self.level < log::LevelFilter::Debug && noisy.iter().any(|n| target.starts_with(n)) {
            return metadata.level() <= log::Level::Warn;
        }
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.target(),
            record.args()
        );
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Ok(mut file) = self.file.lock()
            && let Some(f) = file.as_mut()
        {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

fn log_dir() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) { std::env::var_os("LOCALAPPDATA") } else { None };
    base.map(|b| std::path::PathBuf::from(b).join("Themisio"))
        .or_else(crate::settings::dir)
        .map(|d| d.join("logs"))
}

pub fn install() {
    let level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|v| v.split(',').find_map(|part| (!part.contains('=')).then(|| part.trim().parse().ok()).flatten()))
        .unwrap_or(log::LevelFilter::Info);
    let file = log_dir().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("themisio.log");
        let _ = std::fs::rename(&path, dir.join("themisio.old.log"));
        std::fs::File::create(path).ok()
    });
    let logger = Box::leak(Box::new(Logger { level, file: Mutex::new(file) }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}

/// Every panic is logged (with a backtrace when RUST_BACKTRACE is set). The apps guard their
/// commands, opens and saves with `catch_unwind`; this records what they caught too.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let trace = std::backtrace::Backtrace::capture();
        let report = if trace.status() == std::backtrace::BacktraceStatus::Captured {
            format!("internal error: {info}\n{trace}")
        } else {
            format!("internal error: {info}")
        };
        if log::log_enabled!(log::Level::Error) {
            log::error!("{report}");
        } else {
            let _ = std::io::stderr().write_all(format!("themisio: {report}\n").as_bytes());
        }
    }));
}
