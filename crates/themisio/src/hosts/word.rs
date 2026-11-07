//! WordCraft, set up as `upstream/wordcraft/apps/wordcraft/src/main.rs` sets it up.

use std::path::Path;

use wordcraft_engine::Session;
use wordcraft_ui_egui::{Services, UiState, WordApp};

use super::{Host, path_text};
use super::word_src::{animations, clipboard, file_dialogs, print, region};
use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;

// WordCraft's desktop helpers, compiled in unchanged.

pub struct WordHost {
    app: WordApp,
    dialogs: file_dialogs::Launcher,
    language: Option<Lang>,
}

fn prefs_enabled() -> bool {
    std::env::var_os("WORDCRAFT_NO_PREFS").is_none()
}

fn prefs_path() -> Option<std::path::PathBuf> {
    wordcraft_control_key::settings_dir().map(|b| b.join("ui.json"))
}

fn load_prefs(app: &mut WordApp) {
    if !prefs_enabled() {
        return;
    }
    if let Some(p) = prefs_path()
        && let Ok(bytes) = std::fs::read(&p)
        && let Some(ui) = UiState::from_json(&bytes)
    {
        app.apply_prefs(ui);
    }
}

fn save_prefs(app: &WordApp) {
    if !prefs_enabled() {
        return;
    }
    let Some(p) = prefs_path() else { return };
    if let Some(dir) = p.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        log::warn!("WordCraft preferences: cannot create {}: {e}", dir.display());
        return;
    }
    match serde_json::to_vec_pretty(&app.prefs()) {
        Ok(bytes) => {
            let tmp = p.with_extension("json.themisio-tmp");
            if let Err(e) = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, &p)) {
                let _ = std::fs::remove_file(&tmp);
                log::warn!("WordCraft preferences: cannot save {}: {e}", p.display());
            }
        }
        Err(e) => log::warn!("WordCraft preferences: {e}"),
    }
}

fn load_user_fonts() {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = wordcraft_control_key::settings_dir().map(|d| d.join("fonts")) else { return };
    let (loaded, skipped) = wordcraft_fonts::FontDb::global().load_user_fonts(&dir);
    if !loaded.is_empty() || !skipped.is_empty() {
        log::info!("WordCraft user fonts: {} loaded, {} skipped from {}", loaded.len(), skipped.len(), dir.display());
    }
}

fn load_user_dictionary(app: &mut WordApp) {
    if !prefs_enabled() {
        return;
    }
    if let Some(p) = wordcraft_control_key::settings_dir().map(|b| b.join(Session::USER_DICT_FILE)) {
        wordcraft_proof::load_user_dictionary(&p);
        app.session.user_dict_path = Some(p);
    }
}

fn services(file_dialog: file_dialogs::Hook, ctx: &egui::Context) -> Services {
    Services {
        file_dialog: Some(file_dialog),
        printers: Some(print::discover()),
        print_native: Some(print::submit()),
        cancel_print: Some(print::cancel()),
        clipboard_picture: Some(Box::new(clipboard::picture)),
        // File ▸ Options ▸ "make the default": registers Themisio's file types, then opens
        // Windows' Default apps page, where the user picks Themisio.
        make_default_app: crate::assoc::default_app_hook(),
        templates_dir: wordcraft_control_key::settings_dir().map(|d| d.join("Templates")),
        animations_enabled: Some(animations::hook(ctx)),
        clipboard_text: Some(Box::new(clipboard::text)),
        clipboard_html: Some(Box::new(clipboard::html)),
        set_clipboard_rich: Some(Box::new(clipboard::set_rich)),
        clipboard_rtf: Some(Box::new(clipboard::rtf)),
        clipboard_formats: Some(Box::new(clipboard::formats)),
        ..Default::default()
    }
}

impl WordHost {
    pub fn new(ctx: &egui::Context, lang: Lang) -> Self {
        load_user_fonts();
        let region = region::system();
        let doc = wordcraft_engine::blank_document(&region);
        let (file_dialog, dialogs) = file_dialogs::hook(ctx);
        let mut session = Session::new(doc);
        session.system_region = region;
        let mut app = WordApp::new(session, services(file_dialog, ctx));
        load_prefs(&mut app);
        load_user_dictionary(&mut app);
        wordcraft_engine::packs::use_font_store(true);
        if app.session.prefs.region.is_some() {
            let doc = app.session.blank_document();
            app.replace_startup_document(doc);
        }
        app.watch_desktop_theme(ctx);
        if prefs_enabled()
            && let Some(dir) = wordcraft_control_key::settings_dir()
        {
            let ctx = ctx.clone();
            if let Err(e) = app.session.enable_recovery(dir.join("recovery"), std::sync::Arc::new(move || ctx.request_repaint())) {
                log::warn!("WordCraft local recovery unavailable: {e}");
            }
        }
        app.set_window_builder(crate::window_builder("Themisio"));
        // One window; Themisio's tab bar switches between the open documents.
        app.use_single_window();
        app.integrated_titlebar = false;
        app.ui.language = lang.code().to_string();
        Self { app, dialogs, language: None }
    }

}

impl Host for WordHost {
    fn kind(&self) -> AppKind {
        AppKind::Word
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(lang) = self.language.take() {
            self.app.ui.language = lang.code().to_string();
        }
        self.app.logic(ctx);
        self.dialogs.show(frame);
        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing && (self.app.closes_a_window() || !self.app.close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if self.app.quit_requested && !self.app.printing.busy() {
            // File ▸ Exit, or the save prompt let the close go on: Themisio decides what closes.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui);
        self.dialogs.show(frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw);
    }

    fn open(&mut self, _ctx: &egui::Context, path: &Path, _intent: Intent) {
        let text = path_text(path);
        match path.to_str() {
            Some(p) => {
                if let Err(e) = self.app.run("file.open", serde_json::json!({ "path": p })) {
                    self.app.report_open_failure(p, &e);
                }
            }
            // A name that isn't Unicode: read the bytes here, as WordCraft's main does.
            None => {
                let opened = wordcraft_engine::io::read_document(path).and_then(|bytes| {
                    self.app.run(
                        "file.open",
                        serde_json::json!({"path": text, "data": wordcraft_engine::cmd::insert::base64_encode(&bytes)}),
                    )
                });
                match opened {
                    Ok(r) if r.get("pending").is_none() => self.app.session.path = Some(path.into()),
                    Ok(_) => {}
                    Err(e) => {
                        self.app.report_open_failure(&text, &e);
                    }
                }
            }
        }
    }

    fn new_document(&mut self, _ctx: &egui::Context) {
        if let Err(e) = self.app.run("file.new", serde_json::json!({})) {
            log::warn!("WordCraft file.new: {e}");
        }
    }

    fn set_language(&mut self, lang: Lang) {
        self.language = Some(lang);
    }

    fn set_dark(&mut self, dark: bool) {
        let _ = self.app.run("ui.theme", serde_json::json!({"value": if dark { "dark" } else { "light" }}));
    }

    fn tabs(&self) -> Option<Vec<super::Tab>> {
        let docs = self.app.session.documents();
        Some(docs.into_iter().map(|d| super::Tab { id: d.id, title: d.title, dirty: d.dirty, active: d.active }).collect())
    }

    fn activate_tab(&mut self, id: u64) {
        if let Err(e) = self.app.session.activate(id) {
            log::warn!("Documents: switch to {id}: {e}");
        }
    }

    fn close_tab(&mut self, id: u64) {
        self.activate_tab(id);
        if let Err(e) = self.app.run("file.close", serde_json::json!({})) {
            log::warn!("Documents: close: {e}");
        }
    }

    fn show_start_screen(&mut self) {
        // WordCraft's start screen (templates, recent documents), as when it starts without files.
        self.app.start_screen();
    }

    fn on_exit(&mut self) {
        print::finish_pending_jobs();
        self.app.session.finish_recovery();
        save_prefs(&self.app);
    }
}
