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
    Check,
    Ask,
    Translate,
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
        let mut items = vec![(Mode::Translate, t(lang, "Translate"), "Ctrl+Alt+T")];
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
    Applied,
    Dismissed,
    Failed,
}

enum Outcome {
    Suggestions(Vec<Suggestion>),
    Text(String),
    Translation(String, String),
}

/// What the shell does to the document for the popup.
pub enum Act {
    /// Apply suggestion `i` of the open target.
    Apply(usize),
    /// Replace the whole target with the text.
    Replace(String),
    /// Copy to the clipboard.
    Copy(String),
}

#[derive(Default)]
struct Auto {
    seen: u64,
    changed_at: f64,
    checked: u64,
    next_poll: f64,
    job: Option<Job<Vec<Suggestion>>>,
    job_for: Option<(AppKind, AiTarget)>,
    found: Option<(AppKind, AiTarget, Vec<Suggestion>)>,
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
    auto: Auto,
}

impl Default for Assist {
    fn default() -> Self {
        Self {
            open: false,
            kind: None,
            target: None,
            pos: pos2(200.0, 200.0),
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
            auto: Auto::default(),
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

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
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
        self.kind = Some(kind);
        self.pos = target.as_ref().and_then(|t| t.screen).unwrap_or(pos) + vec2(8.0, 8.0);
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
        if mode != Mode::Ask {
            self.start(ctx, s, lang);
        }
    }

    /// Open on what auto-check found.
    fn open_found(&mut self, kind: AppKind, target: AiTarget, list: Vec<Suggestion>, pos: Pos2) {
        self.open = true;
        self.kind = Some(kind);
        self.pos = target.screen.unwrap_or(pos);
        self.target = Some(target);
        self.mode = Mode::Check;
        self.job = None;
        self.suggestions = list;
        self.answer = None;
        self.note = None;
        self.error = None;
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
                    self.error = Some(t(lang, "Sign in to an AI provider in Settings ▸ AI first.").into());
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
                    self.error = Some(t(lang, "Sign in to an AI provider in Settings ▸ AI first.").into());
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
                let req = Request { model: self.model.clone(), system, messages: vec![("user".into(), prompt)], effort: self.effort.clone(), thinking: self.thinking, max_tokens: 0 };
                self.job = Some(Job::spawn(ctx, move |c| super::complete(&settings, &req, c).map(Outcome::Text)));
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
                    };
                    self.job = Some(Job::spawn(ctx, move |c| super::complete(&settings, &req, c).map(|t| Outcome::Translation(t, String::new()))));
                }
            }
        }
    }

    /// Mark suggestion `i` after the shell tried it.
    pub fn applied(&mut self, i: usize, ok: bool) {
        if let Some(s) = self.suggestions.get_mut(i) {
            s.state = if ok { State::Applied } else { State::Failed };
        }
    }

    /// The popup (and auto-check's badge). Returns what to do to the document, and whether the
    /// model choices changed (to remember them).
    pub fn ui(&mut self, ctx: &egui::Context, app_rect: Rect, s: &AiSettings, lang: Lang, style: &Arc<egui::Style>) -> (Vec<Act>, bool) {
        let mut acts = Vec::new();
        let mut changed = false;
        self.badge(ctx, app_rect, lang, style);
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
                    if !from.is_empty() {
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
        egui::Area::new(egui::Id::unique("themisio-assist"))
            .order(egui::Order::Foreground)
            .default_pos(start)
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
                        for (m, label) in [(Mode::Check, t(lang, "Check")), (Mode::Ask, t(lang, "Ask")), (Mode::Translate, t(lang, "Translate"))] {
                            if ui.selectable_label(self.mode == m, label).clicked() && self.mode != m {
                                self.mode = m;
                                self.answer = None;
                                self.error = None;
                                self.note = None;
                                if m != Mode::Ask && !(m == Mode::Check && !self.suggestions.is_empty()) {
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
                            let label = if self.model.is_empty() { t(lang, "No model — Settings ▸ AI").to_string() } else { s.label(&self.model) };
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
                        Mode::Check => self.check_body(ui, lang, busy, &mut acts, &mut restart),
                        Mode::Ask => self.ask_body(ui, s, lang, busy, &mut acts, &mut restart),
                        Mode::Translate => self.result_body(ui, lang, busy, &mut acts, &mut restart),
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
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
                };
                ui.label(format!("{what} {secs} s"));
                if ui.small_button(t(lang, "Cancel")).clicked() {
                    self.job = None;
                }
            });
            return true;
        }
        if let Some(e) = self.error.clone() {
            ui.colored_label(Color32::from_rgb(0xF0, 0x3C, 0x45), e);
            if ui.button(t(lang, "Try again")).clicked() {
                *restart = true;
            }
            return true;
        }
        false
    }

    fn check_body(&mut self, ui: &mut egui::Ui, lang: Lang, busy: bool, acts: &mut Vec<Act>, restart: &mut bool) {
        if let Some(target) = &self.target {
            let what = if target.selection { t(lang, "Selection") } else { t(lang, "Paragraph") };
            ui.label(RichText::new(format!("{what}: {}", preview(&target.text, 90))).small().weak());
        }
        if self.status(ui, lang, busy, restart) {
            return;
        }
        if let Some(n) = &self.note {
            ui.horizontal(|ui| {
                tick(ui);
                ui.label(n);
            });
            return;
        }
        let open: Vec<usize> = self.suggestions.iter().enumerate().filter(|(_, s)| s.state == State::Open).map(|(i, _)| i).collect();
        ui.horizontal(|ui| {
            let label = if open.is_empty() && !self.suggestions.is_empty() { t(lang, "All done").to_string() } else { tf(lang, "{n} suggestions", &[("n", &open.len().to_string())]) };
            ui.label(RichText::new(label).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !open.is_empty() && ui.button(t(lang, "Accept all")).clicked() {
                    for i in &open {
                        acts.push(Act::Apply(*i));
                    }
                }
                if ui.small_button(t(lang, "Check again")).clicked() {
                    *restart = true;
                }
            });
        });
        egui::ScrollArea::vertical().max_height(380.0).min_scrolled_height(380.0).auto_shrink([false, true]).show(ui, |ui| {
            for (i, s) in self.suggestions.iter_mut().enumerate() {
                if s.state == State::Dismissed {
                    continue;
                }
                egui::Frame::NONE.fill(ui.visuals().faint_bg_color).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        chip(ui, &s.kind, lang);
                        if s.state == State::Open {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(t(lang, "Dismiss")).clicked() {
                                    s.state = State::Dismissed;
                                }
                                if ui.small_button(t(lang, "Accept")).clicked() {
                                    acts.push(Act::Apply(i));
                                }
                            });
                        }
                        match s.state {
                            State::Applied => {
                                ui.label(RichText::new(t(lang, "Applied")).small().color(Color32::from_rgb(0x2B, 0xB8, 0x6A)));
                            }
                            State::Failed => {
                                ui.label(RichText::new(t(lang, "Couldn't apply — the text changed")).small().color(Color32::from_rgb(0xF0, 0x3C, 0x45)));
                            }
                            _ => {}
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&s.original).strikethrough().color(Color32::from_rgb(0xE0, 0x55, 0x5E)));
                        ui.label("→");
                        ui.label(RichText::new(if s.replacement.is_empty() { "∅" } else { &s.replacement }).strong().color(Color32::from_rgb(0x2B, 0xB8, 0x6A)));
                    });
                    if !s.explanation.is_empty() {
                        ui.label(RichText::new(&s.explanation).small());
                    }
                });
                ui.add_space(4.0);
            }
        });
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
                if ui.small_button(t(lang, q)).clicked() {
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
                    ui.label(RichText::new(t(lang, "No skills yet. Add SKILL.md folders in Settings ▸ AI.")).small());
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
                if ui.add_enabled(!busy && !self.request.trim().is_empty(), egui::Button::new(t(lang, "Send"))).on_hover_text("Ctrl+Enter").clicked() || send_key {
                    *restart = true;
                }
            });
        });
        ui.separator();
        self.result_body(ui, lang, busy, acts, &mut false);
    }

    fn result_body(&mut self, ui: &mut egui::Ui, lang: Lang, busy: bool, acts: &mut Vec<Act>, restart: &mut bool) {
        if self.mode == Mode::Translate
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
            if self.target.is_some() && ui.button(t(lang, "Replace")).clicked() {
                acts.push(Act::Replace(answer.clone()));
            }
            if ui.button(t(lang, "Copy")).clicked() {
                acts.push(Act::Copy(answer.clone()));
            }
        });
    }

    // ---------------- auto-check ----------------

    /// After a pause in typing, check the paragraph being written (when on for this app).
    /// `target` is read by the shell at most a few times a second.
    pub fn auto_tick(&mut self, ctx: &egui::Context, kind: AppKind, target: Option<AiTarget>, s: &AiSettings, lang: Lang, now: f64) {
        if let Some(job) = &self.auto.job
            && let Some(r) = job.take()
        {
            self.auto.job = None;
            if let (Ok(list), Some((k, tg))) = (r, self.auto.job_for.take())
                && !list.is_empty()
            {
                self.auto.found = Some((k, tg, list));
            }
        }
        let Some(target) = target else {
            self.auto.found = None;
            return;
        };
        let h = hash(&target.text);
        if h != self.auto.seen {
            self.auto.seen = h;
            self.auto.changed_at = now;
            if self.auto.found.as_ref().is_some_and(|(_, t, _)| hash(&t.text) != h) {
                self.auto.found = None;
            }
            return;
        }
        let delay = f64::from(s.auto_delay_ms.max(800)) / 1000.0;
        if now - self.auto.changed_at < delay || self.auto.checked == h || self.auto.job.is_some() || target.text.trim().chars().count() < 12 {
            if now - self.auto.changed_at < delay {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(delay - (now - self.auto.changed_at) + 0.05));
            }
            return;
        }
        let Some(model) = s.model_for(&s.write_model) else { return };
        self.auto.checked = h;
        let settings = s.clone();
        let text = target.text.clone();
        let req = Request {
            model,
            system: check_system(s, lang),
            messages: vec![("user".into(), format!("Text:\n<<<\n{text}\n>>>"))],
            effort: if s.effort.is_empty() { "low".into() } else { s.effort.clone() },
            thinking: false,
            max_tokens: 0,
        };
        self.auto.job_for = Some((kind, target));
        self.auto.job = Some(Job::spawn(ctx, move |c| {
            let out = super::complete(&settings, &req, c)?;
            parse_suggestions(&out, &text)
        }));
    }

    pub fn auto_poll_due(&mut self, now: f64) -> bool {
        if now >= self.auto.next_poll {
            self.auto.next_poll = now + 0.4;
            return true;
        }
        false
    }

    pub fn forget_auto(&mut self) {
        self.auto = Auto::default();
    }

    /// Auto-check's result: a small badge at the bottom right of the app, opening the popup.
    fn badge(&mut self, ctx: &egui::Context, app_rect: Rect, lang: Lang, style: &Arc<egui::Style>) {
        let busy = self.auto.job.is_some();
        let count = self.auto.found.as_ref().map_or(0, |(_, _, l)| l.len());
        if (count == 0 && !busy) || self.open {
            return;
        }
        let size = vec2(if count > 0 { 150.0 } else { 36.0 }, 32.0);
        let at = pos2(app_rect.right() - size.x - 22.0, app_rect.bottom() - size.y - 34.0);
        let mut clicked = false;
        egui::Area::new(egui::Id::unique("themisio-assist-badge")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            ui.set_style(style.clone());
            let (r, resp) = ui.allocate_exact_size(size, Sense::click());
            let p = ui.painter();
            let fill = if resp.hovered() { crate::theme::BRAND } else { crate::theme::BRAND_DEEP };
            p.rect(r, CornerRadius::same(16), fill, Stroke::new(1.0, crate::theme::GOLD.gamma_multiply(0.6)), egui::StrokeKind::Inside);
            draw_sparkle(p, pos2(r.left() + 18.0, r.center().y), 7.0, crate::theme::GOLD);
            if count > 0 {
                p.text(
                    pos2(r.left() + 32.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    tf(lang, "{n} suggestions", &[("n", &count.to_string())]),
                    crate::theme::ui_font(ctx, 12.5),
                    crate::theme::CREAM,
                );
            } else {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
            clicked = resp.on_hover_text(t(lang, "Writing suggestions")).clicked();
        });
        if clicked
            && let Some((kind, target, list)) = self.auto.found.take()
        {
            self.open_found(kind, target, list, at - vec2(300.0, 300.0));
        }
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

fn preview(text: &str, n: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
}

fn chip(ui: &mut egui::Ui, kind: &str, lang: Lang) {
    let (label, color) = match kind {
        "spelling" => (t(lang, "Spelling"), Color32::from_rgb(0xF0, 0x3C, 0x45)),
        "grammar" => (t(lang, "Grammar"), Color32::from_rgb(0xF5, 0x8E, 0x1C)),
        "punctuation" => (t(lang, "Punctuation"), Color32::from_rgb(0xF5, 0x8E, 0x1C)),
        "syntax" => (t(lang, "Syntax"), Color32::from_rgb(0xF5, 0x8E, 0x1C)),
        "clarity" => (t(lang, "Clarity"), Color32::from_rgb(0x1A, 0x8C, 0xF8)),
        _ => (t(lang, "Style"), Color32::from_rgb(0x8B, 0x5C, 0xF6)),
    };
    egui::Frame::NONE.fill(color.gamma_multiply(0.18)).corner_radius(CornerRadius::same(5)).inner_margin(egui::Margin::symmetric(6, 1)).show(ui, |ui| {
        ui.label(RichText::new(label).small().color(color));
    });
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

fn tick(ui: &mut egui::Ui) {
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
