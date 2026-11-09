//! The writing assistant: a popup by the pointer (or the caret) that checks spelling, grammar,
//! punctuation, syntax and style, rewrites on request (with writing skills), and translates —
//! in Documents, Presentation and CalcSheet. Its header switches provider and model, effort and
//! thinking on the spot. Suggestions are applied as ordinary edits, so Ctrl+Z undoes them.
//!
//! It opens from the apps' right-click menus (Check writing, Ask AI, Translate), from
//! Ctrl+Alt+G / Ctrl+Alt+K / Ctrl+Alt+T, or from the badge auto-check shows after a pause in
//! typing (off by default: every check is a request to the provider).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{Color32, CornerRadius, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};

use super::{AiSettings, Job, Request, json_in};
use crate::app_kind::AppKind;
use crate::hosts::AiTarget;
use crate::i18n::{Lang, t, tf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Check the document (handled by the writing assistant's panel).
    Check,
    Ask,
    Translate,
    Define,
}

/// A request from a right-click menu, picked up on the next pass.
static REQUEST: Mutex<Option<Mode>> = Mutex::new(None);
static VIETNAMESE: AtomicBool = AtomicBool::new(false);
static WRITING_ON: AtomicBool = AtomicBool::new(false);

/// What the menu items say and offer this pass: the interface language, and whether the writing
/// assistant is on for the app shown (Translate is always offered).
pub fn set_menu_context(lang: Lang, writing_on: bool) {
    VIETNAMESE.store(lang == Lang::Vi, Ordering::Relaxed);
    WRITING_ON.store(writing_on, Ordering::Relaxed);
}

pub fn take_request() -> Option<Mode> {
    REQUEST.lock().ok().and_then(|mut r| r.take())
}

/// Add Check writing, Ask AI and Translate to the apps' right-click menus.
pub fn install_menu_hook() {
    egui::themisio::set_context_menu_hook(Some(Arc::new(|ui: &mut egui::Ui| {
        let lang = if VIETNAMESE.load(Ordering::Relaxed) { Lang::Vi } else { Lang::En };
        let mut items = vec![(Mode::Translate, t(lang, "Translate"), "Ctrl+Alt+T"), (Mode::Define, t(lang, "Look up meaning"), "Ctrl+Alt+D")];
        if WRITING_ON.load(Ordering::Relaxed) {
            items.insert(0, (Mode::Ask, t(lang, "Ask AI…"), "Ctrl+Alt+K"));
            items.insert(0, (Mode::Check, t(lang, "Check writing with AI"), "Ctrl+Alt+G"));
        }
        ui.separator();
        let mut clicked = false;
        for (mode, label, keys) in items {
            if ui.add(egui::Button::new(label).shortcut_text(keys).frame_when_inactive(false)).clicked() {
                if let Ok(mut r) = REQUEST.lock() {
                    *r = Some(mode);
                }
                clicked = true;
                ui.close();
            }
        }
        clicked
    })));
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub kind: String,
    pub original: String,
    pub replacement: String,
    pub explanation: String,
    pub state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Open,
}

enum Outcome {
    Suggestions(Vec<Suggestion>),
    Text(String),
    Translation(String, String),
}

/// What the shell does to the document for the popup.
pub enum Act {
    /// Replace the whole target with the text.
    Replace(String),
    /// Copy to the clipboard.
    Copy(String),
}

pub struct Assist {
    pub open: bool,
    pub kind: Option<AppKind>,
    pub target: Option<AiTarget>,
    pos: Pos2,
    mode: Mode,
    pub model: String,
    pub effort: String,
    pub thinking: bool,
    engine: String,
    to: String,
    job: Option<Job<Outcome>>,
    pub suggestions: Vec<Suggestion>,
    answer: Option<String>,
    note: Option<String>,
    error: Option<String>,
    request: String,
    skills: Vec<String>,
    skill_list: Option<Vec<super::skills::Skill>>,
    /// When the pointer moved away from the popup (it closes shortly after).
    away_since: Option<f64>,
    /// Where the pointer was when the popup opened, and whether it has been over the popup since:
    /// moving away only counts once the pointer has been over it or has clearly left the spot.
    opened_from: Option<Pos2>,
    entered: bool,
    /// Place the popup at `pos` on the next pass (it is movable afterwards).
    place: bool,
}

impl Default for Assist {
    fn default() -> Self {
        Self {
            open: false,
            kind: None,
            target: None,
            pos: pos2(200.0, 200.0),
            away_since: None,
            opened_from: None,
            entered: false,
            place: false,
            mode: Mode::Check,
            model: String::new(),
            effort: String::new(),
            thinking: false,
            engine: "google".into(),
            to: "auto".into(),
            job: None,
            suggestions: Vec::new(),
            answer: None,
            note: None,
            error: None,
            request: String::new(),
            skills: Vec::new(),
            skill_list: None,
        }
    }
}

pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Vietnamese ↔ English"),
    ("vi", "Vietnamese"),
    ("en", "English"),
    ("zh-CN", "Chinese (Simplified)"),
    ("zh-TW", "Chinese (Traditional)"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("fr", "French"),
    ("de", "German"),
    ("es", "Spanish"),
    ("pt", "Portuguese"),
    ("it", "Italian"),
    ("ru", "Russian"),
    ("th", "Thai"),
    ("id", "Indonesian"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
];

fn lang_name(code: &str) -> &'static str {
    LANGUAGES.iter().find(|(c, _)| *c == code).map_or("English", |(_, n)| n)
}

/// The system prompt for a writing check.
fn check_system(s: &AiSettings, lang: Lang) -> String {
    let scope = if s.check_style {
        "spelling, grammar, punctuation, syntax, word choice, clarity, concision and style"
    } else {
        "spelling, grammar, punctuation and syntax only (no style changes)"
    };
    let ui_lang = if lang == Lang::Vi { "Vietnamese" } else { "English" };
    let mut p = format!(
        "You are a meticulous copy editor. Check the user's text for {scope}. Keep the author's language (never translate), voice and meaning. \
         Reply with JSON only, no prose and no code fence, exactly in this shape:\n\
         {{\"suggestions\": [{{\"type\": \"spelling|grammar|punctuation|syntax|style|clarity\", \"original\": \"…\", \"replacement\": \"…\", \"explanation\": \"…\"}}]}}\n\
         Rules: \"original\" is copied verbatim from the text (same characters, spacing and case) and is just long enough to be unique — usually a word or a short phrase; \
         \"replacement\" is what replaces it; \"explanation\" is one short sentence in {ui_lang}. List at most 25 suggestions, most important first. \
         If the text needs no change, reply {{\"suggestions\": []}}."
    );
    if !s.instructions.trim().is_empty() {
        p.push_str(&format!("\n\nThe author's standing instructions:\n{}", s.instructions.trim()));
    }
    p.push_str(&super::skills::instructions(&s.active_skills, &s.skill_dirs));
    p
}

fn parse_suggestions(answer: &str, text: &str) -> Result<Vec<Suggestion>, String> {
    let v = json_in(answer).ok_or("The model didn't answer with suggestions. Try again or pick another model.")?;
    let list = v["suggestions"].as_array().or_else(|| v.as_array()).cloned().unwrap_or_default();
    let mut out: Vec<Suggestion> = Vec::new();
    for s in list {
        let original = s["original"].as_str().unwrap_or("").to_string();
        let replacement = s["replacement"].as_str().unwrap_or("").to_string();
        if original.is_empty() || original == replacement || !text.contains(&original) {
            continue;
        }
        if out.iter().any(|o| o.original == original) {
            continue;
        }
        out.push(Suggestion {
            kind: s["type"].as_str().unwrap_or("grammar").to_lowercase(),
            original,
            replacement,
            explanation: s["explanation"].as_str().unwrap_or("").to_string(),
            state: State::Open,
        });
    }
    Ok(out)
}

impl Assist {
    /// Open the popup on `target` in `mode`, at `pos`, and start right away (except Ask, which
    /// waits for the request).
    pub fn open_at(&mut self, ctx: &egui::Context, kind: AppKind, target: Option<AiTarget>, mode: Mode, pos: Pos2, s: &AiSettings, lang: Lang) {
        self.open = true;
        self.away_since = None;
        self.entered = false;
        self.place = true;
        self.opened_from = ctx.pointer_latest_pos();
        self.kind = Some(kind);
        // Right under the pointer (or the caret, from a shortcut), so it is where you look.
        self.pos = match target.as_ref().and_then(|t| t.screen) {
            Some(caret) => caret + vec2(8.0, 8.0),
            None => pos + vec2(-18.0, 14.0),
        };
        self.target = target;
        self.mode = mode;
        self.job = None;
        self.suggestions.clear();
        self.answer = None;
        self.note = None;
        self.error = None;
        if self.model.is_empty() || !s.choices().iter().any(|(_, m)| m.iter().any(|(v, _)| *v == self.model)) {
            self.model = s.model_for(&s.write_model).unwrap_or_default();
            self.effort = s.effort.clone();
            self.thinking = s.thinking;
        }
        self.engine = s.translate_engine.clone();
        self.to = s.translate_to.clone();
        if self.skills.is_empty() {
            self.skills = s.active_skills.clone();
        }
        if matches!(mode, Mode::Translate | Mode::Define) {
            self.start(ctx, s, lang);
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.job = None;
    }

    fn start(&mut self, ctx: &egui::Context, s: &AiSettings, lang: Lang) {
        self.error = None;
        self.answer = None;
        self.note = None;
        let text = self.target.as_ref().map(|t| t.text.clone()).unwrap_or_default();
        let settings = s.clone();
        match self.mode {
            Mode::Check => {
                if text.trim().is_empty() {
                    self.error = Some(t(lang, "Select some text, or put the cursor in a paragraph or a cell with text.").into());
                    return;
                }
                if self.model.is_empty() {
                    self.error = Some(t(lang, "Sign in to an AI provider in Settings › AI first.").into());
                    return;
                }
                self.suggestions.clear();
                let req = Request {
                    model: self.model.clone(),
                    system: check_system(s, lang),
                    messages: vec![("user".into(), format!("Text:\n<<<\n{text}\n>>>"))],
                    effort: self.effort.clone(),
                    thinking: self.thinking,
                    max_tokens: 0,
                    web: false,
                };
                self.job = Some(Job::spawn(ctx, move |c| {
                    let out = super::complete(&settings, &req, c)?;
                    parse_suggestions(&out, &text).map(Outcome::Suggestions)
                }));
            }
            Mode::Ask => {
                if self.request.trim().is_empty() {
                    return;
                }
                if self.model.is_empty() {
                    self.error = Some(t(lang, "Sign in to an AI provider in Settings › AI first.").into());
                    return;
                }
                let mut system = String::from(
                    "You are a skilled writing assistant inside a document editor. Do what the user asks with their text. \
                     Reply with the resulting text only — no preamble, no quotes, no explanation, no Markdown unless the text already uses it. \
                     Keep the text's language unless the user asks otherwise.",
                );
                if !s.instructions.trim().is_empty() {
                    system.push_str(&format!("\n\nThe author's standing instructions:\n{}", s.instructions.trim()));
                }
                system.push_str(&super::skills::instructions(&self.skills, &s.skill_dirs));
                let prompt = if text.trim().is_empty() {
                    format!("Request: {}\n\n(There is no text yet: write the text to insert.)", self.request.trim())
                } else {
                    format!("Request: {}\n\nText:\n<<<\n{text}\n>>>", self.request.trim())
                };
                let req = Request { model: self.model.clone(), system, messages: vec![("user".into(), prompt)], effort: self.effort.clone(), thinking: self.thinking, max_tokens: 0, web: false };
                self.job = Some(Job::spawn(ctx, move |c| super::complete(&settings, &req, c).map(Outcome::Text)));
            }
            Mode::Define => {
                let word = text.trim().to_string();
                if word.is_empty() {
                    self.error = Some(t(lang, "Select a word or a phrase.").into());
                    return;
                }
                let model = self.model.clone();
                let effort = self.effort.clone();
                let ui_lang = if lang == Lang::Vi { "Vietnamese" } else { "English" };
                let context = self.target.as_ref().map(|t| t.text.clone()).unwrap_or_default();
                self.job = Some(Job::spawn(ctx, move |c| {
                    let english = word.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '-' || ch == '\'');
                    if english && let Ok(entry) = super::define_english(&word) {
                        return Ok(Outcome::Translation(entry, "dictionaryapi.dev".into()));
                    }
                    if model.is_empty() {
                        return Err("No dictionary entry — sign in to an AI provider to look up any word.".into());
                    }
                    let req = Request {
                        model,
                        system: format!(
                            "You are a concise dictionary. Explain the meaning of the word or phrase the user gives, as used in its context. \
                             Answer in {ui_lang}: the meaning(s) with part of speech, pronunciation if useful, 2–5 synonyms, and one short example. Plain text."
                        ),
                        messages: vec![("user".into(), format!("Word or phrase: {word}\nContext: {context}"))],
                        effort,
                        thinking: false,
                        max_tokens: 0,
                        web: false,
                    };
                    super::complete(&settings, &req, c).map(|t| Outcome::Translation(t, String::new()))
                }));
            }
            Mode::Translate => {
                if text.trim().is_empty() {
                    self.error = Some(t(lang, "Select the text to translate.").into());
                    return;
                }
                let to = self.to.clone();
                if self.engine == "google" || self.engine.is_empty() {
                    self.job = Some(Job::spawn(ctx, move |_| {
                        let target = if to == "auto" { "vi" } else { to.as_str() };
                        let (out, from) = super::google_translate(&text, target)?;
                        if to == "auto" && from.starts_with("vi") {
                            let (out, from) = super::google_translate(&text, "en")?;
                            return Ok(Outcome::Translation(out, from));
                        }
                        Ok(Outcome::Translation(out, from))
                    }));
                } else {
                    let into = if to == "auto" {
                        "English if the text is in Vietnamese, otherwise Vietnamese".to_string()
                    } else {
                        lang_name(&to).to_string()
                    };
                    let mut system = format!(
                        "You are a professional translator. Translate the user's text into {into}. Keep the meaning, tone, formatting and line breaks. \
                         Reply with the translation only."
                    );
                    if !s.instructions.trim().is_empty() {
                        system.push_str(&format!("\n\nThe author's standing instructions:\n{}", s.instructions.trim()));
                    }
                    let req = Request {
                        model: self.engine.clone(),
                        system,
                        messages: vec![("user".into(), text)],
                        effort: self.effort.clone(),
                        thinking: self.thinking,
                        max_tokens: 0,
                        web: false,
                    };
                    self.job = Some(Job::spawn(ctx, move |c| super::complete(&settings, &req, c).map(|t| Outcome::Translation(t, String::new()))));
                }
            }
        }
    }

    /// The popup (and auto-check's badge). Returns what to do to the document, and whether the
    /// model choices changed (to remember them).
    pub fn ui(&mut self, ctx: &egui::Context, app_rect: Rect, s: &AiSettings, lang: Lang, style: &Arc<egui::Style>) -> (Vec<Act>, bool) {
        let mut acts = Vec::new();
        let mut changed = false;
        let _ = app_rect;
        if !self.open {
            return (acts, changed);
        }
        if let Some(job) = &self.job
            && let Some(r) = job.take()
        {
            self.job = None;
            match r {
                Ok(Outcome::Suggestions(list)) => {
                    if list.is_empty() {
                        self.note = Some(t(lang, "Looks good — no suggestions.").into());
                    }
                    self.suggestions = list;
                }
                Ok(Outcome::Text(text)) => self.answer = Some(text),
                Ok(Outcome::Translation(text, from)) => {
                    self.answer = Some(text);
                    if from == "dictionaryapi.dev" {
                        self.note = Some(t(lang, "Source: Free Dictionary (dictionaryapi.dev)").into());
                    } else if !from.is_empty() {
                        self.note = Some(tf(lang, "Detected language: {lang}", &[("lang", &from)]));
                    }
                }
                Err(e) => self.error = Some(e),
            }
        }
        let screen = ctx.content_rect();
        let width = 440.0;
        let start = pos2(self.pos.x.min(screen.right() - width - 12.0).max(screen.left() + 8.0), self.pos.y.min(screen.bottom() - 320.0).max(screen.top() + 8.0));
        let mut close = false;
        let mut restart = false;
        let busy = self.job.is_some();
        let mut area = egui::Area::new(egui::Id::unique("themisio-assist")).order(egui::Order::Foreground).default_pos(start);
        if std::mem::take(&mut self.place) {
            area = area.current_pos(start);
        }
        let area = area
            .movable(true)
            .constrain(true)
            .show(ctx, |ui| {
                ui.set_style(style.clone());
                egui::Frame::popup(style).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.set_width(width);
                    // Title, mode switch, close.
                    ui.horizontal(|ui| {
                        sparkle(ui, 16.0);
                        ui.label(RichText::new("Themisio AI").strong());
                        ui.add_space(6.0);
                        for (m, label) in [(Mode::Ask, t(lang, "Ask AI")), (Mode::Translate, t(lang, "Translate")), (Mode::Define, t(lang, "Meaning"))] {
                            if ui.selectable_label(self.mode == m, label).clicked() && self.mode != m {
                                self.mode = m;
                                self.answer = None;
                                self.error = None;
                                self.note = None;
                                if m != Mode::Ask {
                                    restart = true;
                                }
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if close_button(ui) {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    // Provider and model, effort, thinking.
                    ui.horizontal(|ui| {
                        if self.mode == Mode::Translate {
                            let label = if self.engine == "google" { "Google Translate".to_string() } else { s.label(&self.engine) };
                            egui::ComboBox::from_id_salt("themisio-assist-engine").selected_text(label).width(190.0).show_ui(ui, |ui| {
                                changed |= ui.selectable_value(&mut self.engine, "google".to_string(), "Google Translate").changed();
                                model_items(ui, s, &mut self.engine, &mut changed);
                            });
                            egui::ComboBox::from_id_salt("themisio-assist-to").selected_text(t(lang, lang_name(&self.to))).width(150.0).show_ui(ui, |ui| {
                                for (code, name) in LANGUAGES {
                                    changed |= ui.selectable_value(&mut self.to, code.to_string(), t(lang, name)).changed();
                                }
                            });
                        } else {
                            let label = if self.model.is_empty() { t(lang, "No model — Settings › AI").to_string() } else { s.label(&self.model) };
                            egui::ComboBox::from_id_salt("themisio-assist-model").selected_text(label).width(200.0).show_ui(ui, |ui| {
                                model_items(ui, s, &mut self.model, &mut changed);
                            });
                        }
                        if self.mode != Mode::Translate || self.engine != "google" {
                            let effort_label = super::EFFORTS.iter().find(|(v, _)| *v == self.effort).map_or("Model default", |(_, l)| l);
                            egui::ComboBox::from_id_salt("themisio-assist-effort").selected_text(t(lang, effort_label)).width(90.0).show_ui(ui, |ui| {
                                for (v, l) in super::EFFORTS {
                                    changed |= ui.selectable_value(&mut self.effort, v.to_string(), t(lang, l)).changed();
                                }
                            });
                            changed |= ui.checkbox(&mut self.thinking, t(lang, "Thinking")).changed();
                        }
                    });
                    ui.separator();
                    match self.mode {
                        Mode::Check | Mode::Ask => self.ask_body(ui, s, lang, busy, &mut acts, &mut restart),
                        Mode::Translate | Mode::Define => self.result_body(ui, lang, busy, &mut acts, &mut restart),
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        // Esc closes it; so does moving the pointer away (not while typing a request, or while one
        // of its menus is open).
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        let rect = area.response.rect.expand(24.0);
        let typing = self.mode == Mode::Ask && (!self.request.trim().is_empty() || ctx.memory(|m| m.focused().is_some()));
        let menu_open = egui::Popup::is_any_open(ctx);
        let dragging = area.response.dragged() || ctx.input(|i| i.pointer.any_down());
        if ctx.pointer_latest_pos().is_some_and(|p| area.response.rect.contains(p)) {
            self.entered = true;
        }
        let left_spot = |p: Pos2| self.entered || self.opened_from.is_none_or(|o| o.distance(p) > 140.0);
        match ctx.pointer_latest_pos() {
            Some(p) if !rect.contains(p) && left_spot(p) && !typing && !menu_open && !dragging => {
                let now = ctx.input(|i| i.time);
                let since = *self.away_since.get_or_insert(now);
                if now - since > 0.6 {
                    close = true;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(60));
                }
            }
            _ => self.away_since = None,
        }
        if restart {
            self.start(ctx, s, lang);
        }
        if close {
            self.close();
        }
        if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        (acts, changed)
    }

    fn status(&mut self, ui: &mut egui::Ui, lang: Lang, busy: bool, restart: &mut bool) -> bool {
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                let secs = self.job.as_ref().map_or(0, |j| j.started.elapsed().as_secs());
                let what = match self.mode {
                    Mode::Check => t(lang, "Checking…"),
                    Mode::Ask => t(lang, "Writing…"),
                    Mode::Translate => t(lang, "Translating…"),
                    Mode::Define => t(lang, "Looking up…"),
                };
                ui.label(format!("{what} {secs} s"));
                if super::kit::secondary(ui, t(lang, "Cancel")).clicked() {
                    self.job = None;
                }
            });
            return true;
        }
        if let Some(e) = self.error.clone() {
            ui.colored_label(Color32::from_rgb(0xF0, 0x3C, 0x45), e);
            if super::kit::secondary(ui, t(lang, "Try again")).clicked() {
                *restart = true;
            }
            return true;
        }
        false
    }

    fn ask_body(&mut self, ui: &mut egui::Ui, s: &AiSettings, lang: Lang, busy: bool, acts: &mut Vec<Act>, restart: &mut bool) {
        if let Some(target) = &self.target {
            let what = if target.selection { t(lang, "Selection") } else { t(lang, "Paragraph") };
            ui.label(RichText::new(format!("{what}: {}", preview(&target.text, 90))).small().weak());
        }
        let edit = egui::TextEdit::multiline(&mut self.request)
            .hint_text(t(lang, "What should AI do? e.g. make it more formal, shorter, fix the tone, continue this paragraph…"))
            .desired_rows(2)
            .desired_width(f32::INFINITY);
        let resp = ui.add(edit);
        let send_key = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
        ui.horizontal(|ui| {
            // Quick requests.
            for q in ["Fix grammar", "More formal", "Shorter", "Clearer", "Friendlier"] {
                if super::kit::secondary(ui, t(lang, q)).clicked() {
                    self.request = t(lang, q).to_string();
                    *restart = true;
                }
            }
        });
        ui.horizontal(|ui| {
            let list = self.skill_list.get_or_insert_with(|| super::skills::list(&s.skill_dirs)).clone();
            let label = if self.skills.is_empty() { t(lang, "Skills").to_string() } else { format!("{} ({})", t(lang, "Skills"), self.skills.len()) };
            ui.menu_button(label, |ui| {
                if list.is_empty() {
                    ui.label(RichText::new(t(lang, "No skills yet. Add SKILL.md folders in Settings › AI.")).small());
                }
                for sk in &list {
                    let mut on = self.skills.contains(&sk.name);
                    if ui.checkbox(&mut on, &sk.name).on_hover_text(&sk.description).changed() {
                        if on {
                            self.skills.push(sk.name.clone());
                        } else {
                            self.skills.retain(|n| *n != sk.name);
                        }
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if super::kit::primary_enabled(ui, !busy && !self.request.trim().is_empty(), t(lang, "Send")).on_hover_text("Ctrl+Enter").clicked() || send_key {
                    *restart = true;
                }
            });
        });
        ui.separator();
        self.result_body(ui, lang, busy, acts, &mut false);
    }

    fn result_body(&mut self, ui: &mut egui::Ui, lang: Lang, busy: bool, acts: &mut Vec<Act>, restart: &mut bool) {
        if matches!(self.mode, Mode::Translate | Mode::Define)
            && let Some(target) = &self.target
        {
            ui.label(RichText::new(preview(&target.text, 120)).small().weak());
        }
        if self.status(ui, lang, busy, restart) {
            return;
        }
        let Some(answer) = self.answer.clone() else { return };
        egui::ScrollArea::vertical().max_height(300.0).min_scrolled_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
            egui::Frame::NONE.fill(ui.visuals().faint_bg_color).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(&answer).selectable(true));
            });
        });
        if let Some(n) = &self.note {
            ui.label(RichText::new(n).small().weak());
        }
        ui.horizontal(|ui| {
            if self.target.is_some() && self.mode != Mode::Define && super::kit::primary(ui, t(lang, "Replace")).clicked() {
                acts.push(Act::Replace(answer.clone()));
            }
            if super::kit::secondary(ui, t(lang, "Copy")).clicked() {
                acts.push(Act::Copy(answer.clone()));
            }
        });
    }
}

fn model_items(ui: &mut egui::Ui, s: &AiSettings, value: &mut String, changed: &mut bool) {
    for (group, models) in s.choices() {
        ui.label(RichText::new(group).small().weak());
        for (v, l) in models {
            *changed |= ui.selectable_value(value, v, l).changed();
        }
    }
}

pub(super) fn preview(text: &str, n: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
}

/// A close button drawn as a cross (no font glyph, so it shows over every app).
pub fn close_button(ui: &mut egui::Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), ui.visuals().widgets.hovered.bg_fill);
    }
    let c = rect.center();
    let st = Stroke::new(1.4, ui.visuals().text_color());
    ui.painter().line_segment([c - vec2(4.5, 4.5), c + vec2(4.5, 4.5)], st);
    ui.painter().line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], st);
    resp.clicked()
}

pub(super) fn tick(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    let c = rect.center();
    ui.painter().circle_filled(c, 8.0, Color32::from_rgb(0x2B, 0xB8, 0x6A));
    let st = Stroke::new(1.7, Color32::WHITE);
    ui.painter().line_segment([c + vec2(-3.8, 0.2), c + vec2(-1.0, 3.0)], st);
    ui.painter().line_segment([c + vec2(-1.0, 3.0), c + vec2(4.0, -2.8)], st);
}

fn sparkle(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    draw_sparkle(ui.painter(), rect.center(), size * 0.45, crate::theme::GOLD);
}

/// A four-pointed star, Themisio AI's mark.
pub fn draw_sparkle(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let w = r * 0.28;
    let pts = vec![
        pos2(c.x, c.y - r),
        pos2(c.x + w, c.y - w),
        pos2(c.x + r, c.y),
        pos2(c.x + w, c.y + w),
        pos2(c.x, c.y + r),
        pos2(c.x - w, c.y + w),
        pos2(c.x - r, c.y),
        pos2(c.x - w, c.y - w),
    ];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_must_quote_the_text() {
        let answer = r#"{"suggestions":[{"type":"spelling","original":"teh","replacement":"the","explanation":"Typo"},{"type":"grammar","original":"not there","replacement":"x"}]}"#;
        let list = parse_suggestions(answer, "I saw teh cat").unwrap_or_default();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].replacement, "the");
    }
}
