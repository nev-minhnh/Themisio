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
    seen_status: Option<(String, f64)>,
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
        Self { app, lang, seen_status: None }
    }
}

impl DeckHost {
    /// The paragraphs of shape `id`'s text.
    fn paragraphs(&mut self, id: u32) -> Option<Vec<String>> {
        let v = self.app.run("text.get", serde_json::json!({"id": id})).ok()?;
        Some(v["paragraphs"].as_array()?.iter().map(|p| p["text"].as_str().unwrap_or("").to_string()).collect())
    }

    /// Edit shape `id`'s text (if it isn't being edited already).
    fn edit_shape(&mut self, id: u64) -> Result<(), String> {
        let editing = self.app.session.active().and_then(|d| d.selection.text.as_ref()).is_some_and(|t| u64::from(t.shape.0) == id);
        if editing {
            return Ok(());
        }
        let _ = self.app.run("selection.set", serde_json::json!({"ids": [id]}));
        self.app.run("text.edit", serde_json::json!({"id": id})).map(|_| ()).map_err(|e| e.to_string())
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

    fn command(&mut self, cmd: super::DocCommand) -> Result<(), String> {
        let id = match cmd {
            super::DocCommand::Save => "file.save",
            super::DocCommand::SaveAs => "file.saveAs",
            super::DocCommand::Print => "app.print",
        };
        self.app.run(id, serde_json::json!({})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn take_notices(&mut self) -> Vec<String> {
        let mut out: Vec<String> = self.app.toast.take().map(|t| t.text).into_iter().collect();
        if let Some(m) = &self.app.status
            && self.seen_status.as_ref() != Some(m)
        {
            self.seen_status = Some(m.clone());
            out.push(m.0.clone());
        }
        out
    }

    fn ai_target(&mut self) -> Option<super::AiTarget> {
        let d = self.app.session.active()?;
        let (shape, sel) = match &d.selection.text {
            Some(t) if !t.notes && t.cell.is_none() => (t.shape.0, Some((t.anchor, t.caret))),
            Some(_) => return None,
            None if d.selection.shapes.len() == 1 => (d.selection.shapes[0].0, None),
            None => return None,
        };
        let paras = self.paragraphs(shape)?;
        let (text, p0, p1, selection) = match sel {
            Some((a, c)) if a != c => {
                let (s, e) = if a <= c { (a, c) } else { (c, a) };
                let mut out = String::new();
                for p in s.0..=e.0 {
                    let line: Vec<char> = paras.get(p)?.chars().collect();
                    let from = if p == s.0 { s.1.min(line.len()) } else { 0 };
                    let to = if p == e.0 { e.1.min(line.len()) } else { line.len() };
                    if p > s.0 {
                        out.push('\n');
                    }
                    out.extend(line[from..to.max(from)].iter());
                }
                (out, s.0, e.0, true)
            }
            Some((_, c)) => (paras.get(c.0)?.clone(), c.0, c.0, false),
            None => (paras.join("\n"), 0, paras.len().saturating_sub(1), false),
        };
        if text.trim().is_empty() {
            return None;
        }
        Some(super::AiTarget { text, selection, anchor: serde_json::json!({"shape": shape, "p0": p0, "p1": p1}), screen: None })
    }

    fn ai_replace(&mut self, target: &super::AiTarget, original: &str, replacement: &str) -> Result<(), String> {
        let shape = target.anchor["shape"].as_u64().ok_or("no shape")?;
        let p0 = target.anchor["p0"].as_u64().unwrap_or(0) as usize;
        let p1 = target.anchor["p1"].as_u64().unwrap_or(0) as usize;
        self.edit_shape(shape)?;
        let paras = self.paragraphs(shape as u32).ok_or("The text box has gone")?;
        for (p, line) in paras.iter().enumerate().skip(p0).take(p1.saturating_sub(p0) + 1) {
            if let Some(byte) = line.find(original) {
                let c0 = line[..byte].chars().count();
                let c1 = c0 + original.chars().count();
                self.app.run("text.select", serde_json::json!({"anchor": [p, c0], "caret": [p, c1]})).map_err(|e| e.to_string())?;
                return self.app.run("text.insert", serde_json::json!({"text": replacement})).map(|_| ()).map_err(|e| e.to_string());
            }
        }
        Err("The text has changed since it was checked".into())
    }

    fn ai_revision(&mut self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(d) = self.app.session.active() {
            d.uid.hash(&mut h);
            d.revision.hash(&mut h);
            d.selection.slide.hash(&mut h);
        }
        h.finish()
    }

    fn ai_units(&mut self) -> Vec<(serde_json::Value, String)> {
        let Some(d) = self.app.session.active() else { return Vec::new() };
        d.shapes()
            .iter()
            .filter_map(|sh| {
                let text = sh.text.as_ref()?.text();
                (text.trim().chars().count() >= 3).then(|| (serde_json::json!({"shape": sh.id.0}), text))
            })
            .collect()
    }

    fn ai_apply(&mut self, key: &serde_json::Value, original: &str, replacement: &str) -> Result<(), String> {
        let shape = key["shape"].as_u64().ok_or("no text box")?;
        let target = super::AiTarget { text: original.into(), selection: false, anchor: serde_json::json!({"shape": shape, "p0": 0, "p1": 10_000}), screen: None };
        self.ai_replace(&target, original, replacement)
    }

    fn ai_focus(&mut self, key: &serde_json::Value, original: &str) {
        let Some(shape) = key["shape"].as_u64() else { return };
        if self.edit_shape(shape).is_err() {
            return;
        }
        if let Some(paras) = self.paragraphs(shape as u32) {
            for (p, line) in paras.iter().enumerate() {
                if let Some(byte) = line.find(original) {
                    let c0 = line[..byte].chars().count();
                    let c1 = c0 + original.chars().count();
                    let _ = self.app.run("text.select", serde_json::json!({"anchor": [p, c0], "caret": [p, c1]}));
                    return;
                }
            }
        }
    }

    fn ai_insert(&mut self, text: &str) -> Result<(), String> {
        let editing = self.app.session.active().is_some_and(|d| d.selection.text.is_some());
        if !editing {
            return Err("Click into a text box first.".into());
        }
        self.app.run("text.insert", serde_json::json!({"text": text})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn ai_replace_all(&mut self, target: &super::AiTarget, text: &str) -> Result<(), String> {
        let shape = target.anchor["shape"].as_u64().ok_or("no shape")?;
        let p0 = target.anchor["p0"].as_u64().unwrap_or(0) as usize;
        let p1 = target.anchor["p1"].as_u64().unwrap_or(0) as usize;
        if target.selection {
            // The selection is still what was read: replace it.
            if self.ai_target().is_some_and(|t| t.text == target.text) {
                return self.app.run("text.insert", serde_json::json!({"text": text})).map(|_| ()).map_err(|e| e.to_string());
            }
            return Err("The selection has changed".into());
        }
        self.edit_shape(shape)?;
        let paras = self.paragraphs(shape as u32).ok_or("The text box has gone")?;
        let end = paras.get(p1).map_or(0, |l| l.chars().count());
        self.app.run("text.select", serde_json::json!({"anchor": [p0, 0], "caret": [p1, end]})).map_err(|e| e.to_string())?;
        self.app.run("text.insert", serde_json::json!({"text": text})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn show_start_screen(&mut self) {
        // DeckCraft's own start page: File › Home (new presentation, themes, recent files).
        self.app.open_backstage(deckcraft_ui_egui::backstage::Page::Home);
    }

    fn is_blank(&self) -> bool {
        self.app.session.documents().iter().all(|d| d.path.is_none() && !d.is_dirty())
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
