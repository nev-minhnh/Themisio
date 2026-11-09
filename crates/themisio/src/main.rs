//! Themisio – Make your documents glow.
//!
//! One Windows 11 app for documents, presentations, workbooks and PDFs: WordCraft, DeckCraft,
//! GridCraft and PdfCraft (storytold, MIT OR Apache-2.0) side by side in one window, each with
//! all of its own features and its own look.
//!
//! Usage: `themisio [--app docs|slides|sheets|pdf] [--new docs|slides|sheets|pdf] [FILE…]`
//!        `themisio --register-file-types` / `--unregister-file-types`
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod app_kind;
mod assoc;
mod ctx_state;
mod deck_i18n;
mod gpu;
mod home;
mod hosts;
mod i18n;
mod instance;
mod logging;
mod memory;
mod motion;
mod rail;
mod route;
mod settings;
mod shell;
mod theme;

use std::path::PathBuf;

use app_kind::AppKind;

/// Themisio's version (About, `--version`, the file names of a release).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// When `main` started, for the startup timings in the log.
pub static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// The apps Themisio hosts, for About.
pub const UPSTREAMS: &[(&str, &str)] = &[
    ("Documents — github.com/storytold/wordcraft", "https://github.com/storytold/wordcraft"),
    ("Presentation — github.com/storytold/deckcraft", "https://github.com/storytold/deckcraft"),
    ("CalcSheet — github.com/storytold/gridcraft", "https://github.com/storytold/gridcraft"),
    ("PDF — github.com/storytold/pdfcraft", "https://github.com/storytold/pdfcraft"),
];

const USAGE: &str = "\
Themisio – Make your documents glow.

Usage: themisio [options] [FILE...]

  --app docs|slides|sheets|pdf   show this app
  --new docs|slides|sheets|pdf   start a new blank document in this app
  --new-window                   don't hand the files to an Themisio that is already running
  --register-file-types          list Themisio under Open With for its file types (this user)
  --unregister-file-types        remove those entries
  --help, -h                     print this help
  --version, -V                  print the version
";

/// A new window for an app (GridCraft's other workbooks, WordCraft's other documents).
pub fn window_builder(title: &str) -> egui::ViewportBuilder {
    let mut b = egui::ViewportBuilder::default()
        .with_title(title)
        .with_app_id("themisio")
        .with_inner_size([1280.0, 840.0])
        .with_min_inner_size([760.0, 480.0])
        .with_drag_and_drop(true);
    if let Some(icon) = icon() {
        b = b.with_icon(icon);
    }
    b
}

fn icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo-256.png")).map_err(|e| log::warn!("app icon: {e}")).ok()
}

/// The apps title the window after their document; it reads "… - Themisio" instead.
pub fn window_title(title: &str) -> String {
    let mut t = title.to_string();
    for name in ["WordCraft", "DeckCraft", "GridCraft", "PdfCraft", "PrintCraft"] {
        if t.contains(name) {
            t = t.replace(name, "Themisio");
        }
    }
    if !t.contains("Themisio") {
        t = if t.trim().is_empty() { "Themisio".into() } else { format!("{t} - Themisio") };
    }
    t
}

fn app_arg(s: Option<String>) -> Option<AppKind> {
    let s = s?.to_ascii_lowercase();
    match s.as_str() {
        "docs" | "doc" | "word" | "wordcraft" => Some(AppKind::Word),
        "slides" | "deck" | "deckcraft" | "presentation" => Some(AppKind::Deck),
        "sheets" | "grid" | "gridcraft" | "workbook" => Some(AppKind::Grid),
        "pdf" | "pdfcraft" => Some(AppKind::Pdf),
        other => AppKind::from_id(other),
    }
}

/// `file:///C:/x.pdf` (some launchers pass URIs) to a plain path.
fn path_from_arg(arg: &str) -> PathBuf {
    let Some(rest) = arg.strip_prefix("file:///") else { return PathBuf::from(arg) };
    let mut out = Vec::with_capacity(rest.len());
    let bytes = rest.as_bytes();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%'
            && let Some(hex) = rest.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(hex, 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b);
        i += 1;
    }
    let text = String::from_utf8(out).unwrap_or_else(|_| rest.to_string());
    if cfg!(windows) { PathBuf::from(text) } else { PathBuf::from(format!("/{text}")) }
}

fn main() -> eframe::Result {
    // Windows: WordCraft prints from an isolated helper process, which is this executable
    // started with a flag; it never opens a window.
    #[cfg(windows)]
    if let Some(code) = hosts::word_src::print::isolated::helper_mode() {
        std::process::exit(code);
    }
    let _ = STARTED.set(std::time::Instant::now());
    let before_main = memory::since_process_start_ms();
    logging::install();
    logging::install_panic_hook();
    if let Some(ms) = before_main {
        log::info!("startup: {ms} ms from process creation to main (loading the program from disk, antivirus scans)");
    }

    let mut launch = shell::Launch::default();
    let mut new_window = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                let _ = std::io::Write::write_all(&mut std::io::stdout(), USAGE.as_bytes());
                return Ok(());
            }
            Some("--version" | "-V") => {
                let _ = std::io::Write::write_all(&mut std::io::stdout(), format!("Themisio {VERSION}\n").as_bytes());
                return Ok(());
            }
            Some("--register-file-types") => {
                if let Err(e) = assoc::register() {
                    log::error!("{e}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            Some("--unregister-file-types") => {
                if let Err(e) = assoc::unregister() {
                    log::error!("{e}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            Some("--app") => launch.app = app_arg(args.next().and_then(|a| a.into_string().ok())),
            Some("--new") => launch.new = app_arg(args.next().and_then(|a| a.into_string().ok())),
            Some("--new-window") => new_window = true,
            Some(a) if a.starts_with("-psn_") => {}
            Some(a) => launch.files.push(path_from_arg(a)),
            None => launch.files.push(PathBuf::from(arg)),
        }
    }
    launch.files = launch.files.into_iter().map(|p| std::path::absolute(&p).unwrap_or(p)).collect();

    // Files for an Themisio that is already open go there.
    let only_files = launch.app.is_none() && launch.new.is_none() && !launch.files.is_empty();
    if only_files && !new_window && instance::hand_off(&launch.files) {
        return Ok(());
    }
    let server = if new_window { None } else { instance::serve() };
    log::info!("Themisio {VERSION} ({} {})", std::env::consts::OS, std::env::consts::ARCH);
    log::info!("DeckCraft Vietnamese dictionary: {} entries", deck_i18n::size());

    let settings = settings::Settings::load();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("Themisio {VERSION}"))
        .with_app_id("themisio")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Some(icon) = icon() {
        viewport = viewport.with_icon(icon);
    }
    let mut native = eframe::NativeOptions {
        viewport,
        persistence_path: settings::dir().map(|d| d.join("app.ron")),
        renderer: if settings.renderer == settings::Renderer::OpenGl || std::env::var("THEMISIO_RENDERER").is_ok_and(|v| v == "gl") {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        ..Default::default()
    };
    // The apps install their own fonts; reading every installed face into each pass is wasted.
    native.system_font_fallback = false;
    gpu::configure(&mut native, settings.presentation);

    let started = std::rc::Rc::new(std::cell::Cell::new(false));
    let launch = std::rc::Rc::new(std::cell::RefCell::new(Some((launch, server))));
    let creator = |started: std::rc::Rc<std::cell::Cell<bool>>, launch: std::rc::Rc<std::cell::RefCell<Option<_>>>| -> eframe::AppCreator<'static> {
        Box::new(move |cc| {
            started.set(true);
            let (launch, server) = launch.borrow_mut().take().unwrap_or_default();
            Ok(Box::new(shell::Themisio::new(cc, launch, server)))
        })
    };
    let result = eframe::run_native("Themisio", native, creator(started.clone(), launch.clone()));
    let result = match result {
        // Old or unusual GPUs: wgpu can't start, OpenGL usually can (as PdfCraft does).
        Err(e @ eframe::Error::Wgpu(_)) if !started.get() => {
            log::error!("the GPU renderer (wgpu) didn't start: {e}. Starting with OpenGL instead.");
            let native = eframe::NativeOptions {
                viewport: window_builder("Themisio").with_inner_size([1440.0, 900.0]),
                persistence_path: settings::dir().map(|d| d.join("app.ron")),
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            };
            eframe::run_native("Themisio", native, creator(started, launch))
        }
        other => other,
    };
    instance::release();
    result
}
