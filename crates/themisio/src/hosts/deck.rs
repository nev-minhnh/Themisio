//! DeckCraft, set up as `upstream/deckcraft/apps/deckcraft/src/main.rs` sets it up.
//!
//! DeckCraft has no string catalog yet (its interface is English only), so its Vietnamese comes
//! from Themisio: `crate::deck_i18n` translates its interface strings while it draws.

use std::path::Path;

use deckcraft_engine::Session;
use deckcraft_ui_egui::{Services, SlideApp};

use super::{Host, path_text};
use super::deck_src::{audio, pasteboard};
use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;


pub struct DeckHost {
    app: SlideApp,
    /// The interface language Themisio shows DeckCraft in (`deck_i18n`); kept for its prefs.
    #[allow(dead_code)]
    lang: Lang,
}

/// DeckCraft's settings folder: `ui.json`, `prefs.json` (shared with a standalone DeckCraft).
fn config_dir() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("DeckCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
            .map(|c| c.join("deckcraft"))
    }
}

fn prefs_enabled() -> bool {
    std::env::var_os("DECKCRAFT_NO_PREFS").is_none()
}

fn load_prefs(app: &mut SlideApp) {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = config_dir() else { return };
    let (ui, prefs) = deckcraft_ui_egui::settings::load(&dir);
    if let Some(ui) = ui {
        app.ui = ui;
    }
    if let Some(prefs) = prefs {
        app.session.prefs = prefs;
    }
}

fn save_prefs(app: &SlideApp) {
    if !prefs_enabled() {
        return;
    }
    if let Some(dir) = config_dir() {
        deckcraft_ui_egui::settings::save(&dir, &app.ui, &app.session.prefs);
    }
}

fn services() -> Services {
    Services {
        pick_open: Some(Box::new(|purpose: &str| {
            let d = rfd::FileDialog::new();
            let d = match purpose {
                "picture" => d.add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"]),
                "audio" => d.add_filter(
                    "Audio",
                    &["wav", "mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "aif", "aiff", "aifc", "caf", "wma", "weba", "mka"],
                ),
                "video" => d.add_filter("Video", &["mp4", "m4v", "mov", "webm", "mkv", "wmv"]),
                "theme" => d.add_filter("Themes and presentations", &["thmx", "pptx", "potx", "ppsx"]).add_filter("Office Theme", &["thmx"]),
                _ => d
                    .add_filter("Presentations", &["deckcraft", "pptx", "potx", "ppsx"])
                    .add_filter("Presentation (.deckcraft)", &["deckcraft"])
                    .add_filter("PowerPoint", &["pptx", "potx", "ppsx"])
                    .add_filter("Outline", &["txt", "md"]),
            };
            d.pick_file().map(|p| p.to_string_lossy().to_string())
        })),
        pick_save: Some(Box::new(|name: &str| rfd::FileDialog::new().set_file_name(name).save_file().map(|p| p.to_string_lossy().to_string()))),
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
        clipboard_image: Some(Box::new(pasteboard::image_png) as Box<dyn FnMut() -> Option<Vec<u8>>>),
        clipboard_html: Some(Box::new(pasteboard::html) as Box<dyn FnMut() -> Option<String>>),
        clipboard_set_html: Some(Box::new(pasteboard::set_html) as Box<dyn FnMut(&str, &str) -> bool>),
        // Media is mixed and clocked by the output device; `DECKCRAFT_NO_AUDIO` plays silently.
        audio_out: std::env::var_os("DECKCRAFT_NO_AUDIO")
            .is_none()
            .then(|| Box::new(audio::CpalOut::default()) as Box<dyn deckcraft_media::AudioOut>),
        ..Default::default()
    }
}

impl DeckHost {
    pub fn new(ctx: &egui::Context, lang: Lang) -> Self {
        deckcraft_ui_egui::install_ime_pass_hooks(ctx);
        let mut app = SlideApp::new(Session::new(), services());
        load_prefs(&mut app);
        // AutoRecover: reopen what a previous run left unsaved, then keep it current.
        if std::env::var_os("DECKCRAFT_NO_RECOVERY").is_none() {
            app.session.recovery_dir = deckcraft_engine::recovery::default_dir();
            if let Ok(v) = app.session.execute("file.recovery.open", &serde_json::json!({}))
                && let Some(n) = v.get("opened").and_then(|o| o.as_array()).map(Vec::len).filter(|n| *n > 0)
            {
                app.set_status(format!("Recovered {n} unsaved presentation{} from the last session", if n == 1 { "" } else { "s" }));
            }
        }
        app.integrated_titlebar = false;
        Self { app, lang }
    }
}

impl Host for DeckHost {
    fn kind(&self) -> AppKind {
        AppKind::Deck
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.logic(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw);
    }

    fn open(&mut self, _ctx: &egui::Context, path: &Path, intent: Intent) {
        let p = path_text(path);
        // `open_from_system` starts the show for a .ppsx given last, as PowerPoint does.
        for (f, e) in self.app.open_from_system(std::slice::from_ref(&p)) {
            log::error!("DeckCraft {f}: {e}");
        }
        if intent == Intent::SlideShow && !crate::route::extension(path).eq("ppsx") {
            self.app.start_show(0, false);
        }
    }

    fn new_document(&mut self, _ctx: &egui::Context) {
        if let Err(e) = self.app.run("file.new", serde_json::json!({})) {
            log::warn!("DeckCraft file.new: {e}");
        }
    }

    fn set_language(&mut self, lang: Lang) {
        self.lang = lang;
    }

    fn set_dark(&mut self, dark: bool) {
        let _ = self.app.run("view.dark", serde_json::json!({"on": dark}));
    }

    fn tabs(&self) -> Option<Vec<super::Tab>> {
        let active = self.app.session.active_index();
        Some(
            self.app
                .session
                .documents()
                .iter()
                .enumerate()
                .map(|(i, d)| super::Tab { id: i as u64, title: d.title(), dirty: d.is_dirty(), active: active == Some(i) })
                .collect(),
        )
    }

    fn activate_tab(&mut self, id: u64) {
        let _ = self.app.run("window.next", serde_json::json!({"index": id}));
    }

    fn close_tab(&mut self, id: u64) {
        self.activate_tab(id);
        self.app.close_presentation();
    }

    fn on_exit(&mut self) {
        save_prefs(&self.app);
    }
}
