//! Settings › AI: providers and their keys (Windows Credential Manager), Claude Code, the models
//! each feature uses, effort and thinking, the writing assistant per app, check-as-you-type,
//! translation and writing skills — as pages of the Settings window, each a set of cards.

use std::collections::HashMap;

use egui::RichText;

use super::kit::{self, Icon};
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
    /// The provider card that is open.
    expanded: Option<String>,
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

    /// Settings › AI providers. Returns whether a setting changed (to save).
    pub fn providers_page(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang) -> bool {
        let mut changed = false;
        let ctx = ui.ctx().clone();
        self.poll();
        kit::page_title(
            ui,
            t(lang, "AI providers"),
            t(lang, "Sign in once and every app uses it. Keys are stored in Windows Credential Manager; requests go straight from your PC to the provider."),
        );

        // Claude Code first: it needs no key.
        kit::card(ui, |ui| {
            let p = kit::palette(ui);
            kit::setting_row(ui, "Claude Code", t(lang, "Use your Claude Pro or Max plan — no API key."), |ui| {
                changed |= kit::switch(ui, &mut s.claude_code).changed();
            });
            if self.cc.is_none() && self.cc_job.is_none() {
                self.cc_job = Some(Job::spawn(&ctx, |_| Ok(super::claude_code_status())));
            }
            kit::row_rule(ui);
            ui.horizontal_wrapped(|ui| match &self.cc {
                None => {
                    ui.spinner();
                    ui.label(RichText::new(t(lang, "Looking for Claude Code…")).color(p.muted));
                }
                Some(st) if !st.installed => {
                    kit::status(ui, kit::RED, t(lang, "Claude Code isn't installed on this PC."));
                    ui.hyperlink_to(t(lang, "How to install"), "https://docs.claude.com/en/docs/claude-code/setup");
                }
                Some(st) => {
                    let v = st.version.clone().unwrap_or_else(|| "Claude Code".into());
                    if st.signed_in {
                        let who = st.account.clone().map(|a| format!(" ({a})")).unwrap_or_default();
                        kit::status(ui, kit::GREEN, &format!("{v} — {}{who}", t(lang, "signed in with your Claude plan")));
                        if kit::secondary(ui, t(lang, "Sign out")).clicked() {
                            let _ = super::claude_code_sign_out();
                            self.cc = None;
                        }
                    } else {
                        kit::status(ui, kit::AMBER, &format!("{v} — {}", t(lang, "not signed in")));
                        if kit::primary(ui, t(lang, "Sign in")).clicked()
                            && let Err(e) = super::claude_code_sign_in()
                        {
                            self.results.insert("claudecode".into(), (false, e));
                        }
                    }
                    if kit::icon_button(ui, Icon::Retry, t(lang, "Refresh")).clicked() {
                        self.cc = None;
                    }
                }
            });
            if let Some((ok, text)) = self.results.get("claudecode") {
                ui.colored_label(if *ok { kit::GREEN } else { kit::RED }, text);
            }
            kit::hint(
                ui,
                t(
                    lang,
                    "Themisio runs Anthropic's official Claude Code with your own sign-in and never sees your password or tokens. It only reads the text you send and changes nothing on your PC. Usage counts toward your plan's limits.",
                ),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let e = self.model_edit.entry("claudecode".into()).or_default();
                ui.add(egui::TextEdit::singleline(e).hint_text(t(lang, "Other model id, e.g. claude-…")).desired_width(220.0));
                if kit::secondary(ui, t(lang, "Add")).clicked() && !e.trim().is_empty() {
                    let mut c = s.cfg("claudecode");
                    if !c.models.contains(&e.trim().to_string()) {
                        c.models.push(e.trim().to_string());
                    }
                    s.providers.insert("claudecode".into(), c);
                    e.clear();
                    changed = true;
                }
            });
        });
        ui.add_space(10.0);

        let mut ids: Vec<(String, String, Wire, &'static str, &'static str)> =
            PRESETS.iter().map(|p| (p.id.to_string(), p.name.to_string(), p.wire, p.base_url, p.key_url)).collect();
        for (id, c) in &s.providers {
            if id.starts_with("custom") {
                ids.push((id.clone(), if c.name.is_empty() { "Custom".into() } else { c.name.clone() }, Wire::OpenAi, "", ""));
            }
        }
        let mut remove: Option<String> = None;
        for (id, name, _wire, base, key_url) in ids {
            let mut cfg = s.cfg(&id);
            let keyless = id == "llamacpp" || id.starts_with("custom");
            let has_key = self.key_saved(&id);
            let (color, state) = if !cfg.enabled {
                (kit::palette(ui).muted, t(lang, "Off").to_string())
            } else if !has_key && !keyless {
                (kit::AMBER, t(lang, "Needs an API key").to_string())
            } else if cfg.models.is_empty() {
                (kit::AMBER, t(lang, "Add a model").to_string())
            } else {
                (kit::GREEN, tf(lang, "Ready · {n} models", &[("n", &cfg.models.len().to_string())]))
            };
            let open = self.expanded.as_deref() == Some(id.as_str());
            kit::card(ui, |ui| {
                let p = kit::palette(ui);
                let head = ui
                    .horizontal(|ui| {
                        ui.set_min_height(30.0);
                        kit::paint_icon(ui.painter(), if open { Icon::ChevronUp } else { Icon::Chevron }, ui.cursor().min + egui::vec2(6.0, 15.0), p.muted);
                        ui.add_space(18.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&name).font(crate::theme::bold_font(ui.ctx(), 13.5)).color(p.text));
                            kit::status(ui, color, &state);
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if kit::switch(ui, &mut cfg.enabled).changed() {
                                changed = true;
                            }
                        });
                    })
                    .response;
                if ui.interact(head.rect, ui.scope_id().with(("provider-head", &id)), egui::Sense::click()).clicked() {
                    self.expanded = if open { None } else { Some(id.clone()) };
                }
                if open {
                    kit::row_rule(ui);
                    changed |= self.provider(ui, s, &mut cfg, &id, base, key_url, lang);
                    if id.starts_with("custom") {
                        ui.add_space(6.0);
                        if kit::secondary(ui, t(lang, "Remove this provider")).clicked() {
                            remove = Some(id.clone());
                        }
                    }
                }
            });
            if s.providers.get(&id) != Some(&cfg) {
                s.providers.insert(id.clone(), cfg);
                changed = true;
            }
            ui.add_space(6.0);
        }
        if let Some(id) = remove {
            s.providers.remove(&id);
            let _ = keys::set(&id, "");
            changed = true;
        }
        ui.add_space(4.0);
        kit::section(ui, t(lang, "Add an OpenAI-compatible provider"));
        ui.horizontal_wrapped(|ui| {
            for (name, url) in super::CUSTOM_TEMPLATES.iter().copied().chain([("Other", "")]) {
                if kit::secondary(ui, if name == "Other" { t(lang, "Other…") } else { name }).clicked() {
                    let n = (1..).find(|i| !s.providers.contains_key(&format!("custom-{i}"))).unwrap_or(1);
                    let id = format!("custom-{n}");
                    s.providers.insert(id.clone(), ProviderCfg { enabled: true, base_url: url.into(), models: Vec::new(), name: if name == "Other" { format!("Custom {n}") } else { name.into() } });
                    self.expanded = Some(id);
                    changed = true;
                }
            }
        });
        changed
    }

    /// Settings › AI features: models, effort, thinking, the writing assistant.
    pub fn features_page(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang) -> bool {
        let mut changed = false;
        kit::page_title(ui, t(lang, "AI features"), t(lang, "Which model each feature uses, and how the writing assistant works."));
        kit::section(ui, t(lang, "Models"));
        kit::card(ui, |ui| {
            kit::setting_row(ui, t(lang, "Writing assistant"), t(lang, "Checks, rewrites, Ask AI."), |ui| {
                changed |= model_combo(ui, "ai-write", s, &mut s.write_model.clone(), lang, false).map(|v| s.write_model = v).is_some();
            });
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "CalcSheet assistant"), t(lang, "Data analysis, formulas and charts."), |ui| {
                changed |= model_combo(ui, "ai-sheet", s, &mut s.sheet_model.clone(), lang, true).map(|v| s.sheet_model = v).is_some();
            });
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "Effort"), t(lang, "More effort is slower but more thorough."), |ui| {
                let label = super::EFFORTS.iter().find(|(v, _)| *v == s.effort).map_or("Model default", |(_, l)| l);
                egui::ComboBox::from_id_salt("themisio-ai-effort").selected_text(t(lang, label)).width(200.0).show_ui(ui, |ui| {
                    for (v, l) in super::EFFORTS {
                        changed |= ui.selectable_value(&mut s.effort, v.to_string(), t(lang, l)).changed();
                    }
                });
            });
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "Thinking"), t(lang, "Let the model think before answering (slower, more thorough)."), |ui| {
                changed |= kit::switch(ui, &mut s.thinking).changed();
            });
            if s.choices().is_empty() {
                ui.add_space(4.0);
                kit::hint(ui, &tf(lang, "Turn on a provider in AI providers, or {cc}.", &[("cc", "Claude Code")]));
            }
        });
        ui.add_space(12.0);
        kit::section(ui, t(lang, "Writing assistant"));
        kit::card(ui, |ui| {
            for (label, on) in [("Documents", &mut s.apps.documents), ("Presentation", &mut s.apps.presentation), ("CalcSheet", &mut s.apps.calcsheet)] {
                kit::setting_row(ui, &tf(lang, "On in {app}", &[("app", label)]), "", |ui| {
                    changed |= kit::switch(ui, on).changed();
                });
                kit::row_rule(ui);
            }
            kit::setting_row(ui, t(lang, "Check as you type"), t(lang, "Underline suggestions while you write or paste (each check is a request to the provider)."), |ui| {
                changed |= kit::switch(ui, &mut s.auto_check).changed();
            });
            if s.auto_check {
                kit::row_rule(ui);
                kit::setting_row(ui, t(lang, "Pause before checking"), "", |ui| {
                    changed |= ui.add(egui::Slider::new(&mut s.auto_delay_ms, 800..=6000).suffix(" ms")).changed();
                });
            }
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "Clarity, engagement and delivery"), t(lang, "Suggest style, clarity and word choice as well as spelling and grammar."), |ui| {
                changed |= kit::switch(ui, &mut s.check_style).changed();
            });
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "Look up words on double-click"), t(lang, "Double-click a word to see what it means."), |ui| {
                changed |= kit::switch(ui, &mut s.define_on_double_click).changed();
            });
            kit::row_rule(ui);
            ui.label(RichText::new(t(lang, "Your instructions for every writing request")).size(13.0));
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut s.instructions)
                        .hint_text(t(lang, "e.g. Use British spelling. Keep an academic tone. Don't change citations."))
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed();
        });
        ui.add_space(8.0);
        kit::hint(ui, t(lang, "Shortcuts: Ctrl+Alt+G check writing, Ctrl+Alt+K ask AI, Ctrl+Alt+T translate, Ctrl+Alt+D look up a word, Ctrl+Alt+A CalcSheet assistant — or right-click the text."));
        changed
    }

    /// Settings › Translation.
    pub fn translation_page(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang) -> bool {
        let mut changed = false;
        kit::page_title(ui, t(lang, "Translation"), t(lang, "Right-click selected text › Translate, or press Ctrl+Alt+T."));
        kit::card(ui, |ui| {
            kit::setting_row(ui, t(lang, "Translate with"), t(lang, "Google Translate is free; an AI model keeps tone and formatting better."), |ui| {
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
            kit::row_rule(ui);
            kit::setting_row(ui, t(lang, "Into"), "", |ui| {
                let name = super::assist::LANGUAGES.iter().find(|(c, _)| *c == s.translate_to).map_or("English", |(_, n)| n);
                egui::ComboBox::from_id_salt("themisio-ai-translate-to").selected_text(t(lang, name)).width(220.0).show_ui(ui, |ui| {
                    for (code, n) in super::assist::LANGUAGES {
                        changed |= ui.selectable_value(&mut s.translate_to, code.to_string(), t(lang, n)).changed();
                    }
                });
            });
        });
        changed
    }

    /// Settings › Skills.
    pub fn skills_page(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang) -> bool {
        let mut changed = false;
        kit::page_title(ui, t(lang, "Skills"), t(lang, "Writing skills are folders with a SKILL.md (as Claude Code uses). Active skills are added to every writing request."));
        let list = self.skills.get_or_insert_with(|| super::skills::list(&s.skill_dirs)).clone();
        kit::card(ui, |ui| {
            if list.is_empty() {
                kit::hint(ui, t(lang, "No skills found yet."));
            }
            for (i, sk) in list.iter().enumerate() {
                if i > 0 {
                    kit::row_rule(ui);
                }
                let mut on = s.active_skills.contains(&sk.name);
                kit::setting_row(ui, &sk.name, &sk.description, |ui| {
                    if kit::switch(ui, &mut on).changed() {
                        if on {
                            s.active_skills.push(sk.name.clone());
                        } else {
                            s.active_skills.retain(|n| *n != sk.name);
                        }
                        changed = true;
                    }
                });
            }
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if kit::secondary(ui, t(lang, "Open Themisio's skills folder")).clicked()
                && let Some(d) = super::skills::own_dir()
            {
                let _ = std::fs::create_dir_all(&d);
                crate::shell::open_folder(&d);
            }
            if kit::secondary(ui, t(lang, "Add a skills folder…")).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                let d = dir.to_string_lossy().into_owned();
                if !s.skill_dirs.contains(&d) {
                    s.skill_dirs.push(d);
                    changed = true;
                }
                self.skills = None;
            }
            if kit::icon_button(ui, Icon::Retry, t(lang, "Refresh")).clicked() {
                self.skills = None;
            }
        });
        let mut drop = None;
        for (i, d) in s.skill_dirs.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(d).size(12.0));
                if kit::quiet(ui, t(lang, "Remove")).clicked() {
                    drop = Some(i);
                }
            });
        }
        if let Some(i) = drop {
            s.skill_dirs.remove(i);
            self.skills = None;
            changed = true;
        }
        changed
    }

    #[allow(clippy::too_many_arguments)]
    fn provider(&mut self, ui: &mut egui::Ui, s: &AiSettings, cfg: &mut ProviderCfg, id: &str, base: &str, key_url: &str, lang: Lang) -> bool {
        let ctx = ui.ctx().clone();
        let mut changed = false;
        if id.starts_with("custom") {
            kit::setting_row(ui, t(lang, "Name"), "", |ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut cfg.name).desired_width(240.0)).changed();
            });
            kit::row_rule(ui);
        }
        // Key.
        let keyless = id == "llamacpp" || id.starts_with("custom");
        kit::setting_row(ui, t(lang, "API key"), if keyless { t(lang, "Optional for this provider.") } else { "" }, |ui| {
            if self.key_saved(id) && !self.key_edit.contains_key(id) {
                if kit::quiet(ui, t(lang, "Remove")).clicked() {
                    let _ = keys::set(id, "");
                    self.has_key.insert(id.into(), false);
                }
                if kit::secondary(ui, t(lang, "Change")).clicked() {
                    self.key_edit.insert(id.into(), String::new());
                }
                kit::status(ui, kit::GREEN, t(lang, "Saved"));
            } else {
                let e = self.key_edit.entry(id.into()).or_default();
                let save = kit::primary(ui, t(lang, "Save")).clicked();
                ui.add(egui::TextEdit::singleline(e).password(true).hint_text(t(lang, "Paste the key")).desired_width(200.0));
                if save {
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
        });
        if !key_url.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.hyperlink_to(RichText::new(t(lang, "Get a key")).size(12.0), key_url);
            });
        }
        kit::row_rule(ui);
        kit::setting_row(ui, t(lang, "Address"), "", |ui| {
            changed |= ui.add(egui::TextEdit::singleline(&mut cfg.base_url).hint_text(base).desired_width(260.0)).changed();
        });
        kit::row_rule(ui);
        ui.label(RichText::new(t(lang, "Models")).size(13.0));
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            let mut drop = None;
            for (i, m) in cfg.models.iter().enumerate() {
                if kit::chip_removable(ui, m) {
                    drop = Some(i);
                }
            }
            if let Some(i) = drop {
                cfg.models.remove(i);
                changed = true;
            }
            if cfg.models.is_empty() {
                kit::hint(ui, t(lang, "No models yet — load the provider's list or add one by id."));
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let e = self.model_edit.entry(id.into()).or_default();
            let resp = ui.add(egui::TextEdit::singleline(e).hint_text(t(lang, "model id")).desired_width(170.0));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (kit::secondary(ui, t(lang, "Add")).clicked() || enter) && !e.trim().is_empty() {
                if !cfg.models.contains(&e.trim().to_string()) {
                    cfg.models.push(e.trim().to_string());
                    changed = true;
                }
                e.clear();
            }
            let loading = self.models_job.as_ref().is_some_and(|(p, _)| p == id);
            if loading {
                ui.spinner();
            } else if kit::secondary(ui, t(lang, "Load models")).clicked() {
                let mut probe = s.clone();
                probe.providers.insert(id.into(), cfg.clone());
                if let Some(ep) = probe.endpoint(id) {
                    self.models_job = Some((id.into(), Job::spawn(&ctx, move |_| super::list_models(&ep))));
                }
            }
            let testing = self.test_job.as_ref().is_some_and(|(p, _)| p == id);
            if testing {
                ui.spinner();
            } else if ui.add_enabled_ui(!cfg.models.is_empty(), |ui| kit::secondary(ui, t(lang, "Test"))).inner.clicked() {
                let mut probe = s.clone();
                let mut c = cfg.clone();
                c.enabled = true;
                probe.providers.insert(id.into(), c);
                let req = Request {
                    model: format!("{id}|{}", cfg.models[0]),
                    system: String::new(),
                    messages: vec![("user".into(), "Reply with the single word OK.".into())],
                    max_tokens: 0,
                    ..Default::default()
                };
                self.test_job = Some((id.into(), Job::spawn(&ctx, move |c| super::complete(&probe, &req, c))));
            }
        });
        if let Some(list) = self.fetched.get(id).cloned() {
            ui.add_space(4.0);
            kit::hint(ui, t(lang, "Tick the models to use:"));
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
            ui.add_space(4.0);
            kit::notice(ui, if *ok { kit::GREEN } else { kit::RED }, if *ok { Icon::Check } else { Icon::Cross }, |ui| {
                ui.add(egui::Label::new(RichText::new(text).size(12.0)).wrap());
            });
        }
        changed
    }

    fn poll(&mut self) {
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
            self.results.insert(
                id,
                match r {
                    Ok(text) => (true, format!("OK — {}", text.chars().take(80).collect::<String>())),
                    Err(e) => (false, e),
                },
            );
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
    egui::ComboBox::from_id_salt(("themisio", id)).selected_text(label).width(220.0).show_ui(ui, |ui| {
        ui.selectable_value(value, String::new(), if same_as { t(lang, "Same as the writing assistant") } else { t(lang, "First available") });
        for (group, models) in s.choices() {
            ui.label(RichText::new(group).small().weak());
            for (v, l) in models {
                ui.selectable_value(value, v, l);
            }
        }
    });
    (*value != before).then(|| value.clone())
}
