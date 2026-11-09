//! The CalcSheet assistant: a chat docked beside the workbook. It reads the data (every cell with
//! its address, as displayed, and a profile of every column over all rows), streams its answer
//! token by token as formatted text, and — only when you allow it — carries out what it proposes
//! with CalcSheet's own commands: formulas, tables, sorting, formatting and charts. Every step is
//! an ordinary edit (Ctrl+Z undoes it), only a fixed list of commands can run, and the cells it
//! wrote are checked afterwards: errors such as #DIV/0! are reported, with a one-click fix.

use std::sync::Arc;

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, vec2};
use serde_json::Value;

use super::kit::{self, Icon};
use super::{AiSettings, Job, Request};
use crate::i18n::{Lang, t, tf};

/// The commands the assistant may propose (and Themisio will run, after you allow them).
pub const ALLOWED: &[&str] = &[
    "cell.set",
    "range.setValues",
    "range.fill",
    "selection.set",
    "home.insertSheet",
    "sheet.activate",
    "sheet.rename",
    "insert.chart",
    "chart.changeType",
    "chart.quickLayout",
    "chart.switchRowColumn",
    "insert.table",
    "table.totalRow",
    "table.style",
    "insert.sparkline",
    "insert.pivotTable",
    "home.bold",
    "home.fillColor",
    "home.numberFormat",
    "home.fontColor",
    "home.autoSum",
    "data.sortAscending",
    "data.sortDescending",
    "data.filter",
    "home.conditionalFormatting",
    "format.conditional",
];

#[derive(Clone, Debug)]
pub struct Action {
    pub command: String,
    pub params: Value,
    pub label: String,
    pub checked: bool,
    pub result: Option<Result<(), String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ran {
    No,
    Skipped,
    Yes,
}

#[derive(Clone, Debug)]
struct Msg {
    user: bool,
    text: String,
    actions: Vec<Action>,
    ran: Ran,
    /// Cells showing errors after the actions ran: (cell, error, formula).
    errors: Vec<(String, String, String)>,
    details: bool,
}

impl Msg {
    fn user(text: String) -> Msg {
        Msg { user: true, text, actions: Vec::new(), ran: Ran::No, errors: Vec::new(), details: false }
    }
}

/// What the shell does for the panel.
pub enum ChatAct {
    /// Read the workbook (context and command docs) and send `prompt`.
    Send(String),
    /// Run the checked actions of message `i`.
    Run(usize),
}

pub struct SheetChat {
    pub open: bool,
    msgs: Vec<Msg>,
    input: String,
    pub model: String,
    pub effort: String,
    pub thinking: bool,
    job: Option<Job<String>>,
    error: Option<String>,
    /// The prompt to send again after an error.
    last_prompt: Option<String>,
}

impl Default for SheetChat {
    fn default() -> Self {
        Self { open: false, msgs: Vec::new(), input: String::new(), model: String::new(), effort: String::new(), thinking: false, job: None, error: None, last_prompt: None }
    }
}

/// The `actions` block at the end of an answer, and the answer without it.
pub fn split_actions(answer: &str) -> (String, Vec<Action>) {
    let (text, block) = if let Some(i) = answer.find("```actions") {
        let rest = &answer[i + "```actions".len()..];
        let end = rest.find("```").unwrap_or(rest.len());
        (format!("{}{}", &answer[..i], rest.get(end + 3..).unwrap_or("")), rest[..end].to_string())
    } else if let Some(i) = answer.find("```json") {
        let rest = &answer[i + "```json".len()..];
        let end = rest.find("```").unwrap_or(rest.len());
        let block = rest[..end].to_string();
        if block.contains("\"command\"") { (format!("{}{}", &answer[..i], rest.get(end + 3..).unwrap_or("")), block) } else { (answer.to_string(), String::new()) }
    } else {
        (answer.to_string(), String::new())
    };
    let v: Value = serde_json::from_str(block.trim()).unwrap_or(Value::Null);
    let list = v.as_array().cloned().or_else(|| v["actions"].as_array().cloned()).unwrap_or_default();
    let actions = list
        .into_iter()
        .filter_map(|a| {
            let command = a["command"].as_str()?.to_string();
            Some(Action {
                checked: ALLOWED.contains(&command.as_str()),
                label: a["label"].as_str().or_else(|| a["description"].as_str()).unwrap_or("").to_string(),
                params: if a["params"].is_object() { a["params"].clone() } else { serde_json::json!({}) },
                command,
                result: None,
            })
        })
        .collect();
    (text.trim().to_string(), actions)
}

/// The text of a reply while it streams: the prose before the actions block, and whether the
/// block has started.
fn streaming_view(partial: &str) -> (&str, bool) {
    match partial.find("```actions") {
        Some(i) => (&partial[..i], true),
        None => (partial, false),
    }
}

pub fn system_prompt(context: &str, docs: &str, lang: Lang) -> String {
    let reply_lang = if lang == Lang::Vi { "Vietnamese (unless the user writes in another language)" } else { "the user's language" };
    format!(
        "You are a careful data analyst working inside CalcSheet, a spreadsheet app like Excel. Help the user understand their data: summaries, \
         trends, comparisons, outliers, data-quality problems, and what to do next. Reply in {reply_lang}, in Markdown (short headings, bullet \
         lists, **bold** for key numbers, tables when they help). Be concrete and concise.\n\n\
         Accuracy rules:\n\
         - The workbook is described below with exact cell addresses: the first column of the cell listing is the row number and the other \
         columns are labelled with their sheet column letters. Always cite real addresses from it (e.g. \"B7:B52\"); never shift rows or columns.\n\
         - Values are shown as the sheet displays them (dates as dates, percentages as percentages). The column profile is computed over ALL \
         data rows; use it for counts, totals, minimum, maximum and averages. Rows left out of the listing are not visible to you: never \
         guess them — propose a formula instead, so the sheet computes the exact figure.\n\
         - Don't report a data problem unless the cells shown prove it; quote the cells.\n\n\
         When a calculation, formula, table, sort, formatting or chart would help, propose it as actions the user can approve. Put the \
         actions in ONE fenced block at the very end of your reply, exactly like this:\n\
         ```actions\n[{{\"command\": \"cell.set\", \"params\": {{\"cell\": \"F2\", \"input\": \"=AVERAGE(B2:B50)\"}}, \"label\": \"Average sales in F2\"}}]\n```\n\
         Rules for actions: use only the commands documented below, with their parameters. Write Excel formulas (English function names, \
         comma separators, absolute references like $B$7:$B$52 for data ranges) so results stay live instead of pasting numbers. A formula's \
         ranges must cover exactly the data rows of the right columns. Write each summary row's formula for that row (or use range.setValues \
         with every row's formula) — don't leave cells for a later fill. Never overwrite the user's data: write results in empty cells beside it \
         (at least one empty column away from the used range) or on a new sheet; to work on another sheet, first \
         {{\"command\": \"home.insertSheet\", \"params\": {{\"name\": \"…\"}}}} then {{\"command\": \"sheet.activate\", \"params\": {{\"sheet\": \"…\"}}}}. \
         Charts go on the active sheet at \"at\" (a free cell). Give every action a short \"label\". Leave the block out when no change is \
         needed. Never invent data.\n\n\
         # Commands you may use\n{docs}\n\n# The workbook\n{context}"
    )
}

impl SheetChat {
    /// Send `prompt` with the workbook's `context` and the `docs` of the allowed commands.
    pub fn send(&mut self, ctx: &egui::Context, s: &AiSettings, lang: Lang, prompt: String, context: String, docs: String) {
        self.error = None;
        let model = if self.model.is_empty() { s.model_for(&s.sheet_model).unwrap_or_default() } else { self.model.clone() };
        if model.is_empty() {
            self.error = Some(t(lang, "Sign in to an AI provider in Settings › AI first.").into());
            return;
        }
        self.model = model.clone();
        self.last_prompt = Some(prompt.clone());
        self.msgs.push(Msg::user(prompt));
        let messages: Vec<(String, String)> = self
            .msgs
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|m| (if m.user { "user".to_string() } else { "assistant".to_string() }, m.text.clone()))
            .collect();
        let req = Request { model, system: system_prompt(&context, &docs, lang), messages, effort: self.effort.clone(), thinking: self.thinking, max_tokens: 8000, web: false };
        let settings = s.clone();
        self.job = Some(Job::spawn_stream(ctx, move |c, on| super::complete_stream(&settings, &req, c, on)));
    }

    /// Record how the actions of message `i` went, and the cells that show errors afterwards.
    pub fn ran(&mut self, i: usize, results: Vec<Result<(), String>>, errors: Vec<(String, String, String)>) {
        if let Some(m) = self.msgs.get_mut(i) {
            let mut it = results.into_iter();
            for a in m.actions.iter_mut().filter(|a| a.checked) {
                a.result = it.next();
            }
            m.ran = Ran::Yes;
            m.errors = errors;
        }
    }

    pub fn actions(&self, i: usize) -> Vec<Action> {
        self.msgs.get(i).map(|m| m.actions.iter().filter(|a| a.checked && ALLOWED.contains(&a.command.as_str())).cloned().collect()).unwrap_or_default()
    }

    /// Pick up a finished answer (runs every frame, open or not).
    fn poll(&mut self) {
        if let Some(job) = &self.job
            && let Some(r) = job.take()
        {
            self.job = None;
            match r {
                Ok(answer) => {
                    let (text, actions) = split_actions(&answer);
                    self.msgs.push(Msg { user: false, text, actions, ran: Ran::No, errors: Vec::new(), details: false });
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// The pane, in `rect` (docked at the right of the workbook).
    pub fn ui(&mut self, ctx: &egui::Context, rect: egui::Rect, s: &AiSettings, lang: Lang, style: &Arc<egui::Style>) -> Vec<ChatAct> {
        let mut acts = Vec::new();
        self.poll();
        if !self.open {
            return acts;
        }
        if self.model.is_empty() {
            self.model = s.model_for(&s.sheet_model).unwrap_or_default();
            if self.effort.is_empty() {
                self.effort = s.effort.clone();
            }
            self.thinking = s.thinking;
        }
        let mut close = false;
        kit::pane(ctx, "themisio-sheet-chat", rect, style, |ui| {
            let p = kit::palette(ui);
            kit::pane_header(ui, t(lang, "CalcSheet assistant"), |ui| {
                if kit::icon_button(ui, Icon::Close, t(lang, "Close")).clicked() {
                    close = true;
                }
                if !self.msgs.is_empty() && kit::icon_button(ui, Icon::Compose, t(lang, "New chat")).clicked() {
                    self.msgs.clear();
                    self.error = None;
                    self.job = None;
                }
            });
            let composer_h = 132.0;
            let body_h = (ui.available_height() - composer_h).max(80.0);
            egui::ScrollArea::vertical().id_salt("themisio-chat-scroll").max_height(body_h).min_scrolled_height(body_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 8 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.msgs.is_empty() && self.job.is_none() {
                        self.empty_state(ui, lang, &mut acts);
                    }
                    let count = self.msgs.len();
                    for i in 0..count {
                        self.message(ui, i, lang, &mut acts);
                    }
                    if let Some(job) = &self.job {
                        let partial = job.partial();
                        let (prose, acting) = streaming_view(&partial);
                        assistant_row(ui, |ui| {
                            if prose.trim().is_empty() {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new(t(lang, "Reading the sheet and thinking…")).color(p.muted));
                                });
                            } else {
                                super::md::show(ui, prose, 13.0);
                            }
                            if acting {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new(t(lang, "Preparing changes…")).color(p.muted));
                                });
                            }
                        });
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
                    }
                    if let Some(e) = self.error.clone() {
                        ui.add_space(4.0);
                        kit::notice(ui, kit::RED, Icon::Cross, |ui| {
                            ui.add(egui::Label::new(RichText::new(e).color(p.text)).wrap());
                            if let Some(prompt) = self.last_prompt.clone()
                                && kit::quiet(ui, t(lang, "Try again")).clicked()
                            {
                                self.error = None;
                                if self.msgs.last().is_some_and(|m| m.user) {
                                    self.msgs.pop();
                                }
                                acts.push(ChatAct::Send(prompt));
                            }
                        });
                    }
                });
            });
            self.composer(ui, s, lang, &mut acts);
        });
        if close {
            self.open = false;
        }
        acts
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, lang: Lang, acts: &mut Vec<ChatAct>) {
        let p = kit::palette(ui);
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 22.0, crate::theme::GOLD.gamma_multiply(0.16));
            super::assist::draw_sparkle(ui.painter(), r.center(), 11.0, crate::theme::GOLD_DEEP);
            ui.add_space(6.0);
            ui.label(RichText::new(t(lang, "Ask about your data")).font(crate::theme::bold_font(ui.ctx(), 16.0)).color(p.text));
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(t(lang, "I read the selection, or the whole sheet when nothing is selected, and propose formulas and charts you can apply.")).size(12.5).color(p.muted)).wrap());
        });
        ui.add_space(14.0);
        for q in ["Summarise this data and point out anything unusual", "What trends do you see?", "Suggest useful formulas and a chart", "Check the data for errors and duplicates"] {
            let text = t(lang, q);
            let resp = ui.add(
                egui::Button::new(RichText::new(text).color(p.text).size(12.5))
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.border))
                    .corner_radius(CornerRadius::same(9))
                    .min_size(vec2(ui.available_width(), 34.0)),
            );
            if resp.clicked() {
                acts.push(ChatAct::Send(text.to_string()));
            }
            ui.add_space(2.0);
        }
    }

    fn message(&mut self, ui: &mut egui::Ui, i: usize, lang: Lang, acts: &mut Vec<ChatAct>) {
        let p = kit::palette(ui);
        let dark = ui.visuals().dark_mode;
        let Some(m) = self.msgs.get_mut(i) else { return };
        if m.user {
            // The user's message: a bubble on the right, in the logo's navy.
            let fill = if dark { Color32::from_rgb(0x1E, 0x3A, 0x5F) } else { crate::theme::BRAND };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let max = ui.available_width() * 0.85;
                egui::Frame::NONE.fill(fill).corner_radius(CornerRadius { nw: 14, ne: 4, sw: 14, se: 14 }).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_max_width(max);
                    ui.add(egui::Label::new(RichText::new(&m.text).color(crate::theme::CREAM).size(13.0)).wrap().selectable(true));
                });
            });
            ui.add_space(10.0);
            return;
        }
        let mut fix: Option<String> = None;
        let mut run = false;
        assistant_row(ui, |ui| {
            if !m.text.is_empty() {
                super::md::show(ui, &m.text, 13.0);
            }
            ui.horizontal(|ui| {
                if kit::icon_button(ui, Icon::Copy, t(lang, "Copy")).clicked() {
                    ui.ctx().copy_text(m.text.clone());
                }
            });
            if m.actions.is_empty() {
                return;
            }
            ui.add_space(4.0);
            kit::card(ui, |ui| {
                let n_checked = m.actions.iter().filter(|a| a.checked).count();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tf(lang, "Proposed changes ({n})", &[("n", &m.actions.len().to_string())])).font(crate::theme::bold_font(ui.ctx(), 13.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if kit::quiet(ui, if m.details { t(lang, "Hide details") } else { t(lang, "Details") }).clicked() {
                            m.details = !m.details;
                        }
                    });
                });
                ui.add_space(2.0);
                for a in &mut m.actions {
                    let allowed = ALLOWED.contains(&a.command.as_str());
                    ui.horizontal_top(|ui| {
                        match &a.result {
                            Some(Ok(())) => status_dot(ui, kit::GREEN, Icon::Check),
                            Some(Err(_)) => status_dot(ui, kit::RED, Icon::Cross),
                            None => {
                                kit::check(ui, &mut a.checked, allowed && m.ran == Ran::No);
                            }
                        }
                        ui.vertical(|ui| {
                            let label = if a.label.is_empty() { a.command.clone() } else { a.label.clone() };
                            ui.add(egui::Label::new(RichText::new(label).color(p.text).size(12.8)).wrap());
                            if m.details {
                                ui.add(egui::Label::new(RichText::new(format!("{} {}", a.command, a.params)).size(11.0).monospace().color(p.muted)).wrap());
                            }
                            if !allowed {
                                ui.label(RichText::new(t(lang, "Not allowed — skipped")).size(11.5).color(kit::RED));
                            }
                            if let Some(Err(e)) = &a.result {
                                ui.add(egui::Label::new(RichText::new(e).size(11.5).color(kit::RED)).wrap());
                            }
                        });
                    });
                    ui.add_space(2.0);
                }
                match m.ran {
                    Ran::No => {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if kit::primary_enabled(ui, n_checked > 0, tf(lang, "Allow and run {n} changes", &[("n", &n_checked.to_string())])).clicked() {
                                run = true;
                            }
                            if kit::secondary(ui, t(lang, "Skip")).clicked() {
                                m.ran = Ran::Skipped;
                            }
                        });
                        kit::hint(ui, t(lang, "Nothing changes until you allow it. Ctrl+Z undoes every change."));
                    }
                    Ran::Skipped => kit::hint(ui, t(lang, "Skipped — nothing was changed.")),
                    Ran::Yes => {
                        let failed = m.actions.iter().filter(|a| matches!(a.result, Some(Err(_)))).count();
                        if m.errors.is_empty() && failed == 0 {
                            ui.horizontal(|ui| {
                                status_dot(ui, kit::GREEN, Icon::Check);
                                ui.add(egui::Label::new(RichText::new(t(lang, "Done — the results were checked and show no errors.")).size(12.5).color(p.text)).wrap());
                            });
                        }
                    }
                }
            });
            if !m.errors.is_empty() {
                ui.add_space(6.0);
                kit::notice(ui, kit::AMBER, Icon::Warning, |ui| {
                    ui.label(RichText::new(if m.errors.len() == 1 { t(lang, "1 cell shows an error").to_string() } else { tf(lang, "{n} cells show errors", &[("n", &m.errors.len().to_string())]) }).font(crate::theme::bold_font(ui.ctx(), 13.0)).color(p.text));
                    for (cell, err, f) in m.errors.iter().take(8) {
                        ui.add(egui::Label::new(RichText::new(format!("{cell}  {err}  {f}")).size(11.5).monospace().color(p.muted)).wrap());
                    }
                    if m.errors.len() > 8 {
                        ui.label(RichText::new(tf(lang, "…and {n} more", &[("n", &(m.errors.len() - 8).to_string())])).size(11.5).color(p.muted));
                    }
                    ui.add_space(4.0);
                    if kit::primary(ui, t(lang, "Ask AI to fix them")).clicked() {
                        let list: Vec<String> = m.errors.iter().take(40).map(|(c, e, f)| format!("{c}: {e} ({f})")).collect();
                        fix = Some(format!(
                            "{}\n{}",
                            t(lang, "These cells you wrote show errors. Find the cause by checking each formula against the real columns and rows in the workbook below, then propose corrected actions:"),
                            list.join("\n")
                        ));
                    }
                });
            }
        });
        if run {
            acts.push(ChatAct::Run(i));
        }
        if let Some(f) = fix {
            acts.push(ChatAct::Send(f));
        }
        ui.add_space(12.0);
    }

    fn composer(&mut self, ui: &mut egui::Ui, s: &AiSettings, lang: Lang, acts: &mut Vec<ChatAct>) {
        let p = kit::palette(ui);
        let busy = self.job.is_some();
        egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 10 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let input_id = egui::Id::unique("themisio-chat-input");
            // Enter sends; Shift+Enter starts a new line.
            let focused = ui.memory(|m| m.has_focus(input_id));
            let enter = focused && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            let mut send = false;
            egui::Frame::NONE
                .fill(p.bg)
                .stroke(Stroke::new(1.0, if focused { p.accent.gamma_multiply(0.7) } else { p.border }))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin { left: 10, right: 8, top: 8, bottom: 6 })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().id_salt("themisio-chat-input-scroll").max_height(110.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.input)
                                .id(input_id)
                                .hint_text(t(lang, "Ask about your data…"))
                                .frame(egui::Frame::NONE)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Body),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let label = if self.model.is_empty() { t(lang, "No model — Settings › AI").to_string() } else { s.label(&self.model) };
                        let mut picked = None;
                        kit::chip_menu(ui, "themisio-chat-model", &label, 170.0, |ui| {
                            for (group, models) in s.choices() {
                                ui.label(RichText::new(group).size(11.0).color(p.muted));
                                for (v, l) in models {
                                    if ui.selectable_label(self.model == v, l).clicked() {
                                        picked = Some(v);
                                    }
                                }
                            }
                        });
                        if let Some(v) = picked {
                            self.model = v;
                        }
                        let effort_label = super::EFFORTS.iter().find(|(v, _)| *v == self.effort).map_or("Model default", |(_, l)| l);
                        let mut effort = None;
                        kit::chip_menu(ui, "themisio-chat-effort", t(lang, effort_label), 110.0, |ui| {
                            ui.label(RichText::new(t(lang, "Effort")).size(11.0).color(p.muted));
                            for (v, l) in super::EFFORTS {
                                if ui.selectable_label(self.effort == *v, t(lang, l)).clicked() {
                                    effort = Some(v.to_string());
                                }
                            }
                        });
                        if let Some(e) = effort {
                            self.effort = e;
                        }
                        kit::pill_toggle(ui, &mut self.thinking, t(lang, "Thinking"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if busy {
                                if kit::round_button(ui, Icon::Stop, true, t(lang, "Stop")).clicked() {
                                    self.job = None;
                                }
                            } else if kit::round_button(ui, Icon::Send, !self.input.trim().is_empty(), t(lang, "Send (Enter)")).clicked() {
                                send = true;
                            }
                        });
                    });
                });
            ui.add_space(4.0);
            ui.label(RichText::new(t(lang, "Enter to send · Shift+Enter for a new line")).size(11.0).color(p.muted));
            if (send || enter) && !busy && !self.input.trim().is_empty() {
                acts.push(ChatAct::Send(std::mem::take(&mut self.input).trim().to_string()));
                ui.memory_mut(|m| m.request_focus(input_id));
            }
        });
    }
}

/// An assistant reply: a small AI mark, then the content.
fn assistant_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 11.0, crate::theme::GOLD.gamma_multiply(0.18));
        super::assist::draw_sparkle(ui.painter(), r.center(), 6.0, crate::theme::GOLD_DEEP);
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    });
}

fn status_dot(ui: &mut egui::Ui, color: Color32, icon: Icon) {
    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 8.0, color);
    kit::paint_icon(ui.painter(), icon, r.center(), Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_are_split_from_the_answer() {
        let a = "Sales rose.\n```actions\n[{\"command\":\"cell.set\",\"params\":{\"cell\":\"F2\",\"input\":\"=SUM(B:B)\"},\"label\":\"Total\"},{\"command\":\"file.delete\",\"params\":{}}]\n```";
        let (text, acts) = split_actions(a);
        assert_eq!(text, "Sales rose.");
        assert_eq!(acts.len(), 2);
        assert!(acts[0].checked);
        assert!(!acts[1].checked, "a command outside the list is never ticked");
    }

    #[test]
    fn streaming_hides_the_actions_block() {
        assert_eq!(streaming_view("Hi\n```actions\n[{\"comm"), ("Hi\n", true));
        assert_eq!(streaming_view("Hi there"), ("Hi there", false));
    }
}
