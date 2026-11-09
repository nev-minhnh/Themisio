//! Themisio's writing assistant, working the way Grammarly does:
//!
//! 1. **Collect** — every paragraph (Documents), text box on the slide (Presentation) or text
//!    cell (CalcSheet) is read; as you type or paste, only what changed is sent again (real-time
//!    when Settings › AI › "Check as you type" is on, else with the Check button).
//! 2. **Analyse** — against four criteria: Correctness, Clarity, Engagement and Delivery, and the
//!    writing goals you set (domain, formality, audience, intent, language variety).
//! 3. **Suggest** — issues are underlined in the text (red: correctness; blue, green, purple:
//!    clarity, engagement, delivery). Hover an underline for a card with the fix and a short
//!    explanation: Accept, Dismiss, or Add to dictionary. The side panel lists them all.
//! 4. **Write with AI** — compose from a prompt, rephrase (formal, friendly, concise…), ideas.
//! 5. **Report** — an overall score (0–100), counts per criterion, the document's tone, an
//!    originality check (web search), and a progress report; a personal dictionary.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AiSettings, Job, Request, json_in};
use crate::app_kind::AppKind;
use crate::hosts::Host;
use crate::i18n::{Lang, t, tf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cat {
    Correctness,
    Clarity,
    Engagement,
    Delivery,
}

impl Cat {
    pub const ALL: [Cat; 4] = [Cat::Correctness, Cat::Clarity, Cat::Engagement, Cat::Delivery];
    fn parse(s: &str) -> Cat {
        match s.trim().to_lowercase().as_str() {
            "clarity" => Cat::Clarity,
            "engagement" => Cat::Engagement,
            "delivery" | "tone" => Cat::Delivery,
            _ => Cat::Correctness,
        }
    }
    pub fn color(self) -> Color32 {
        match self {
            Cat::Correctness => Color32::from_rgb(0xF0, 0x3C, 0x45),
            Cat::Clarity => Color32::from_rgb(0x1A, 0x8C, 0xF8),
            Cat::Engagement => Color32::from_rgb(0x2B, 0xB8, 0x6A),
            Cat::Delivery => Color32::from_rgb(0x8B, 0x5C, 0xF6),
        }
    }
    fn label(self, lang: Lang) -> &'static str {
        match self {
            Cat::Correctness => t(lang, "Correctness"),
            Cat::Clarity => t(lang, "Clarity"),
            Cat::Engagement => t(lang, "Engagement"),
            Cat::Delivery => t(lang, "Delivery"),
        }
    }
    fn index(self) -> usize {
        match self {
            Cat::Correctness => 0,
            Cat::Clarity => 1,
            Cat::Engagement => 2,
            Cat::Delivery => 3,
        }
    }
    /// How much an issue lowers the score.
    fn weight(self) -> f32 {
        match self {
            Cat::Correctness => 4.0,
            Cat::Clarity => 2.0,
            Cat::Engagement => 1.0,
            Cat::Delivery => 1.5,
        }
    }
}

/// What the document is for (Grammarly's goals).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Goals {
    pub domain: String,
    pub formality: String,
    pub audience: String,
    pub intent: String,
    /// "auto", "en-US", "en-GB", "en-CA", "en-AU", "en-IN", "vi".
    pub variant: String,
}

impl Default for Goals {
    fn default() -> Self {
        Self { domain: "General".into(), formality: "Neutral".into(), audience: "General".into(), intent: "Inform".into(), variant: "auto".into() }
    }
}

const DOMAINS: &[&str] = &["General", "Academic", "Business", "Email", "Technical", "Creative", "Casual"];
const FORMALITY: &[&str] = &["Informal", "Neutral", "Formal"];
const AUDIENCE: &[&str] = &["General", "Knowledgeable", "Expert"];
const INTENT: &[&str] = &["Inform", "Describe", "Convince", "Tell a story"];
const VARIANTS: &[(&str, &str)] = &[
    ("auto", "Same as the text"),
    ("en-US", "American English"),
    ("en-GB", "British English"),
    ("en-CA", "Canadian English"),
    ("en-AU", "Australian English"),
    ("en-IN", "Indian English"),
    ("vi", "Vietnamese"),
];

/// Progress kept across sessions (the report).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// Suggestions accepted per criterion (Correctness, Clarity, Engagement, Delivery).
    pub accepted: [u64; 4],
    pub dismissed: u64,
    /// Accepted suggestions per ISO week ("2026-W45").
    pub weeks: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub cat: Cat,
    pub kind: String,
    pub title: String,
    pub original: String,
    pub replacement: String,
    pub explanation: String,
}

struct Unit {
    key: Value,
    text: String,
    hash: u64,
}

struct Batch {
    results: Vec<(u64, Vec<Issue>)>,
    tone: Option<(String, bool)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Suggestions,
    Goals,
    Write,
    Report,
    Originality,
}

/// An issue as shown: which unit, which issue.
#[derive(Clone)]
struct Shown {
    unit: usize,
    issue: Issue,
}

struct Hover {
    unit: usize,
    issue: Issue,
    rect: Rect,
    left_at: Option<f64>,
}

/// What the shell does after the assistant's pass.
#[derive(Default)]
pub struct Out {
    pub notices: Vec<(crate::notices::Level, String)>,
    /// Settings changed (goals, dictionary, stats): save them.
    pub save: bool,
}

pub struct Writer {
    pub panel: bool,
    /// Set by the shell while a dialog (Settings, …) covers the page: no hover cards then.
    pub blocked: bool,
    tab: Tab,
    filter: Option<Cat>,
    kind: Option<AppKind>,
    rev: Option<u64>,
    units: Vec<Unit>,
    checked: HashMap<u64, Vec<Issue>>,
    /// After a suggestion is accepted: the unit it changed (its hash before) and the text that was
    /// replaced, so the unit's other suggestions carry over instead of waiting for a new check.
    carry: Option<(u64, String)>,
    /// Suggestions carried over from an earlier version of a changed unit, shown until it is
    /// checked again (so the list is updated, not wiped).
    provisional: HashMap<u64, Vec<Issue>>,
    pending: HashSet<u64>,
    job: Option<Job<Batch>>,
    /// When the document last scrolled (checks wait until it has been still for a second).
    scrolled_at: f64,
    /// Stop checking was pressed: checks as you type pause until the text changes again.
    stopped_at: Option<f64>,
    changed_at: f64,
    /// Check everything now (the Check button, or a right-click Check writing).
    wanted: bool,
    dismissed: HashSet<(u64, String)>,
    tone: Option<(String, bool)>,
    error: Option<String>,
    hover: Option<Hover>,
    expanded: Option<(u64, String)>,
    // Write with AI.
    prompt: String,
    go_job: Option<Job<String>>,
    go_result: Option<String>,
    go_error: Option<String>,
    // Originality.
    orig_job: Option<Job<Value>>,
    orig: Option<Value>,
    orig_error: Option<String>,
    accepted_now: [u64; 4],
}

impl Default for Writer {
    fn default() -> Self {
        Self {
            panel: false,
            blocked: false,
            tab: Tab::Suggestions,
            filter: None,
            kind: None,
            rev: None,
            units: Vec::new(),
            checked: HashMap::new(),
            carry: None,
            provisional: HashMap::new(),
            pending: HashSet::new(),
            job: None,
            scrolled_at: 0.0,
            stopped_at: None,
            changed_at: 0.0,
            wanted: false,
            dismissed: HashSet::new(),
            tone: None,
            error: None,
            hover: None,
            expanded: None,
            prompt: String::new(),
            go_job: None,
            go_result: None,
            go_error: None,
            orig_job: None,
            orig: None,
            orig_error: None,
            accepted_now: [0; 4],
        }
    }
}

fn hash_of(text: &str, goals: &Goals) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    goals.hash(&mut h);
    h.finish()
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn week_key() -> String {
    use chrono::Datelike;
    let w = chrono::Local::now().iso_week();
    format!("{}-W{:02}", w.year(), w.week())
}

fn system_prompt(s: &AiSettings, lang: Lang) -> String {
    let g = &s.goals;
    let ui_lang = if lang == Lang::Vi { "Vietnamese" } else { "English" };
    let variant = VARIANTS.iter().find(|(c, _)| *c == g.variant).map_or("the text's own language", |(_, n)| n);
    let mut p = format!(
        "You are Themisio's writing assistant, an expert editor working like Grammarly. Review each numbered unit of the user's document against four criteria:\n\
         - correctness: spelling, grammar, punctuation, agreement, wrong or missing words (these are errors);\n\
         - clarity: wordy, vague or hard-to-follow phrasing, needless passive voice, overlong sentences — suggest a concise rewrite;\n\
         - engagement: repeated, flat or overused words and monotonous sentences — suggest livelier word choices (synonyms);\n\
         - delivery: tone and formality that miss the goals, hedging, impolite or overly casual phrasing.\n\
         Writing goals: domain {domain}; formality {formality}; audience {audience}; intent {intent}; language variety {variant}.\n\
         Keep each unit's language (never translate) and the author's meaning. Do not flag what is correct and fits the goals.\n\
         Reply with JSON only, no prose and no code fence:\n\
         {{\"issues\": [{{\"unit\": 1, \"category\": \"correctness|clarity|engagement|delivery\", \"type\": \"spelling|grammar|punctuation|word choice|wordiness|passive voice|repetition|tone|…\", \
         \"title\": \"<a short action in {ui_lang}, e.g. Correct the spelling>\", \"original\": \"<exact substring of that unit>\", \"replacement\": \"<its replacement; empty to delete>\", \
         \"explanation\": \"<one short sentence in {ui_lang}>\"}}], \"tone\": \"<the document's overall tone in 2–4 words, in {ui_lang}>\", \"tone_fits\": true}}\n\
         \"original\" is copied character for character from the unit and is as short as possible while unique in it. At most 8 issues per unit, most important first.",
        domain = g.domain,
        formality = g.formality,
        audience = g.audience,
        intent = g.intent,
    );
    if !s.dictionary.is_empty() {
        p.push_str(&format!("\nNever flag these words (the author's dictionary): {}.", s.dictionary.join(", ")));
    }
    if !s.instructions.trim().is_empty() {
        p.push_str(&format!("\n\nThe author's standing instructions:\n{}", s.instructions.trim()));
    }
    p.push_str(&super::skills::instructions(&s.active_skills, &s.skill_dirs));
    p
}

fn parse_batch(answer: &str, batch: &[(u64, String)]) -> Result<Batch, String> {
    let v = json_in(answer).ok_or("The model didn't answer in the expected form. Try again, or pick another model.")?;
    let mut by_unit: Vec<Vec<Issue>> = vec![Vec::new(); batch.len()];
    for i in v["issues"].as_array().into_iter().flatten() {
        let Some(n) = i["unit"].as_u64().map(|n| n as usize).filter(|n| *n >= 1 && *n <= batch.len()) else { continue };
        let text = &batch[n - 1].1;
        let original = i["original"].as_str().unwrap_or("").to_string();
        let replacement = i["replacement"].as_str().unwrap_or("").to_string();
        if original.is_empty() || original == replacement || !text.contains(&original) {
            continue;
        }
        let list = &mut by_unit[n - 1];
        if list.iter().any(|x: &Issue| x.original == original) || list.len() >= 8 {
            continue;
        }
        list.push(Issue {
            cat: Cat::parse(i["category"].as_str().unwrap_or("")),
            kind: i["type"].as_str().unwrap_or("").to_string(),
            title: i["title"].as_str().unwrap_or("").to_string(),
            original,
            replacement,
            explanation: i["explanation"].as_str().unwrap_or("").to_string(),
        });
    }
    let tone = v["tone"].as_str().filter(|s| !s.trim().is_empty()).map(|s| (s.to_string(), v["tone_fits"].as_bool().unwrap_or(true)));
    Ok(Batch { results: batch.iter().map(|(h, _)| *h).zip(by_unit).collect(), tone })
}

impl Writer {
    /// Open the panel and check everything (Check writing).
    pub fn check_now(&mut self) {
        self.panel = true;
        self.tab = Tab::Suggestions;
        self.wanted = true;
        self.error = None;
    }

    fn visible(&self, s: &AiSettings) -> Vec<Shown> {
        let dict: HashSet<String> = s.dictionary.iter().map(|w| w.to_lowercase()).collect();
        let mut out = Vec::new();
        for (u, unit) in self.units.iter().enumerate() {
            for issue in self.checked.get(&unit.hash).or_else(|| self.provisional.get(&unit.hash)).into_iter().flatten() {
                if self.dismissed.contains(&(unit.hash, issue.original.clone())) || dict.contains(&issue.original.trim().to_lowercase()) {
                    continue;
                }
                if !unit.text.contains(&issue.original) {
                    continue;
                }
                out.push(Shown { unit: u, issue: issue.clone() });
            }
        }
        out
    }

    fn score(&self, shown: &[Shown]) -> Option<u32> {
        let total: usize = self.units.iter().map(|u| words(&u.text)).sum();
        let checked: usize = self.units.iter().filter(|u| self.checked.contains_key(&u.hash)).map(|u| words(&u.text)).sum();
        if checked == 0 {
            return None;
        }
        // Weighted issues per 100 words, eased so a few slips cost a few points and a page full of
        // errors still scores above zero (100 / (1 + d/40): d = 4 → 91, 10 → 80, 40 → 50).
        let penalty: f32 = shown.iter().map(|s| s.issue.cat.weight()).sum();
        let per_100 = penalty * 100.0 / (checked.max(40) as f32);
        let _ = total;
        Some((100.0 / (1.0 + per_100 / 40.0)).clamp(0.0, 100.0).round() as u32)
    }

    /// One pass: read the document, run checks, underline, cards, panel.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, ctx: &egui::Context, host: &mut dyn Host, kind: AppKind, s: &mut AiSettings, lang: Lang, now: f64, area: Rect, dock: Rect, style: &Arc<egui::Style>) -> Out {
        let mut out = Out::default();
        if self.kind != Some(kind) {
            *self = Writer { panel: self.panel, tab: self.tab, accepted_now: self.accepted_now, ..Writer::default() };
            self.kind = Some(kind);
        }
        // 1. Collect.
        let rev = host.ai_revision();
        if self.rev != Some(rev) {
            self.rev = Some(rev);
            let units: Vec<Unit> = host.ai_units().into_iter().map(|(key, text)| Unit { hash: hash_of(&text, &s.goals), key, text }).collect();
            if let Some((old_hash, accepted)) = self.carry.take()
                && let Some(old) = self.units.iter().find(|u| u.hash == old_hash)
                && let Some(issues) = self.checked.get(&old_hash).cloned()
            {
                let carried: Vec<(u64, Vec<Issue>)> = units
                    .iter()
                    .filter(|u| u.key == old.key && !self.checked.contains_key(&u.hash))
                    .map(|u| (u.hash, issues.iter().filter(|i| i.original != accepted && u.text.contains(&i.original)).cloned().collect()))
                    .collect();
                self.checked.extend(carried);
            }
            let before: HashSet<u64> = self.units.iter().map(|u| u.hash).collect();
            if units.iter().any(|u| !before.contains(&u.hash)) {
                self.changed_at = now;
            }
            // A changed unit keeps its earlier suggestions whose text is still there until it is
            // checked again.
            let mut provisional = HashMap::new();
            for u in units.iter().filter(|u| !self.checked.contains_key(&u.hash)) {
                let earlier = self
                    .units
                    .iter()
                    .filter(|o| o.key == u.key && o.hash != u.hash)
                    .find_map(|o| self.checked.get(&o.hash).or_else(|| self.provisional.get(&o.hash)))
                    .or_else(|| self.provisional.get(&u.hash));
                if let Some(list) = earlier {
                    let keep: Vec<Issue> = list.iter().filter(|i| u.text.contains(&i.original)).cloned().collect();
                    if !keep.is_empty() {
                        provisional.insert(u.hash, keep);
                    }
                }
            }
            self.provisional = provisional;
            self.units = units;
        }
        // 2. Analyse what isn't checked yet.
        if let Some(job) = &self.job
            && let Some(r) = job.take()
        {
            self.job = None;
            match r {
                Ok(batch) => {
                    for (h, list) in batch.results {
                        self.pending.remove(&h);
                        self.provisional.remove(&h);
                        self.checked.insert(h, list);
                    }
                    if batch.tone.is_some() {
                        self.tone = batch.tone;
                    }
                }
                Err(e) => {
                    self.pending.clear();
                    self.wanted = false;
                    out.notices.push((crate::notices::Level::Error, e.clone()));
                    self.error = Some(e);
                }
            }
        }
        let model = s.model_for(&s.write_model);
        let delay = f64::from(s.auto_delay_ms.max(600)) / 1000.0;
        // Scrolling: hold checks until the page has been still for a second.
        if ctx.input(|i| i.smooth_scroll_delta != egui::Vec2::ZERO || i.raw.events.iter().any(|e| matches!(e, egui::Event::MouseWheel { .. }))) {
            self.scrolled_at = now;
        }
        let still = now - self.scrolled_at >= 1.0;
        let paused = self.stopped_at.is_some_and(|t| self.changed_at <= t);
        let due = self.wanted || (s.auto_check && still && !paused && now - self.changed_at >= delay);
        if self.job.is_none() && due {
            // As you type, only the text on screen is checked; the whole document only when you
            // ask (Check the whole document, Ctrl+Alt+G). Checked, unchanged text never goes again.
            let unchecked: Vec<usize> = (0..self.units.len()).filter(|&i| !self.checked.contains_key(&self.units[i].hash) && !self.pending.contains(&self.units[i].hash)).collect();
            let on_screen: Option<HashSet<usize>> = if self.wanted || unchecked.is_empty() {
                None
            } else {
                let view = host.ai_viewport();
                Some(unchecked.iter().copied().filter(|&i| view.is_none_or(|v| unit_on_screen(host, &self.units[i], v))).collect())
            };
            let todo: Vec<(u64, String)> = unchecked
                .iter()
                .filter(|i| on_screen.as_ref().is_none_or(|set| set.contains(i)))
                .map(|&i| &self.units[i])
                .map(|u| (u.hash, u.text.clone()))
                .fold(Vec::new(), |mut acc: Vec<(u64, String)>, u| {
                    let size: usize = acc.iter().map(|(_, t)| t.len()).sum();
                    if acc.len() < 24 && size + u.1.len() < 7000 && !acc.iter().any(|(h, _)| *h == u.0) {
                        acc.push(u);
                    }
                    acc
                });
            if todo.is_empty() {
                self.wanted = false;
            } else if let Some(model) = model.clone() {
                for (h, _) in &todo {
                    self.pending.insert(*h);
                }
                let numbered: String = todo.iter().enumerate().map(|(i, (_, t))| format!("[{}] {}\n\n", i + 1, t)).collect();
                let req = Request {
                    model,
                    system: system_prompt(s, lang),
                    messages: vec![("user".into(), numbered)],
                    effort: s.effort.clone(),
                    thinking: s.thinking,
                    max_tokens: 8000,
                    web: false,
                };
                let settings = s.clone();
                self.error = None;
                self.job = Some(Job::spawn(ctx, move |c| {
                    let answer = super::complete(&settings, &req, c)?;
                    parse_batch(&answer, &todo)
                }));
            } else if self.wanted {
                self.wanted = false;
                self.panel = true;
                self.error = Some(t(lang, "Sign in to an AI provider in Settings › AI first.").into());
            }
        }
        if self.job.is_some() || (s.auto_check && (now - self.changed_at < delay || !still)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        // 3. Suggest: underlines and the hover card.
        let shown = self.visible(s);
        let mut accept: Option<Shown> = None;
        let mut dismiss: Option<Shown> = None;
        let mut learn: Option<String> = None;
        let viewport = host.ai_viewport();
        if let Some(view) = viewport {
            let painter = ctx.layer_painter(egui::LayerId::background()).with_clip_rect(view);
            // Only when the pointer is on the page itself, not on a dialog, menu or popup above it.
            let pointer = ctx.pointer_hover_pos().filter(|p| !self.blocked && ctx.layer_id_at(*p).is_none_or(|l| l.order <= egui::Order::Middle));
            if self.blocked {
                self.hover = None;
            }
            let mut over: Option<(usize, Issue, Rect)> = None;
            for sh in shown.iter().take(150) {
                let key = self.units[sh.unit].key.clone();
                for r in host.ai_marks(&key, &sh.issue.original) {
                    if !view.intersects(r) {
                        continue;
                    }
                    let color = sh.issue.cat.color();
                    let y = r.bottom() - 1.0;
                    painter.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(2.0, color));
                    let hit = r.expand2(vec2(0.0, 3.0));
                    if pointer.is_some_and(|p| hit.contains(p)) {
                        painter.rect_filled(r, CornerRadius::same(2), color.gamma_multiply(0.14));
                        over = Some((sh.unit, sh.issue.clone(), r));
                    }
                }
            }
            match (over, &mut self.hover) {
                (Some((unit, issue, rect)), _) => self.hover = Some(Hover { unit, issue, rect, left_at: None }),
                (None, Some(h)) if h.left_at.is_none() => h.left_at = Some(now),
                _ => {}
            }
        }
        if let Some((unit, issue, rect)) = self.hover.as_ref().map(|h| (h.unit, h.issue.clone(), h.rect)) {
            let mut keep = false;
            let mut open_panel = false;
            let panel_open = self.panel;
            let card_pos = pos2(rect.left(), rect.bottom() + 6.0);
            let r = egui::Area::new(Id::unique("themisio-writer-card")).order(egui::Order::Foreground).fixed_pos(card_pos).constrain(true).show(ctx, |ui| {
                ui.set_style(style.clone());
                egui::Frame::popup(style).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_max_width(340.0);
                    issue_body(
                        ui,
                        &issue,
                        lang,
                        true,
                        &mut || accept = Some(Shown { unit, issue: issue.clone() }),
                        &mut || dismiss = Some(Shown { unit, issue: issue.clone() }),
                        &mut || learn = Some(issue.original.clone()),
                    );
                    if !panel_open && super::kit::quiet(ui, t(lang, "Open the writing panel")).clicked() {
                        open_panel = true;
                    }
                });
            });
            if open_panel {
                self.panel = true;
            }
            if ctx.pointer_hover_pos().is_some_and(|p| r.response.rect.expand(4.0).contains(p)) {
                keep = true;
            }
            if let Some(h) = &mut self.hover {
                if keep {
                    h.left_at = None;
                } else if h.left_at.is_some_and(|t| now - t > 0.35) {
                    self.hover = None;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(60));
                }
            }
        }

        // The round button with the count, and the panel.
        let open_count = shown.len();
        let score = self.score(&shown);
        self.button(ctx, area, kind, open_count, &shown, style, lang);
        if self.panel {
            self.panel_ui(ctx, host, dock, s, lang, style, &shown, score, &mut accept, &mut dismiss, &mut learn, &mut out);
        }

        // Act.
        if let Some(sh) = accept {
            let key = self.units.get(sh.unit).map(|u| u.key.clone()).unwrap_or(Value::Null);
            let old_hash = self.units.get(sh.unit).map(|u| u.hash);
            match host.ai_apply(&key, &sh.issue.original, &sh.issue.replacement) {
                Ok(()) => {
                    if let Some(h) = old_hash {
                        self.carry = Some((h, sh.issue.original.clone()));
                    }
                    let i = sh.issue.cat.index();
                    s.stats.accepted[i] += 1;
                    *s.stats.weeks.entry(week_key()).or_insert(0) += 1;
                    self.accepted_now[i] += 1;
                    out.save = true;
                    self.hover = None;
                }
                Err(e) => out.notices.push((crate::notices::Level::Error, tf(lang, "Couldn't apply the suggestion: {error}", &[("error", &e)]))),
            }
        }
        if let Some(sh) = dismiss {
            if let Some(u) = self.units.get(sh.unit) {
                self.dismissed.insert((u.hash, sh.issue.original.clone()));
            }
            s.stats.dismissed += 1;
            out.save = true;
            self.hover = None;
        }
        if let Some(word) = learn {
            let w = word.trim().to_string();
            if !w.is_empty() && !s.dictionary.iter().any(|d| d.eq_ignore_ascii_case(&w)) {
                s.dictionary.push(w.clone());
                out.save = true;
                out.notices.push((crate::notices::Level::Success, tf(lang, "“{word}” added to your dictionary", &[("word", &w)])));
            }
            self.hover = None;
        }
        out
    }

    /// Grammarly's round button: the number of suggestions, coloured by the most serious one.
    #[allow(clippy::too_many_arguments)]
    fn button(&mut self, ctx: &egui::Context, area: Rect, kind: AppKind, count: usize, shown: &[Shown], style: &Arc<egui::Style>, lang: Lang) {
        if self.panel {
            return;
        }
        let d = 40.0;
        let lift = if kind == AppKind::Grid { 122.0 } else { 70.0 };
        let at = pos2(area.right() - d - 26.0, area.bottom() - d - lift);
        let worst = Cat::ALL.into_iter().find(|c| shown.iter().any(|s| s.issue.cat == *c));
        let busy = self.job.is_some();
        let mut clicked = false;
        egui::Area::new(Id::unique("themisio-writer-button")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            ui.set_style(style.clone());
            let (r, resp) = ui.allocate_exact_size(vec2(d, d), Sense::click());
            let p = ui.painter();
            p.add(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color: Color32::from_black_alpha(70) }.as_shape(r, CornerRadius::same(20)));
            let fill = if resp.hovered() { crate::theme::BRAND } else { crate::theme::BRAND_DEEP };
            p.circle_filled(r.center(), d / 2.0, fill);
            let ring = worst.map_or(crate::theme::GOLD, Cat::color);
            p.circle_stroke(r.center(), d / 2.0 - 2.0, Stroke::new(2.5, ring));
            if busy {
                let a = ui.input(|i| i.time) as f32 * 4.0;
                let pts: Vec<Pos2> = (0..14).map(|k| {
                    let t = a + k as f32 * 0.22;
                    r.center() + vec2(t.cos(), t.sin()) * (d / 2.0 - 2.0)
                }).collect();
                p.add(egui::Shape::line(pts, Stroke::new(2.5, crate::theme::GOLD)));
                ui.ctx().request_repaint();
            }
            if count > 0 {
                p.text(r.center(), egui::Align2::CENTER_CENTER, count.min(99).to_string(), crate::theme::ui_font(ctx, 14.0), crate::theme::CREAM);
            } else {
                super::assist::draw_sparkle(p, r.center(), 8.0, crate::theme::GOLD);
            }
            clicked = resp.on_hover_text(t(lang, "Writing assistant")).clicked();
        });
        if clicked {
            self.panel = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn panel_ui(
        &mut self,
        ctx: &egui::Context,
        host: &mut dyn Host,
        area: Rect,
        s: &mut AiSettings,
        lang: Lang,
        style: &Arc<egui::Style>,
        shown: &[Shown],
        score: Option<u32>,
        accept: &mut Option<Shown>,
        dismiss: &mut Option<Shown>,
        learn: &mut Option<String>,
        out: &mut Out,
    ) {
        let rect = area;
        let mut close = false;
        super::kit::pane(ctx, "themisio-writer-panel", rect, style, |ui| {
            let p = super::kit::palette(ui);
            super::kit::pane_header(ui, t(lang, "Writing assistant"), |ui| {
                if super::kit::icon_button(ui, super::kit::Icon::Close, t(lang, "Close")).clicked() {
                    close = true;
                }
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 10 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(ui.available_height());
                // Header: score, counts, Check.
                ui.horizontal(|ui| {
                    score_ring(ui, score);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(t(lang, "Overall score")).font(crate::theme::bold_font(ui.ctx(), 14.0)).color(p.text));
                        let n = shown.len();
                        let checked = self.units.iter().filter(|u| self.checked.contains_key(&u.hash)).count();
                        ui.label(RichText::new(tf(lang, "{n} suggestions · {c} of {t} checked", &[("n", &n.to_string()), ("c", &checked.to_string()), ("t", &self.units.len().to_string())])).size(12.0).color(p.muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.job.is_some() {
                            ui.spinner();
                        }
                    });
                });
                ui.add_space(6.0);
                let all_checked = self.units.iter().all(|u| self.checked.contains_key(&u.hash));
                ui.horizontal(|ui| {
                    if self.job.is_some() || self.wanted {
                        // Stop: drop the request under way and the rest of a whole-document check.
                        if super::kit::secondary(ui, t(lang, "Stop checking")).clicked() {
                            self.job = None;
                            self.wanted = false;
                            self.pending.clear();
                            self.stopped_at = Some(ui.ctx().input(|i| i.time));
                        }
                    } else if super::kit::primary_enabled(ui, !all_checked, t(lang, "Check whole document")).clicked() {
                        self.wanted = true;
                        self.stopped_at = None;
                    }
                    super::kit::hint(ui, if s.auto_check { t(lang, "As you type, only the text on screen is checked.") } else { "" });
                });
                ui.add_space(8.0);
                // The four criteria: one tile each (click to show only that kind).
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let w = (ui.available_width() - 18.0) / 4.0;
                    for c in Cat::ALL {
                        let n = shown.iter().filter(|s| s.issue.cat == c).count();
                        let on = self.filter == Some(c);
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
                        let fill = if on { c.color().gamma_multiply(0.16) } else if resp.hovered() { p.surface_hover } else { p.surface };
                        ui.painter().rect(r, CornerRadius::same(8), fill, Stroke::new(1.0, if on { c.color() } else { p.border }), egui::StrokeKind::Inside);
                        ui.painter().rect_filled(Rect::from_min_size(r.min + vec2(8.0, 0.0), vec2(r.width() - 16.0, 3.0)), CornerRadius::same(1), c.color());
                        ui.painter().text(pos2(r.left() + 9.0, r.top() + 17.0), egui::Align2::LEFT_CENTER, n.to_string(), crate::theme::bold_font(ui.ctx(), 15.0), if n > 0 { c.color() } else { p.muted });
                        ui.painter().text(pos2(r.left() + 9.0, r.top() + 34.0), egui::Align2::LEFT_CENTER, c.label(lang), crate::theme::ui_font(ui.ctx(), 11.0), p.muted);
                        if resp.on_hover_text(c.label(lang)).clicked() {
                            self.filter = if on { None } else { Some(c) };
                            self.tab = Tab::Suggestions;
                        }
                    }
                });
                if let Some((tone, fits)) = &self.tone {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(if *fits { tf(lang, "Tone: {tone} — fits your goals", &[("tone", tone)]) } else { tf(lang, "Tone: {tone} — doesn't quite fit your goals", &[("tone", tone)]) })
                                .size(12.0)
                                .color(if *fits { Cat::Engagement.color() } else { Cat::Delivery.color() }),
                        )
                        .wrap(),
                    );
                }
                ui.add_space(8.0);
                // Tabs: an underline under the active one.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for (tab, label) in [
                        (Tab::Suggestions, t(lang, "Suggestions")),
                        (Tab::Goals, t(lang, "Goals")),
                        (Tab::Write, t(lang, "Write")),
                        (Tab::Report, t(lang, "Report")),
                        (Tab::Originality, t(lang, "Originality")),
                    ] {
                        let on = self.tab == tab;
                        let resp = ui.add(egui::Label::new(RichText::new(label).size(12.8).color(if on { p.text } else { p.muted })).sense(Sense::click()));
                        if on {
                            let r = resp.rect;
                            ui.painter().line_segment([pos2(r.left(), r.bottom() + 4.0), pos2(r.right(), r.bottom() + 4.0)], Stroke::new(2.0, p.accent));
                        }
                        if resp.clicked() {
                            self.tab = tab;
                        }
                    }
                });
                ui.add_space(5.0);
                let r = ui.available_rect_before_wrap();
                ui.painter().line_segment([pos2(r.left() - 14.0, r.top()), pos2(r.right() + 14.0, r.top())], Stroke::new(1.0, p.border));
                ui.add_space(8.0);
                match self.tab {
                    Tab::Suggestions => self.suggestions_tab(ui, host, lang, shown, accept, dismiss, learn),
                    Tab::Goals => self.goals_tab(ui, s, lang, out),
                    Tab::Write => self.write_tab(ui, host, s, lang, out),
                    Tab::Report => self.report_tab(ui, s, lang, shown, score),
                    Tab::Originality => self.originality_tab(ui, s, lang),
                }
            });
        });
        if close {
            self.panel = false;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn suggestions_tab(&mut self, ui: &mut egui::Ui, host: &mut dyn Host, lang: Lang, shown: &[Shown], accept: &mut Option<Shown>, dismiss: &mut Option<Shown>, learn: &mut Option<String>) {
        if let Some(e) = &self.error {
            ui.colored_label(Cat::Correctness.color(), e);
        }
        let list: Vec<&Shown> = shown.iter().filter(|s| self.filter.is_none_or(|f| s.issue.cat == f)).collect();
        if list.is_empty() {
            ui.horizontal_top(|ui| {
                super::assist::tick(ui);
                let all_checked = !self.units.is_empty() && self.units.iter().all(|u| self.checked.contains_key(&u.hash));
                let text = if all_checked {
                    t(lang, "No suggestions — your writing looks great.")
                } else if self.units.is_empty() {
                    t(lang, "Start writing, and suggestions show up here.")
                } else {
                    t(lang, "Press Check whole document, or turn on Check as you type in Settings › AI.")
                };
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(text).wrap());
                });
            });
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for sh in list.iter().take(300) {
                let h = self.units.get(sh.unit).map_or(0, |u| u.hash);
                let id = (h, sh.issue.original.clone());
                let open = self.expanded.as_ref() == Some(&id);
                let resp = egui::Frame::NONE
                    .fill(ui.visuals().faint_bg_color)
                    .stroke(Stroke::new(if open { 1.5 } else { 0.0 }, sh.issue.cat.color()))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if open {
                            let issue = sh.issue.clone();
                            let unit = sh.unit;
                            issue_body(ui, &issue, lang, false, &mut || *accept = Some(Shown { unit, issue: issue.clone() }), &mut || *dismiss = Some(Shown { unit, issue: issue.clone() }), &mut || *learn = Some(issue.original.clone()));
                        } else {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.0, sh.issue.cat.color());
                                ui.label(RichText::new(super::assist::preview(&sh.issue.original, 26)).strikethrough().weak());
                                ui.label("→");
                                ui.label(RichText::new(if sh.issue.replacement.is_empty() { "∅".to_string() } else { super::assist::preview(&sh.issue.replacement, 30) }).strong());
                            });
                            let title = if sh.issue.title.is_empty() { sh.issue.kind.clone() } else { sh.issue.title.clone() };
                            ui.label(RichText::new(title).small().weak());
                        }
                    })
                    .response;
                // Only a folded card is one big button; an open one leaves its buttons the clicks.
                if !open && resp.interact(Sense::click()).clicked() {
                    self.expanded = Some(id);
                    if let Some(u) = self.units.get(sh.unit) {
                        host.ai_focus(&u.key, &sh.issue.original);
                    }
                }
                ui.add_space(4.0);
            }
        });
    }

    fn goals_tab(&mut self, ui: &mut egui::Ui, s: &mut AiSettings, lang: Lang, out: &mut Out) {
        let before = s.goals.clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new(t(lang, "Set goals to get suggestions that fit what you're writing.")).small().weak());
            ui.add_space(4.0);
            for (label, value, options) in [
                (t(lang, "Domain"), &mut s.goals.domain, DOMAINS),
                (t(lang, "Formality"), &mut s.goals.formality, FORMALITY),
                (t(lang, "Audience"), &mut s.goals.audience, AUDIENCE),
                (t(lang, "Intent"), &mut s.goals.intent, INTENT),
            ] {
                ui.label(RichText::new(label).strong());
                ui.horizontal_wrapped(|ui| {
                    for o in options {
                        if ui.selectable_label(value == o, t(lang, o)).clicked() {
                            *value = o.to_string();
                        }
                    }
                });
                ui.add_space(4.0);
            }
            ui.label(RichText::new(t(lang, "Language variety")).strong());
            let name = VARIANTS.iter().find(|(c, _)| *c == s.goals.variant).map_or("", |(_, n)| n);
            egui::ComboBox::from_id_salt("themisio-writer-variant").selected_text(t(lang, name)).show_ui(ui, |ui| {
                for (c, n) in VARIANTS {
                    ui.selectable_value(&mut s.goals.variant, c.to_string(), t(lang, n));
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(t(lang, "Personal dictionary")).strong());
            ui.label(RichText::new(t(lang, "Words here are never flagged. Add them from a suggestion card.")).small().weak());
            let mut drop = None;
            ui.horizontal_wrapped(|ui| {
                for (i, w) in s.dictionary.iter().enumerate() {
                    if super::kit::chip_removable(ui, w) {
                        drop = Some(i);
                    }
                }
            });
            if let Some(i) = drop {
                s.dictionary.remove(i);
                out.save = true;
            }
        });
        if s.goals != before {
            out.save = true;
            // New goals: everything is checked again against them.
            self.rev = None;
            self.wanted = true;
        }
    }

    fn write_tab(&mut self, ui: &mut egui::Ui, host: &mut dyn Host, s: &mut AiSettings, lang: Lang, out: &mut Out) {
        let ctx = ui.ctx().clone();
        if let Some(job) = &self.go_job
            && let Some(r) = job.take()
        {
            self.go_job = None;
            match r {
                Ok(text) => self.go_result = Some(text),
                Err(e) => self.go_error = Some(e),
            }
        }
        ui.label(RichText::new(t(lang, "Compose, rewrite or brainstorm with AI. Works on the selection (or the paragraph at the cursor).")).small().weak());
        let mut run: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            for (label, instruction) in [
                ("More formal", "Rewrite it in a more formal, professional tone."),
                ("Friendlier", "Rewrite it in a warmer, friendlier tone."),
                ("Concise", "Make it more concise without losing meaning."),
                ("Expand", "Expand it with more detail and supporting points."),
                ("Simplify", "Rewrite it in simpler words and shorter sentences."),
                ("Improve it", "Improve its clarity, flow and word choice."),
                ("Ideas", "Brainstorm 8 ideas, angles or titles related to it, as a list."),
                ("Summarise", "Summarise it in a few sentences."),
            ] {
                if super::kit::secondary(ui, t(lang, label)).clicked() {
                    run = Some(instruction.to_string());
                }
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.prompt)
                .hint_text(t(lang, "Or tell AI what to write, e.g. Write an email asking for a deadline extension…"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if self.go_job.is_some() {
                ui.spinner();
                ui.label(t(lang, "Writing…"));
            } else if super::kit::primary_enabled(ui, !self.prompt.trim().is_empty(), t(lang, "Generate")).clicked() {
                run = Some(self.prompt.trim().to_string());
            }
        });
        if let Some(instruction) = run {
            let Some(model) = s.model_for(&s.write_model) else {
                self.go_error = Some(t(lang, "Sign in to an AI provider in Settings › AI first.").into());
                return;
            };
            let target = host.ai_target().map(|t| t.text).unwrap_or_default();
            let mut system = String::from(
                "You are a skilled writing assistant inside a document editor (like GrammarlyGO). Do what the user asks. Reply with the resulting text only — \
                 no preamble, no quotes, no explanation. Keep the text's language unless asked otherwise.",
            );
            let g = &s.goals;
            system.push_str(&format!("\nWriting goals: domain {}, formality {}, audience {}, intent {}.", g.domain, g.formality, g.audience, g.intent));
            if !s.instructions.trim().is_empty() {
                system.push_str(&format!("\n\nThe author's standing instructions:\n{}", s.instructions.trim()));
            }
            system.push_str(&super::skills::instructions(&s.active_skills, &s.skill_dirs));
            let prompt = if target.trim().is_empty() { instruction } else { format!("{instruction}\n\nText:\n<<<\n{target}\n>>>") };
            let req = Request { model, system, messages: vec![("user".into(), prompt)], effort: s.effort.clone(), thinking: s.thinking, max_tokens: 0, web: false };
            let settings = s.clone();
            self.go_error = None;
            self.go_result = None;
            self.go_job = Some(Job::spawn(&ctx, move |c| super::complete(&settings, &req, c)));
        }
        if let Some(e) = &self.go_error {
            ui.colored_label(Cat::Correctness.color(), e);
        }
        if let Some(text) = self.go_result.clone() {
            egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Frame::NONE.fill(ui.visuals().faint_bg_color).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(&text).selectable(true));
                });
            });
            ui.horizontal(|ui| {
                if super::kit::primary(ui, t(lang, "Insert")).on_hover_text(t(lang, "At the cursor, after the selection")).clicked() {
                    match host.ai_insert(&text) {
                        Ok(()) => out.notices.push((crate::notices::Level::Success, t(lang, "Text inserted").into())),
                        Err(e) => out.notices.push((crate::notices::Level::Error, e)),
                    }
                }
                if super::kit::secondary(ui, t(lang, "Replace selection")).clicked() {
                    let r = host.ai_target().ok_or_else(|| t(lang, "Select the text to replace.").to_string()).and_then(|tg| host.ai_replace_all(&tg, &text));
                    match r {
                        Ok(()) => out.notices.push((crate::notices::Level::Success, t(lang, "Text replaced").into())),
                        Err(e) => out.notices.push((crate::notices::Level::Error, e)),
                    }
                }
                if super::kit::secondary(ui, t(lang, "Copy")).clicked() {
                    ui.ctx().copy_text(text.clone());
                    out.notices.push((crate::notices::Level::Success, t(lang, "Copied to the clipboard").into()));
                }
            });
        }
    }

    fn report_tab(&mut self, ui: &mut egui::Ui, s: &AiSettings, lang: Lang, shown: &[Shown], score: Option<u32>) {
        let text: String = self.units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>().join("\n");
        let word_list: Vec<String> = text.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
        let n_words = word_list.len();
        let sentences = text.split(['.', '!', '?', '\n']).filter(|x| x.split_whitespace().count() > 0).count().max(1);
        let unique: HashSet<&String> = word_list.iter().collect();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new(t(lang, "This document")).strong());
            egui::Grid::new("themisio-report").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                let rows = [
                    (t(lang, "Overall score"), score.map_or("—".to_string(), |s| s.to_string())),
                    (t(lang, "Words"), n_words.to_string()),
                    (t(lang, "Sentences"), sentences.to_string()),
                    (t(lang, "Average sentence length"), format!("{:.1}", n_words as f32 / sentences as f32)),
                    (t(lang, "Reading time"), format!("{} min", (n_words as f32 / 230.0).ceil().max(1.0))),
                    (t(lang, "Unique words"), format!("{}%", if n_words == 0 { 0 } else { unique.len() * 100 / n_words })),
                ];
                for (k, v) in rows {
                    ui.label(k);
                    ui.label(RichText::new(v).strong());
                    ui.end_row();
                }
                for c in Cat::ALL {
                    ui.label(c.label(lang));
                    ui.label(RichText::new(shown.iter().filter(|x| x.issue.cat == c).count().to_string()).color(c.color()));
                    ui.end_row();
                }
            });
            // Most used words (the vocabulary).
            let stop: HashSet<&str> = [
                "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "is", "are", "was", "were", "be", "it", "this", "that", "with", "as", "at", "by",
                "i", "we", "you", "he", "she", "they", "not", "but", "from", "have", "has", "will", "can", "và", "là", "của", "các", "những", "một", "cho", "với",
                "trong", "được", "có", "không", "này", "đã", "để", "khi", "thì", "về", "người", "như", "từ", "đến", "cũng",
            ]
            .into_iter()
            .collect();
            let mut freq: HashMap<&str, usize> = HashMap::new();
            for w in &word_list {
                if w.chars().count() > 2 && !stop.contains(w.as_str()) && !w.chars().all(|c| c.is_ascii_digit()) {
                    *freq.entry(w.as_str()).or_insert(0) += 1;
                }
            }
            let mut top: Vec<(&str, usize)> = freq.into_iter().filter(|(_, n)| *n > 1).collect();
            top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            if !top.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(t(lang, "Most used words")).strong());
                ui.label(top.iter().take(10).map(|(w, n)| format!("{w} ({n})")).collect::<Vec<_>>().join(", "));
            }
            ui.add_space(8.0);
            ui.label(RichText::new(t(lang, "Your progress")).strong());
            let this_week = s.stats.weeks.get(&week_key()).copied().unwrap_or(0);
            let total: u64 = s.stats.accepted.iter().sum();
            ui.label(tf(lang, "{n} suggestions accepted this week, {t} in all", &[("n", &this_week.to_string()), ("t", &total.to_string())]));
            for c in Cat::ALL {
                ui.label(RichText::new(format!("{}: {}", c.label(lang), s.stats.accepted[c.index()])).small());
            }
            ui.label(RichText::new(tf(lang, "{n} dismissed", &[("n", &s.stats.dismissed.to_string())])).small().weak());
            let weeks: Vec<(&String, &u64)> = s.stats.weeks.iter().rev().take(8).collect();
            if weeks.len() > 1 {
                ui.add_space(4.0);
                let max = weeks.iter().map(|(_, n)| **n).max().unwrap_or(1).max(1) as f32;
                for (w, n) in weeks.into_iter().rev() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(w).small().monospace());
                        let (r, _) = ui.allocate_exact_size(vec2(160.0 * (*n as f32 / max).max(0.02), 10.0), Sense::hover());
                        ui.painter().rect_filled(r, CornerRadius::same(3), crate::theme::GOLD);
                        ui.label(RichText::new(n.to_string()).small());
                    });
                }
            }
        });
    }

    fn originality_tab(&mut self, ui: &mut egui::Ui, s: &AiSettings, lang: Lang) {
        let ctx = ui.ctx().clone();
        if let Some(job) = &self.orig_job
            && let Some(r) = job.take()
        {
            self.orig_job = None;
            match r {
                Ok(v) => self.orig = Some(v),
                Err(e) => self.orig_error = Some(e),
            }
        }
        ui.add(
            egui::Label::new(
                RichText::new(t(lang, "Searches the web for passages of this document that appear elsewhere. Needs a model that can search: Claude (API), Claude Code or Perplexity."))
                    .small()
                    .weak(),
            )
            .wrap(),
        );
        let model = s.choices().into_iter().flat_map(|(_, m)| m.into_iter().map(|(v, _)| v)).find(|v| super::can_search(v));
        let preferred = if super::can_search(&s.write_model) { Some(s.write_model.clone()) } else { model };
        ui.horizontal(|ui| {
            if self.orig_job.is_some() {
                ui.spinner();
                ui.label(t(lang, "Searching the web…"));
            } else if super::kit::primary_enabled(ui, preferred.is_some() && !self.units.is_empty(), t(lang, "Check originality")).clicked()
                && let Some(model) = preferred.clone()
            {
                let text: String = self.units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>().join("\n").chars().take(6000).collect();
                let req = Request {
                    model,
                    system: "You check text for plagiarism. Search the web for distinctive sentences of the user's text and find pages that contain the same or nearly the same wording. \
                             Reply with JSON only: {\"original_percent\": <0-100, how much of the text appears to be original>, \"matches\": [{\"quote\": \"<the user's passage>\", \"url\": \"<page>\", \"title\": \"<page title>\"}]}. \
                             List only real pages you found. If nothing matches, return original_percent 100 and no matches."
                        .into(),
                    messages: vec![("user".into(), text)],
                    effort: String::new(),
                    thinking: false,
                    max_tokens: 4000,
                    web: true,
                };
                let settings = s.clone();
                self.orig = None;
                self.orig_error = None;
                self.orig_job = Some(Job::spawn(&ctx, move |c| {
                    let answer = super::complete(&settings, &req, c)?;
                    json_in(&answer).ok_or_else(|| "The model didn't return a report.".to_string())
                }));
            }
        });
        if preferred.is_none() {
            ui.add(egui::Label::new(RichText::new(t(lang, "Turn on Claude, Claude Code or Perplexity in Settings › AI.")).small().color(super::kit::AMBER)).wrap());
        }
        if let Some(e) = &self.orig_error {
            ui.colored_label(Cat::Correctness.color(), e);
        }
        if let Some(v) = &self.orig {
            let pct = v["original_percent"].as_f64().unwrap_or(100.0).clamp(0.0, 100.0);
            ui.add_space(6.0);
            ui.label(RichText::new(tf(lang, "{p}% original", &[("p", &format!("{pct:.0}"))])).heading().color(if pct >= 90.0 { Cat::Engagement.color() } else { Cat::Correctness.color() }));
            let matches = v["matches"].as_array().cloned().unwrap_or_default();
            if matches.is_empty() {
                ui.label(t(lang, "No matching text found online."));
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for m in matches {
                    egui::Frame::NONE.fill(ui.visuals().faint_bg_color).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(format!("“{}”", m["quote"].as_str().unwrap_or(""))).italics());
                        if let Some(url) = m["url"].as_str() {
                            ui.hyperlink_to(RichText::new(m["title"].as_str().unwrap_or(url)).small(), url);
                        }
                    });
                    ui.add_space(4.0);
                }
            });
        }
    }
}

/// A suggestion: what to change, the fix as a button, why — and Dismiss, Add to dictionary.
fn issue_body(ui: &mut egui::Ui, issue: &Issue, lang: Lang, card: bool, accept: &mut dyn FnMut(), dismiss: &mut dyn FnMut(), learn: &mut dyn FnMut()) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 4.5, issue.cat.color());
        ui.label(RichText::new(issue.cat.label(lang)).small().color(issue.cat.color()));
        if !issue.kind.is_empty() {
            ui.label(RichText::new(format!("· {}", issue.kind)).small().weak());
        }
    });
    if !issue.title.is_empty() {
        ui.label(RichText::new(&issue.title).strong());
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&issue.original).strikethrough().color(Cat::Correctness.color().gamma_multiply(0.85)));
        let fix = if issue.replacement.is_empty() { t(lang, "(remove)").to_string() } else { issue.replacement.clone() };
        let b = egui::Button::new(RichText::new(fix).strong().color(Color32::WHITE)).fill(Cat::Engagement.color()).corner_radius(CornerRadius::same(6));
        if ui.add(b).on_hover_text(t(lang, "Accept")).clicked() {
            accept();
        }
    });
    if !issue.explanation.is_empty() {
        ui.label(RichText::new(&issue.explanation).small());
    }
    ui.horizontal(|ui| {
        if !card && super::kit::primary(ui, t(lang, "Accept")).clicked() {
            accept();
        }
        if super::kit::secondary(ui, t(lang, "Dismiss")).clicked() {
            dismiss();
        }
        let one_word = !issue.original.trim().contains(char::is_whitespace) && issue.cat == Cat::Correctness;
        if one_word && super::kit::secondary(ui, t(lang, "Add to dictionary")).clicked() {
            learn();
        }
    });
}

/// The overall score in a ring, coloured from red to green.
fn score_ring(ui: &mut egui::Ui, score: Option<u32>) {
    let (r, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
    let c = r.center();
    let p = ui.painter();
    p.circle_stroke(c, 22.0, Stroke::new(5.0, ui.visuals().faint_bg_color));
    if let Some(sc) = score {
        let f = sc as f32 / 100.0;
        let color = if sc >= 85 { Cat::Engagement.color() } else if sc >= 60 { crate::theme::GOLD } else { Cat::Correctness.color() };
        let n = (64.0 * f).max(2.0) as usize;
        let pts: Vec<Pos2> = (0..=n)
            .map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * f * k as f32 / n as f32;
                c + vec2(a.cos(), a.sin()) * 22.0
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(5.0, color)));
        p.text(c, egui::Align2::CENTER_CENTER, sc.to_string(), crate::theme::ui_font(ui.ctx(), 16.0), ui.visuals().strong_text_color());
    } else {
        p.text(c, egui::Align2::CENTER_CENTER, "—", crate::theme::ui_font(ui.ctx(), 16.0), ui.visuals().weak_text_color());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_answer_is_matched_to_its_units() {
        let batch = vec![(1, "I recieve it.".to_string()), (2, "The meeting is scheduled to take place at 3 PM.".to_string())];
        let answer = r#"{"issues":[{"unit":1,"category":"correctness","type":"spelling","original":"recieve","replacement":"receive","explanation":"x"},
            {"unit":2,"category":"clarity","original":"is scheduled to take place at","replacement":"is at"},
            {"unit":3,"category":"clarity","original":"nope","replacement":"x"},
            {"unit":1,"category":"engagement","original":"not in the text","replacement":"x"}],"tone":"Neutral","tone_fits":true}"#;
        let b = parse_batch(answer, &batch).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(b.results[0].1.len(), 1);
        assert_eq!(b.results[1].1[0].cat, Cat::Clarity);
        assert_eq!(b.tone, Some(("Neutral".into(), true)));
    }
}

/// Whether some of `u` is in `view`: its first and last words are found on screen, and the unit
/// counts as visible when either is inside the view or they straddle it (a long paragraph).
fn unit_on_screen(host: &mut dyn Host, u: &Unit, view: Rect) -> bool {
    let words: Vec<&str> = u.text.split_whitespace().collect();
    let (Some(first), Some(last)) = (words.first(), words.last()) else { return false };
    let a = host.ai_marks(&u.key, first);
    let b = if words.len() > 1 { host.ai_marks(&u.key, last) } else { Vec::new() };
    if a.is_empty() && b.is_empty() {
        // The app can't place it: treat it as visible (as before).
        return true;
    }
    if a.iter().chain(b.iter()).any(|r| r.intersects(view)) {
        return true;
    }
    let top = a.first().map(|r| r.top());
    let bottom = b.last().map(|r| r.bottom());
    matches!((top, bottom), (Some(t), Some(bm)) if t < view.top() && bm > view.bottom())
}
