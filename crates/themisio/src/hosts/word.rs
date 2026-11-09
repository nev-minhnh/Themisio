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
    /// The last status message handed to Themisio's notifications.
    seen_status: Option<(String, f64)>,
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
        // File › Options › "make the default": registers Themisio's file types, then opens
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
        // WordCraft's default user is "WordCraft User" (initials "WU" on the account button,
        // author of comments and tracked changes). Until the user picks a name in Options,
        // use theirs: the name Office has, else the Windows account's.
        if app.session.author == "WordCraft User"
            && let Some(name) = crate::hosts::user_name()
        {
            app.session.author = name;
        }
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
        Self { app, dialogs, language: None, seen_status: None }
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
            // File › Exit, or the save prompt let the close go on: Themisio decides what closes.
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
        // With no document open (the last one was closed), WordCraft keeps a stand-in that is
        // not a document the user has: no tab for it.
        let none = !self.app.has_document();
        Some(docs.into_iter().filter(|d| !(none && d.active)).map(|d| super::Tab { id: d.id, title: d.title, dirty: d.dirty, active: d.active }).collect())
    }

    fn command(&mut self, cmd: super::DocCommand) -> Result<(), String> {
        let id = match cmd {
            super::DocCommand::Save => "file.save",
            super::DocCommand::SaveAs => "file.saveAs",
            super::DocCommand::Print => "file.print",
        };
        self.app.run(id, serde_json::json!({})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn take_notices(&mut self) -> Vec<String> {
        match &self.app.status_msg {
            Some(m) if self.seen_status.as_ref() != Some(m) => {
                self.seen_status = Some(m.clone());
                vec![m.0.clone()]
            }
            _ => Vec::new(),
        }
    }

    fn ai_target(&mut self) -> Option<super::AiTarget> {
        let screen = wordcraft_ui_egui::canvas::cursor::caret_screen(&mut self.app).map(|(p, h)| p + egui::vec2(0.0, h));
        let s = &self.app.session;
        let (a, b) = s.sel.ordered();
        if a != b {
            let text = s.selected_text();
            if text.trim().is_empty() {
                return None;
            }
            return Some(super::AiTarget { text, selection: true, anchor: serde_json::json!({"start": a, "end": b}), screen });
        }
        let para = s.doc.para_at(&s.sel.focus)?;
        let text = para.plain_text();
        if text.trim().is_empty() {
            return None;
        }
        let mut start = s.sel.focus.clone();
        start.off = 0;
        let mut end = s.sel.focus.clone();
        end.off = para.len();
        Some(super::AiTarget { text, selection: false, anchor: serde_json::json!({"start": start, "end": end}), screen })
    }

    fn ai_replace(&mut self, target: &super::AiTarget, original: &str, replacement: &str) -> Result<(), String> {
        let start: wordcraft_doc::Pos = serde_json::from_value(target.anchor["start"].clone()).map_err(|e| e.to_string())?;
        let saved_find = self.app.session.find.clone();
        self.app.session.sel = wordcraft_engine::Selection { anchor: start.clone(), focus: start.clone() };
        let found = self.app.run("edit.find", serde_json::json!({"text": original, "matchCase": true, "special": false}));
        let (a, _) = self.app.session.sel.ordered();
        let ok = found.is_ok() && a >= start && self.app.session.selected_text() == original;
        self.app.session.find = saved_find;
        if !ok {
            self.app.session.sel = wordcraft_engine::Selection { anchor: start.clone(), focus: start };
            return Err("The text has changed since it was checked".into());
        }
        self.app.run("text.insert", serde_json::json!({"text": replacement})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn ai_revision(&mut self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.app.session.rev().hash(&mut h);
        self.app.session.path.hash(&mut h);
        self.app.session.doc.body.len().hash(&mut h);
        h.finish()
    }

    fn ai_units(&mut self) -> Vec<(serde_json::Value, String)> {
        let mut out = Vec::new();
        for (i, b) in self.app.session.doc.body.iter().enumerate().take(5000) {
            if let Some(p) = b.as_para() {
                let text = p.plain_text();
                if text.trim().chars().count() >= 3 {
                    out.push((serde_json::json!({"block": i}), text));
                }
            }
        }
        out
    }

    fn ai_marks(&mut self, key: &serde_json::Value, original: &str) -> Vec<egui::Rect> {
        use wordcraft_ui_egui::canvas::cursor::pos_screen;
        let Some(i) = key["block"].as_u64().map(|i| i as usize) else { return Vec::new() };
        let Some(raw) = self.app.session.doc.body.get(i).and_then(|b| b.as_para()).map(|p| p.text.clone()) else { return Vec::new() };
        let Some(b0) = raw.find(original) else { return Vec::new() };
        let b1 = b0 + original.len();
        let at = |app: &mut WordApp, off: usize| pos_screen(app, &wordcraft_doc::Pos::body(i, off));
        let (Some((p0, h0)), Some((p1, _))) = (at(&mut self.app, b0), at(&mut self.app, b1)) else { return Vec::new() };
        if (p0.y - p1.y).abs() < 1.0 && p1.x >= p0.x {
            return vec![egui::Rect::from_min_max(p0, egui::pos2(p1.x, p0.y + h0))];
        }
        // Across lines: one rectangle per line.
        let mut rects: Vec<egui::Rect> = Vec::new();
        let mut line: Option<(egui::Pos2, f32, f32)> = None;
        let bounds: Vec<usize> = raw[b0..b1].char_indices().map(|(k, _)| b0 + k).chain(std::iter::once(b1)).take(400).collect();
        for off in bounds {
            let Some((p, h)) = at(&mut self.app, off) else { continue };
            match line {
                Some((start, height, x)) if (start.y - p.y).abs() < 1.0 => line = Some((start, height, x.max(p.x))),
                Some((start, height, x)) => {
                    rects.push(egui::Rect::from_min_max(start, egui::pos2(x, start.y + height)));
                    line = Some((p, h, p.x));
                }
                None => line = Some((p, h, p.x)),
            }
        }
        if let Some((start, height, x)) = line {
            rects.push(egui::Rect::from_min_max(start, egui::pos2(x, start.y + height)));
        }
        rects.retain(|r| r.width() > 1.0);
        rects
    }

    fn ai_viewport(&mut self) -> Option<egui::Rect> {
        self.app.canvas.canvas_rect
    }

    fn ai_apply(&mut self, key: &serde_json::Value, original: &str, replacement: &str) -> Result<(), String> {
        let i = key["block"].as_u64().ok_or("no paragraph")? as usize;
        let target = super::AiTarget { text: original.into(), selection: false, anchor: serde_json::json!({"start": wordcraft_doc::Pos::body(i, 0)}), screen: None };
        self.ai_replace(&target, original, replacement)
    }

    fn ai_focus(&mut self, key: &serde_json::Value, original: &str) {
        let Some(i) = key["block"].as_u64().map(|i| i as usize) else { return };
        let Some(raw) = self.app.session.doc.body.get(i).and_then(|b| b.as_para()).map(|p| p.text.clone()) else { return };
        let (a, b) = match raw.find(original) {
            Some(b0) => (b0, b0 + original.len()),
            None => (0, 0),
        };
        let _ = self.app.run(
            "select.range",
            serde_json::json!({"anchor": wordcraft_doc::Pos::body(i, a), "focus": wordcraft_doc::Pos::body(i, b)}),
        );
        self.app.canvas.want_focus = true;
    }

    fn ai_insert(&mut self, text: &str) -> Result<(), String> {
        let (_, end) = self.app.session.sel.ordered();
        self.app.session.sel = wordcraft_engine::Selection { anchor: end.clone(), focus: end };
        self.app.run("text.insert", serde_json::json!({"text": text})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn ai_replace_all(&mut self, target: &super::AiTarget, text: &str) -> Result<(), String> {
        let start: wordcraft_doc::Pos = serde_json::from_value(target.anchor["start"].clone()).map_err(|e| e.to_string())?;
        let end: wordcraft_doc::Pos = serde_json::from_value(target.anchor["end"].clone()).map_err(|e| e.to_string())?;
        let s = &mut self.app.session;
        s.sel = wordcraft_engine::Selection { anchor: s.doc.clamp(&start), focus: s.doc.clamp(&end) };
        if s.selected_text() != target.text {
            return Err("The text has changed since it was read".into());
        }
        self.app.run("text.insert", serde_json::json!({"text": text})).map(|_| ()).map_err(|e| e.to_string())
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
