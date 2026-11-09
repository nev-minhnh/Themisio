//! Settings ▸ AI: providers and their keys (Windows Credential Manager), Claude Code, the models
//! each feature uses, effort and thinking, the writing assistant per app, auto-check,
//! translation and writing skills.

use std::collections::HashMap;

use egui::{Color32, RichText};

use super::{AiSettings, CcStatus, Job, PRESETS, ProviderCfg, Request, Wire, keys};
use crate::i18n::{Lang, t, tf};

#[derive(Default)]
pub struct AiSettingsUi {
    key_edit: HashMap<String, String>,
    has_key: HashMap<String, bool>,
    model_edit: HashMap<String, String>,
    fetched: HashMap<String, Vec<String>>,
    models_job: Option<(String, Job<Vec<String>>)>,
    test_job: Option<(String, Job<String>)>,
    results: HashMap<String, (bool, String)>,
    cc: Option<CcStatus>,
    cc_job: Option<Job<CcStatus>>,
    skills: Option<Vec<super::skills::Skill>>,
}

fn green() -> Color32 {
    Color32::from_rgb(0x2B, 0xB8, 0x6A)
}
fn red() -> Color32 {
    Color32::from_rgb(0xF0, 0x3C, 0x45)
}

impl AiSettingsUi {
    /// Forget cached state (the dialog was opened again).
    pub fn refresh(&mut self) {
        self.has_key.clear();
        self.skills = None;
        self.cc = None;
    }

    fn key_saved(&mut self, id: &str) -> bool {
        *self.has_key.entry(id.to_string()).or_insert_with(|| keys::has(id))
    }

    /// The page. Returns whether a setting changed (to save).
    pub fn show(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang) -> bool {
        let mut changed = false;
        let ctx = ui.ctx().clone();
        self.poll(&ctx, s, &mut changed);
        egui::ScrollArea::vertical().max_height(560.0).min_scrolled_height(540.0).auto_shrink([false, true]).show(ui, |ui| {
            // ---------------- Providers ----------------
            ui.label(RichText::new(t(lang, "Providers")).heading());
            ui.label(
                RichText::new(t(lang, "Keys are stored in Windows Credential Manager. Requests go directly from your PC to the provider. Keys you saved in Callio are used too."))
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);
            let mut ids: Vec<(String, String, Wire, &'static str, &'static str)> =
                PRESETS.iter().map(|p| (p.id.to_string(), p.name.to_string(), p.wire, p.base_url, p.key_url)).collect();
            for (id, c) in &s.providers {
                if id.starts_with("custom") {
                    ids.push((id.clone(), if c.name.is_empty() { "Custom".into() } else { c.name.clone() }, Wire::OpenAi, "", ""));
                }
            }
            let mut remove: Option<String> = None;
            for (id, name, _wire, base, key_url) in ids {
                let cfg = s.cfg(&id);
                let ready = cfg.enabled && (self.key_saved(&id) || id == "llamacpp" || id.starts_with("custom")) && !cfg.models.is_empty();
                let title = RichText::new(format!("{}  {}", if ready { "●" } else { "○" }, name)).color(if ready { green() } else { ui.visuals().text_color() });
                egui::CollapsingHeader::new(title).id_salt(("themisio-ai-provider", &id)).show(ui, |ui| {
                    changed |= self.provider(ui, s, &id, base, key_url, lang);
                    if id.starts_with("custom") && ui.button(t(lang, "Remove this provider")).clicked() {
                        remove = Some(id.clone());
                    }
                });
            }
            if let Some(id) = remove {
                s.providers.remove(&id);
                let _ = keys::set(&id, "");
                changed = true;
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(t(lang, "Add an OpenAI-compatible provider:"));
                for (name, url) in super::CUSTOM_TEMPLATES.iter().copied().chain([("Other", "")]) {
                    if ui.small_button(if name == "Other" { t(lang, "Other…") } else { name }).clicked() {
                        let n = (1..).find(|i| !s.providers.contains_key(&format!("custom-{i}"))).unwrap_or(1);
                        s.providers.insert(
                            format!("custom-{n}"),
                            ProviderCfg { enabled: true, base_url: url.into(), models: Vec::new(), name: if name == "Other" { format!("Custom {n}") } else { name.into() } },
                        );
                        changed = true;
                    }
                }
            });
            ui.separator();

            // ---------------- Claude Code ----------------
            ui.label(RichText::new("Claude Code").heading());
            changed |= ui.checkbox(&mut s.claude_code, t(lang, "Use Claude Code (works with a Claude Pro or Max plan, no API key)")).changed();
            if self.cc.is_none() && self.cc_job.is_none() {
                self.cc_job = Some(Job::spawn(&ctx, |_| Ok(super::claude_code_status())));
            }
            ui.horizontal_wrapped(|ui| {
                match &self.cc {
                    None => {
                        ui.spinner();
                        ui.label(t(lang, "Looking for Claude Code…"));
                    }
                    Some(st) if !st.installed => {
                        ui.colored_label(red(), t(lang, "Claude Code isn't installed on this PC."));
                        ui.hyperlink_to(t(lang, "How to install"), "https://docs.claude.com/en/docs/claude-code/setup");
                    }
                    Some(st) => {
                        let v = st.version.clone().unwrap_or_else(|| "Claude Code".into());
                        if st.signed_in {
                            let who = st.account.clone().map(|a| format!(" ({a})")).unwrap_or_default();
                            ui.colored_label(green(), format!("{v} — {}{who}", t(lang, "signed in with your Claude plan")));
                            if ui.small_button(t(lang, "Sign out")).clicked() {
                                let _ = super::claude_code_sign_out();
                                self.cc = None;
                            }
                        } else {
                            ui.label(format!("{v} — {}", t(lang, "not signed in")));
                            if ui.small_button(t(lang, "Sign in")).clicked() {
                                if let Err(e) = super::claude_code_sign_in() {
                                    self.results.insert("claudecode".into(), (false, e));
                                }
                            }
                        }
                        if ui.small_button(t(lang, "Refresh")).clicked() {
                            self.cc = None;
                        }
                    }
                }
            });
            ui.label(
                RichText::new(t(
                    lang,
                    "Themisio runs Anthropic's official Claude Code with your own sign-in and never sees your password or tokens. It only reads the text you send and changes nothing on your PC. Usage counts toward your plan's limits.",
                ))
                .small()
                .weak(),
            );
            ui.horizontal(|ui| {
                ui.label(t(lang, "Other Claude Code model id:"));
                let e = self.model_edit.entry("claudecode".into()).or_default();
                ui.add(egui::TextEdit::singleline(e).hint_text("claude-…").desired_width(160.0));
                if ui.small_button(t(lang, "Add")).clicked() && !e.trim().is_empty() {
                    let mut c = s.cfg("claudecode");
                    if !c.models.contains(&e.trim().to_string()) {
                        c.models.push(e.trim().to_string());
                    }
                    s.providers.insert("claudecode".into(), c);
                    e.clear();
                    changed = true;
                }
            });
            ui.separator();

            // ---------------- Models ----------------
            ui.label(RichText::new(t(lang, "Models")).heading());
            egui::Grid::new("themisio-ai-models").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(t(lang, "Writing assistant"));
                changed |= model_combo(ui, "ai-write", s, &mut s.write_model.clone(), lang, false).map(|v| s.write_model = v).is_some();
                ui.end_row();
                ui.label(t(lang, "CalcSheet assistant"));
                changed |= model_combo(ui, "ai-sheet", s, &mut s.sheet_model.clone(), lang, true).map(|v| s.sheet_model = v).is_some();
                ui.end_row();
                ui.label(t(lang, "Effort"));
                let label = super::EFFORTS.iter().find(|(v, _)| *v == s.effort).map_or("Model default", |(_, l)| l);
                egui::ComboBox::from_id_salt("themisio-ai-effort").selected_text(t(lang, label)).show_ui(ui, |ui| {
                    for (v, l) in super::EFFORTS {
                        changed |= ui.selectable_value(&mut s.effort, v.to_string(), t(lang, l)).changed();
                    }
                });
                ui.end_row();
                ui.label(t(lang, "Thinking"));
                changed |= ui.checkbox(&mut s.thinking, t(lang, "Let the model think before answering (slower, more thorough)")).changed();
                ui.end_row();
            });
            ui.separator();

            // ---------------- Writing assistant ----------------
            ui.label(RichText::new(t(lang, "Writing assistant")).heading());
            ui.horizontal(|ui| {
                ui.label(t(lang, "On in:"));
                changed |= ui.checkbox(&mut s.apps.documents, "Documents").changed();
                changed |= ui.checkbox(&mut s.apps.presentation, "Presentation").changed();
                changed |= ui.checkbox(&mut s.apps.calcsheet, "CalcSheet").changed();
            });
            changed |= ui.checkbox(&mut s.check_style, t(lang, "Suggest style, clarity and word choice as well as spelling and grammar")).changed();
            changed |= ui.checkbox(&mut s.auto_check, t(lang, "Check automatically after a pause in typing (each check is a request to the provider)")).changed();
            if s.auto_check {
                ui.horizontal(|ui| {
                    ui.label(t(lang, "Pause before checking"));
                    changed |= ui.add(egui::Slider::new(&mut s.auto_delay_ms, 800..=6000).suffix(" ms")).changed();
                });
            }
            ui.label(t(lang, "Your instructions for every writing request"));
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut s.instructions)
                        .hint_text(t(lang, "e.g. Use British spelling. Keep an academic tone. Don't change citations."))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            ui.label(RichText::new(t(lang, "Shortcuts: Ctrl+Alt+G check writing, Ctrl+Alt+K ask AI, Ctrl+Alt+T translate — or right-click the text.")).small().weak());
            ui.separator();

            // ---------------- Translation ----------------
            ui.label(RichText::new(t(lang, "Translation")).heading());
            ui.horizontal(|ui| {
                ui.label(t(lang, "Translate with"));
                let label = if s.translate_engine == "google" { "Google Translate".to_string() } else { s.label(&s.translate_engine) };
                let mut v = s.translate_engine.clone();
                egui::ComboBox::from_id_salt("themisio-ai-translate").selected_text(label).width(220.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut v, "google".to_string(), "Google Translate");
                    for (group, models) in s.choices() {
                        ui.label(RichText::new(group).small().weak());
                        for (val, l) in models {
                            ui.selectable_value(&mut v, val, l);
                        }
                    }
                });
                if v != s.translate_engine {
                    s.translate_engine = v;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(t(lang, "Into"));
                let name = super::assist::LANGUAGES.iter().find(|(c, _)| *c == s.translate_to).map_or("English", |(_, n)| n);
                egui::ComboBox::from_id_salt("themisio-ai-translate-to").selected_text(t(lang, name)).show_ui(ui, |ui| {
                    for (code, n) in super::assist::LANGUAGES {
                        changed |= ui.selectable_value(&mut s.translate_to, code.to_string(), t(lang, n)).changed();
                    }
                });
            });
            ui.separator();

            // ---------------- Skills ----------------
            ui.label(RichText::new(t(lang, "Skills")).heading());
            ui.label(RichText::new(t(lang, "Writing skills are folders with a SKILL.md (as Claude Code uses). Active skills are added to every writing request.")).small().weak());
            let list = self.skills.get_or_insert_with(|| super::skills::list(&s.skill_dirs)).clone();
            if list.is_empty() {
                ui.label(RichText::new(t(lang, "No skills found yet.")).small());
            }
            for sk in &list {
                let mut on = s.active_skills.contains(&sk.name);
                if ui.checkbox(&mut on, &sk.name).on_hover_text(&sk.description).changed() {
                    if on {
                        s.active_skills.push(sk.name.clone());
                    } else {
                        s.active_skills.retain(|n| *n != sk.name);
                    }
                    changed = true;
                }
            }
            ui.horizontal(|ui| {
                if ui.button(t(lang, "Open Themisio's skills folder")).clicked()
                    && let Some(d) = super::skills::own_dir()
                {
                    let _ = std::fs::create_dir_all(&d);
                    crate::shell::open_folder(&d);
                }
                if ui.button(t(lang, "Add a skills folder…")).clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    let d = dir.to_string_lossy().into_owned();
                    if !s.skill_dirs.contains(&d) {
                        s.skill_dirs.push(d);
                        changed = true;
                    }
                    self.skills = None;
                }
                if ui.button(t(lang, "Refresh")).clicked() {
                    self.skills = None;
                }
            });
            let mut drop = None;
            for (i, d) in s.skill_dirs.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(d).small());
                    if ui.small_button(t(lang, "Remove")).clicked() {
                        drop = Some(i);
                    }
                });
            }
            if let Some(i) = drop {
                s.skill_dirs.remove(i);
                self.skills = None;
                changed = true;
            }
        });
        changed
    }

    fn provider(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, id: &str, base: &str, key_url: &str, lang: Lang) -> bool {
        let ctx = ui.ctx().clone();
        let mut changed = false;
        let mut cfg = s.cfg(id);
        changed |= ui.checkbox(&mut cfg.enabled, t(lang, "Use this provider")).changed();
        if id.starts_with("custom") {
            ui.horizontal(|ui| {
                ui.label(t(lang, "Name"));
                changed |= ui.add(egui::TextEdit::singleline(&mut cfg.name).desired_width(200.0)).changed();
            });
        }
        // Key.
        ui.horizontal(|ui| {
            ui.label(t(lang, "API key"));
            if self.key_saved(id) && !self.key_edit.contains_key(id) {
                ui.colored_label(green(), t(lang, "Saved"));
                if ui.small_button(t(lang, "Change")).clicked() {
                    self.key_edit.insert(id.into(), String::new());
                }
                if ui.small_button(t(lang, "Remove")).clicked() {
                    let _ = keys::set(id, "");
                    self.has_key.insert(id.into(), false);
                }
            } else {
                let e = self.key_edit.entry(id.into()).or_default();
                ui.add(egui::TextEdit::singleline(e).password(true).hint_text(if id == "llamacpp" || id.starts_with("custom") { t(lang, "optional") } else { "" }).desired_width(220.0));
                if ui.small_button(t(lang, "Save")).clicked() {
                    match keys::set(id, e) {
                        Ok(()) => {
                            let saved = !e.trim().is_empty();
                            self.has_key.insert(id.into(), saved);
                            self.key_edit.remove(id);
                            if saved && !cfg.enabled {
                                cfg.enabled = true;
                                changed = true;
                            }
                        }
                        Err(err) => {
                            self.results.insert(id.into(), (false, err));
                        }
                    }
                }
            }
            if !key_url.is_empty() {
                ui.hyperlink_to(t(lang, "Get a key"), key_url);
            }
        });
        // Address.
        ui.horizontal(|ui| {
            ui.label(t(lang, "Address"));
            changed |= ui.add(egui::TextEdit::singleline(&mut cfg.base_url).hint_text(base).desired_width(300.0)).changed();
        });
        // Models.
        ui.label(t(lang, "Models"));
        ui.horizontal_wrapped(|ui| {
            let mut drop = None;
            for (i, m) in cfg.models.iter().enumerate() {
                if ui.small_button(format!("{m}  ×")).on_hover_text(t(lang, "Remove")).clicked() {
                    drop = Some(i);
                }
            }
            if let Some(i) = drop {
                cfg.models.remove(i);
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            let e = self.model_edit.entry(id.into()).or_default();
            ui.add(egui::TextEdit::singleline(e).hint_text(t(lang, "model id")).desired_width(180.0));
            if ui.small_button(t(lang, "Add")).clicked() && !e.trim().is_empty() {
                if !cfg.models.contains(&e.trim().to_string()) {
                    cfg.models.push(e.trim().to_string());
                    changed = true;
                }
                e.clear();
            }
            let loading = self.models_job.as_ref().is_some_and(|(p, _)| p == id);
            if ui.add_enabled(!loading, egui::Button::new(t(lang, "Load the provider's models")).small()).clicked() {
                let mut probe = s.clone();
                probe.providers.insert(id.into(), cfg.clone());
                if let Some(ep) = probe.endpoint(id) {
                    self.models_job = Some((id.into(), Job::spawn(&ctx, move |_| super::list_models(&ep))));
                }
            }
            if loading {
                ui.spinner();
            }
            let testing = self.test_job.as_ref().is_some_and(|(p, _)| p == id);
            if ui.add_enabled(!testing && !cfg.models.is_empty(), egui::Button::new(t(lang, "Test")).small()).clicked() {
                let mut probe = s.clone();
                cfg.enabled = true;
                probe.providers.insert(id.into(), cfg.clone());
                let req = Request {
                    model: format!("{id}|{}", cfg.models[0]),
                    system: String::new(),
                    messages: vec![("user".into(), "Reply with the single word OK.".into())],
                    max_tokens: 0,
                    ..Default::default()
                };
                self.test_job = Some((id.into(), Job::spawn(&ctx, move |c| super::complete(&probe, &req, c))));
            }
            if testing {
                ui.spinner();
            }
        });
        if let Some(list) = self.fetched.get(id).cloned() {
            egui::ScrollArea::vertical().id_salt(("fetched", id)).max_height(160.0).show(ui, |ui| {
                for m in list {
                    let mut on = cfg.models.contains(&m);
                    if ui.checkbox(&mut on, &m).changed() {
                        if on {
                            cfg.models.push(m.clone());
                        } else {
                            cfg.models.retain(|x| *x != m);
                        }
                        changed = true;
                    }
                }
            });
        }
        if let Some((ok, text)) = self.results.get(id) {
            ui.colored_label(if *ok { green() } else { red() }, text);
        }
        if changed || s.providers.get(id) != Some(&cfg) {
            s.providers.insert(id.into(), cfg);
            changed = true;
        }
        changed
    }

    fn poll(&mut self, _ctx: &egui::Context, _s: &mut AiSettings, _changed: &mut bool) {
        if let Some((id, job)) = &self.models_job
            && let Some(r) = job.take()
        {
            let id = id.clone();
            match r {
                Ok(list) if list.is_empty() => {
                    self.results.insert(id.clone(), (false, "The provider listed no models — add a model id by hand.".into()));
                }
                Ok(list) => {
                    self.fetched.insert(id.clone(), list);
                    self.results.remove(&id);
                }
                Err(e) => {
                    self.results.insert(id.clone(), (false, e));
                }
            }
            self.models_job = None;
        }
        if let Some((id, job)) = &self.test_job
            && let Some(r) = job.take()
        {
            let id = id.clone();
            self.results.insert(id, match r {
                Ok(text) => (true, format!("✓ {}", text.chars().take(80).collect::<String>())),
                Err(e) => (false, e),
            });
            self.test_job = None;
        }
        if let Some(job) = &self.cc_job
            && let Some(r) = job.take()
        {
            self.cc = r.ok();
            self.cc_job = None;
        }
    }
}

/// A model picker; `Some(new value)` when it changed. `same_as` offers "Same as writing".
fn model_combo(ui: &mut egui::Ui, id: &str, s: &AiSettings, value: &mut String, lang: Lang, same_as: bool) -> Option<String> {
    let before = value.clone();
    let label = if value.is_empty() {
        if same_as { t(lang, "Same as the writing assistant").to_string() } else { t(lang, "First available").to_string() }
    } else {
        s.label(value)
    };
    egui::ComboBox::from_id_salt(("themisio", id)).selected_text(label).width(240.0).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), if same_as { t(lang, "Same as the writing assistant") } else { t(lang, "First available") });
        for (group, models) in s.choices() {
            ui.label(RichText::new(group).small().weak());
            for (v, l) in models {
                ui.selectable_value(value, v, l);
            }
        }
    });
    if s.choices().is_empty() {
        ui.label(RichText::new(tf(lang, "Turn on a provider above, or {cc}.", &[("cc", "Claude Code")])).small().weak());
    }
    (*value != before).then(|| value.clone())
}
