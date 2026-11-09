//! The CalcSheet assistant: a chat beside the workbook that reads the data (the selection and the
//! active sheet's used range), answers with insights and next steps, and — only when you allow
//! it — carries out what it proposes with CalcSheet's own commands: formulas, tables, sorting,
//! formatting and charts, on the sheet it names. Every step is an ordinary edit (Ctrl+Z undoes
//! it), and only a fixed list of commands can run.

use std::sync::Arc;

use egui::{Color32, CornerRadius, RichText, pos2, vec2};
use serde_json::Value;

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

#[derive(Clone, Debug)]
struct Msg {
    user: bool,
    text: String,
    actions: Vec<Action>,
    ran: bool,
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
}

impl Default for SheetChat {
    fn default() -> Self {
        Self { open: false, msgs: Vec::new(), input: String::new(), model: String::new(), effort: String::new(), thinking: false, job: None, error: None }
    }
}

const WIDTH: f32 = 400.0;

/// The `actions` block at the end of an answer, and the answer without it.
fn split_actions(answer: &str) -> (String, Vec<Action>) {
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

pub fn system_prompt(context: &str, docs: &str, lang: Lang) -> String {
    let reply_lang = if lang == Lang::Vi { "Vietnamese (unless the user writes in another language)" } else { "the user's language" };
    format!(
        "You are a data analyst working inside CalcSheet, a spreadsheet app like Excel. Help the user understand their data: summaries, trends, \
         comparisons, outliers, data-quality problems, and what to do next. Be concrete and concise; cite cells and ranges. Reply in {reply_lang}.\n\n\
         When a calculation, formula, table, sort, formatting or chart would help, propose it as actions the user can approve. Put the actions in ONE \
         fenced block at the very end of your reply, exactly like this:\n\
         ```actions\n[{{\"command\": \"cell.set\", \"params\": {{\"cell\": \"F2\", \"input\": \"=AVERAGE(B2:B50)\"}}, \"label\": \"Average sales in F2\"}}]\n```\n\
         Rules for actions: use only the commands documented below, with their parameters; write Excel formulas (in English function names, comma \
         separators) so results stay live instead of pasting numbers; never overwrite the user's data unless asked — write results next to it or on \
         a new sheet; to work on another sheet, first {{\"command\": \"sheet.activate\", \"params\": {{\"sheet\": \"Name\"}}}} (create it first with \
         home.insertSheet {{\"name\": \"…\"}} if it doesn't exist); charts go on the active sheet at \"at\" (a free cell). Give every action a short \
         \"label\". Leave the block out when no change is needed. Never invent data.\n\n\
         # Commands you may use\n{docs}\n\n# The workbook\n{context}"
    )
}

impl SheetChat {
    /// Send `prompt` with the workbook's `context` and the `docs` of the allowed commands.
    pub fn send(&mut self, ctx: &egui::Context, s: &AiSettings, lang: Lang, prompt: String, context: String, docs: String) {
        self.error = None;
        let model = if self.model.is_empty() { s.model_for(&s.sheet_model).unwrap_or_default() } else { self.model.clone() };
        if model.is_empty() {
            self.error = Some(t(lang, "Sign in to an AI provider in Settings ▸ AI first.").into());
            return;
        }
        self.model = model.clone();
        self.msgs.push(Msg { user: true, text: prompt, actions: Vec::new(), ran: false });
        let messages: Vec<(String, String)> = self
            .msgs
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|m| (if m.user { "user".to_string() } else { "assistant".to_string() }, m.text.clone()))
            .collect();
        let req = Request { model, system: system_prompt(&context, &docs, lang), messages, effort: self.effort.clone(), thinking: self.thinking, max_tokens: 8000 };
        let settings = s.clone();
        self.job = Some(Job::spawn(ctx, move |c| super::complete(&settings, &req, c)));
    }

    /// Record how the actions of message `i` went.
    pub fn ran(&mut self, i: usize, results: Vec<Result<(), String>>) {
        if let Some(m) = self.msgs.get_mut(i) {
            let mut it = results.into_iter();
            for a in m.actions.iter_mut().filter(|a| a.checked) {
                a.result = it.next();
            }
            m.ran = true;
        }
    }

    pub fn actions(&self, i: usize) -> Vec<Action> {
        self.msgs.get(i).map(|m| m.actions.iter().filter(|a| a.checked && ALLOWED.contains(&a.command.as_str())).cloned().collect()).unwrap_or_default()
    }

    /// The panel, at the right of `area`.
    pub fn ui(&mut self, ctx: &egui::Context, area: egui::Rect, s: &AiSettings, lang: Lang, style: &Arc<egui::Style>) -> Vec<ChatAct> {
        let mut acts = Vec::new();
        if let Some(job) = &self.job
            && let Some(r) = job.take()
        {
            self.job = None;
            match r {
                Ok(answer) => {
                    let (text, actions) = split_actions(&answer);
                    self.msgs.push(Msg { user: false, text, actions, ran: false });
                }
                Err(e) => self.error = Some(e),
            }
        }
        if !self.open {
            return acts;
        }
        let rect = egui::Rect::from_min_max(pos2(area.right() - WIDTH - 8.0, area.top() + 8.0), pos2(area.right() - 8.0, area.bottom() - 30.0));
        let mut close = false;
        egui::Area::new(egui::Id::unique("themisio-sheet-chat")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            ui.set_style(style.clone());
            egui::Frame::popup(style).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(WIDTH - 24.0);
                ui.set_height(rect.height() - 24.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                    super::assist::draw_sparkle(ui.painter(), r.center(), 7.0, crate::theme::GOLD);
                    ui.label(RichText::new(t(lang, "CalcSheet assistant")).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if super::assist::close_button(ui) {
                            close = true;
                        }
                        if !self.msgs.is_empty() && ui.small_button(t(lang, "New chat")).clicked() {
                            self.msgs.clear();
                            self.error = None;
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if self.model.is_empty() {
                        self.model = s.model_for(&s.sheet_model).unwrap_or_default();
                        if self.effort.is_empty() {
                            self.effort = s.effort.clone();
                        }
                        self.thinking = s.thinking;
                    }
                    let label = if self.model.is_empty() { t(lang, "No model — Settings ▸ AI").to_string() } else { s.label(&self.model) };
                    egui::ComboBox::from_id_salt("themisio-chat-model").selected_text(label).width(180.0).show_ui(ui, |ui| {
                        for (group, models) in s.choices() {
                            ui.label(RichText::new(group).small().weak());
                            for (v, l) in models {
                                ui.selectable_value(&mut self.model, v, l);
                            }
                        }
                    });
                    let effort_label = super::EFFORTS.iter().find(|(v, _)| *v == self.effort).map_or("Model default", |(_, l)| l);
                    egui::ComboBox::from_id_salt("themisio-chat-effort").selected_text(t(lang, effort_label)).width(80.0).show_ui(ui, |ui| {
                        for (v, l) in super::EFFORTS {
                            ui.selectable_value(&mut self.effort, v.to_string(), t(lang, l));
                        }
                    });
                    ui.checkbox(&mut self.thinking, t(lang, "Thinking"));
                });
                ui.separator();
                let input_h = 92.0;
                egui::ScrollArea::vertical().max_height(ui.available_height() - input_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    if self.msgs.is_empty() {
                        ui.label(RichText::new(t(lang, "Ask about the data on this sheet — the selection, or the whole sheet when nothing is selected.")).weak());
                        ui.add_space(6.0);
                        for q in ["Summarise this data and point out anything unusual", "What trends do you see?", "Suggest useful formulas and a chart", "Check the data for errors and duplicates"] {
                            if ui.button(t(lang, q)).clicked() {
                                acts.push(ChatAct::Send(t(lang, q).to_string()));
                            }
                        }
                    }
                    for (i, m) in self.msgs.iter_mut().enumerate() {
                        let fill = if m.user { ui.visuals().selection.bg_fill } else { ui.visuals().faint_bg_color };
                        egui::Frame::NONE.fill(fill).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(egui::Label::new(&m.text).selectable(true));
                            if !m.actions.is_empty() {
                                ui.add_space(4.0);
                                ui.label(RichText::new(t(lang, "Proposed changes")).strong().small());
                                for a in &mut m.actions {
                                    let allowed = ALLOWED.contains(&a.command.as_str());
                                    ui.horizontal(|ui| {
                                        ui.add_enabled(allowed && !m.ran, egui::Checkbox::without_text(&mut a.checked));
                                        let label = if a.label.is_empty() { a.command.clone() } else { a.label.clone() };
                                        ui.vertical(|ui| {
                                            ui.label(label);
                                            ui.label(RichText::new(format!("{} {}", a.command, a.params)).small().monospace().weak());
                                            if !allowed {
                                                ui.label(RichText::new(t(lang, "Not allowed — skipped")).small().color(Color32::from_rgb(0xF0, 0x3C, 0x45)));
                                            }
                                            match &a.result {
                                                Some(Ok(())) => {
                                                    ui.label(RichText::new(t(lang, "Done")).small().color(Color32::from_rgb(0x2B, 0xB8, 0x6A)));
                                                }
                                                Some(Err(e)) => {
                                                    ui.label(RichText::new(e).small().color(Color32::from_rgb(0xF0, 0x3C, 0x45)));
                                                }
                                                None => {}
                                            }
                                        });
                                    });
                                }
                                if !m.ran {
                                    let n = m.actions.iter().filter(|a| a.checked).count();
                                    if ui.add_enabled(n > 0, egui::Button::new(tf(lang, "Allow and run {n} changes", &[("n", &n.to_string())]))).clicked() {
                                        acts.push(ChatAct::Run(i));
                                    }
                                }
                            }
                        });
                        ui.add_space(6.0);
                    }
                    if self.job.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t(lang, "Analysing…"));
                            if ui.small_button(t(lang, "Cancel")).clicked() {
                                self.job = None;
                            }
                        });
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
                    }
                    if let Some(e) = &self.error {
                        ui.colored_label(Color32::from_rgb(0xF0, 0x3C, 0x45), e);
                    }
                });
                ui.separator();
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut self.input)
                        .hint_text(t(lang, "Ask about your data… (Ctrl+Enter to send)"))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                let send_key = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t(lang, "Nothing changes until you allow it.")).small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if (ui.add_enabled(self.job.is_none() && !self.input.trim().is_empty(), egui::Button::new(t(lang, "Send"))).clicked() || send_key)
                            && self.job.is_none()
                            && !self.input.trim().is_empty()
                        {
                            acts.push(ChatAct::Send(std::mem::take(&mut self.input).trim().to_string()));
                        }
                    });
                });
            });
        });
        if close {
            self.open = false;
        }
        acts
    }
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
}
