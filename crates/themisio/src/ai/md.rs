//! A small Markdown renderer for AI answers: headings, paragraphs, **bold**, *italic*, `code`,
//! ~~strike~~, links, bullet and numbered lists (nested), quotes, rules, fenced code and tables.
//! Text is selectable. Unfinished markup while an answer streams in is drawn as plain text.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, CornerRadius, FontId, RichText, Stroke, Ui, vec2};

/// The colours and sizes of the text.
struct Look {
    body: FontId,
    bold: FontId,
    mono: FontId,
    text: Color32,
    strong: Color32,
    weak: Color32,
    code_bg: Color32,
    link: Color32,
}

impl Look {
    fn of(ui: &Ui, size: f32) -> Look {
        let ctx = ui.ctx();
        let v = ui.visuals();
        Look {
            body: crate::theme::ui_font(ctx, size),
            bold: crate::theme::bold_font(ctx, size),
            mono: FontId::monospace(size - 0.5),
            text: v.text_color(),
            strong: v.strong_text_color(),
            weak: v.weak_text_color(),
            code_bg: if v.dark_mode { Color32::from_white_alpha(18) } else { Color32::from_black_alpha(14) },
            link: v.hyperlink_color,
        }
    }
}

/// Draw `text` as Markdown at body size `size`.
pub fn show(ui: &mut Ui, text: &str, size: f32) {
    let look = Look::of(ui, size);
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut para: Vec<&str> = Vec::new();
    let flush = |ui: &mut Ui, para: &mut Vec<&str>| {
        if !para.is_empty() {
            let joined = para.join("\n");
            inline_label(ui, &joined, &look, look.body.clone(), look.text);
            para.clear();
        }
    };
    ui.spacing_mut().item_spacing.y = 5.0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        // Fenced code.
        if trimmed.starts_with("```") {
            flush(ui, &mut para);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            i += 1;
            code_block(ui, &code.join("\n"), &look);
            continue;
        }
        // A table: a row of cells followed by a |---| line.
        if trimmed.starts_with('|') && lines.get(i + 1).is_some_and(|l| is_table_rule(l)) {
            flush(ui, &mut para);
            let header = cells(trimmed);
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                rows.push(cells(lines[i].trim_start()));
                i += 1;
            }
            table(ui, &header, &rows, &look);
            continue;
        }
        if trimmed.is_empty() {
            flush(ui, &mut para);
            i += 1;
            continue;
        }
        // Headings.
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            flush(ui, &mut para);
            let size = match hashes {
                1 => look.body.size + 4.0,
                2 => look.body.size + 2.5,
                3 => look.body.size + 1.0,
                _ => look.body.size,
            };
            ui.add_space(if hashes <= 2 { 4.0 } else { 2.0 });
            inline_label(ui, trimmed[hashes..].trim(), &look, FontId::new(size, look.bold.family.clone()), look.strong);
            i += 1;
            continue;
        }
        // Rules.
        let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.len() >= 3 && (compact.chars().all(|c| c == '-') || compact.chars().all(|c| c == '*') || compact.chars().all(|c| c == '_')) {
            flush(ui, &mut para);
            ui.separator();
            i += 1;
            continue;
        }
        // Quotes.
        if let Some(rest) = trimmed.strip_prefix('>') {
            flush(ui, &mut para);
            let resp = egui::Frame::NONE.inner_margin(egui::Margin { left: 10, right: 0, top: 2, bottom: 2 }).show(ui, |ui| {
                inline_label(ui, rest.trim_start(), &look, look.body.clone(), look.weak);
            });
            let r = resp.response.rect;
            ui.painter().line_segment([r.left_top() + vec2(2.0, 0.0), r.left_bottom() + vec2(2.0, 0.0)], Stroke::new(3.0, look.weak.gamma_multiply(0.5)));
            i += 1;
            continue;
        }
        // List items.
        if let Some((marker, rest)) = list_item(trimmed) {
            flush(ui, &mut para);
            let indent = (line.len() - trimmed.len()) as f32;
            let level = (indent / 2.0).floor().min(6.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(2.0 + level * 16.0);
                let w = if marker == "•" { 14.0 } else { 8.0 * marker.chars().count() as f32 + 6.0 };
                let (r, _) = ui.allocate_exact_size(vec2(w, look.body.size + 4.0), egui::Sense::hover());
                if marker == "•" {
                    let c = egui::pos2(r.left() + 4.0, r.center().y);
                    if level >= 1.0 {
                        ui.painter().circle_stroke(c, 2.5, Stroke::new(1.2, look.text));
                    } else {
                        ui.painter().circle_filled(c, 2.6, look.text);
                    }
                } else {
                    ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, marker, look.body.clone(), look.weak);
                }
                inline_label(ui, rest, &look, look.body.clone(), look.text);
            });
            i += 1;
            continue;
        }
        para.push(line);
        i += 1;
    }
    flush(ui, &mut para);
}

/// `- x`, `* x`, `+ x` (→ "•") or `1. x` / `1) x` (→ "1.").
fn list_item(s: &str) -> Option<(String, &str)> {
    for b in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = s.strip_prefix(b) {
            return Some(("•".into(), rest));
        }
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let after = &s[digits..];
        if let Some(rest) = after.strip_prefix(". ").or_else(|| after.strip_prefix(") ")) {
            return Some((format!("{}.", &s[..digits]), rest));
        }
    }
    None
}

fn is_table_rule(l: &str) -> bool {
    let t = l.trim();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn cells(row: &str) -> Vec<String> {
    let t = row.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn code_block(ui: &mut Ui, code: &str, look: &Look) {
    egui::Frame::NONE.fill(look.code_bg).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::horizontal().id_salt(code.len()).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(code).font(look.mono.clone()).color(look.text)).selectable(true).extend());
        });
    });
}

fn table(ui: &mut Ui, header: &[String], rows: &[Vec<String>], look: &Look) {
    let cols = header.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    egui::Frame::NONE.stroke(Stroke::new(1.0, look.weak.gamma_multiply(0.35))).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
        egui::ScrollArea::horizontal().id_salt(("md-table", header.join("|"))).show(ui, |ui| {
            egui::Grid::new(("md-table-grid", header.join("|"), rows.len())).striped(true).spacing(vec2(14.0, 4.0)).show(ui, |ui| {
                for c in 0..cols {
                    cell_label(ui, header.get(c).map_or("", String::as_str), look, look.bold.clone(), look.strong);
                }
                ui.end_row();
                for row in rows {
                    for c in 0..cols {
                        let cell = row.get(c).map_or("", String::as_str);
                        let numeric = cell.chars().any(|ch| ch.is_ascii_digit()) && cell.chars().all(|ch| ch.is_ascii_digit() || ".,%-+$ €₫".contains(ch));
                        if numeric {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| cell_label(ui, cell, look, look.body.clone(), look.text));
                        } else {
                            cell_label(ui, cell, look, look.body.clone(), look.text);
                        }
                    }
                    ui.end_row();
                }
            });
        });
    });
}

/// One wrapped, selectable label with inline markup.
fn inline_label(ui: &mut Ui, text: &str, look: &Look, font: FontId, color: Color32) {
    let mut job = LayoutJob::default();
    inline(&mut job, text, look, &font, color);
    job.wrap.max_width = ui.available_width();
    ui.add(egui::Label::new(job).selectable(true).wrap());
}

/// A table cell: on one line (the table scrolls sideways when it is wide).
fn cell_label(ui: &mut Ui, text: &str, look: &Look, font: FontId, color: Color32) {
    let mut job = LayoutJob::default();
    inline(&mut job, text, look, &font, color);
    job.wrap.max_width = f32::INFINITY;
    ui.add(egui::Label::new(job).selectable(true).extend());
}

/// Lay out `text` with **bold**, *italic*, `code`, ~~strike~~ and [links](url).
fn inline(job: &mut LayoutJob, text: &str, look: &Look, font: &FontId, color: Color32) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut buf = String::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let base_bold = font.family == look.bold.family;
    let push = |job: &mut LayoutJob, buf: &mut String, bold: bool, italic: bool, strike: bool| {
        if buf.is_empty() {
            return;
        }
        let f = if bold || base_bold { FontId::new(font.size, look.bold.family.clone()) } else { font.clone() };
        let mut fmt = TextFormat { font_id: f, color: if bold { look.strong } else { color }, italics: italic, ..Default::default() };
        if strike {
            fmt.strikethrough = Stroke::new(1.0, color);
        }
        job.append(buf, 0.0, fmt);
        buf.clear();
    };
    let closes = |from: usize, pat: &[char]| -> bool {
        let n = pat.len();
        (from..chars.len().saturating_sub(n - 1)).any(|k| chars[k..k + n] == *pat)
    };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // Escapes.
        if c == '\\' && next.is_some_and(|n| "*_`~[]\\#|".contains(n)) {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        // `code`
        if c == '`'
            && let Some(end) = (i + 1..chars.len()).find(|&k| chars[k] == '`')
        {
            push(job, &mut buf, bold, italic, strike);
            let code: String = chars[i + 1..end].iter().collect();
            job.append(&code, 0.0, TextFormat { font_id: look.mono.clone(), color: look.strong, background: look.code_bg, ..Default::default() });
            i = end + 1;
            continue;
        }
        // **bold** / __bold__
        if (c == '*' || c == '_') && next == Some(c) {
            let pat = [c, c];
            if bold || closes(i + 2, &pat) {
                push(job, &mut buf, bold, italic, strike);
                bold = !bold;
                i += 2;
                continue;
            }
        }
        // ~~strike~~
        if c == '~' && next == Some('~') && (strike || closes(i + 2, &['~', '~'])) {
            push(job, &mut buf, bold, italic, strike);
            strike = !strike;
            i += 2;
            continue;
        }
        // *italic* / _italic_ (not inside words, so snake_case and 2*3 stay as they are)
        if c == '*' || c == '_' {
            let prev = if i == 0 { ' ' } else { chars[i - 1] };
            let opening = !italic && !prev.is_alphanumeric() && next.is_some_and(|n| !n.is_whitespace()) && closes(i + 1, &[c]);
            let closing = italic && !prev.is_whitespace() && next.is_none_or(|n| !n.is_alphanumeric());
            if opening || closing {
                push(job, &mut buf, bold, italic, strike);
                italic = !italic;
                i += 1;
                continue;
            }
        }
        // [text](url)
        if c == '['
            && let Some(close) = (i + 1..chars.len()).find(|&k| chars[k] == ']')
            && chars.get(close + 1) == Some(&'(')
            && let Some(end) = (close + 2..chars.len()).find(|&k| chars[k] == ')')
        {
            push(job, &mut buf, bold, italic, strike);
            let label: String = chars[i + 1..close].iter().collect();
            job.append(&label, 0.0, TextFormat { font_id: font.clone(), color: look.link, underline: Stroke::new(1.0, look.link.gamma_multiply(0.6)), ..Default::default() });
            i = end + 1;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    push(job, &mut buf, bold, italic, strike);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_markers() {
        assert_eq!(list_item("- a").map(|(m, r)| (m, r.to_string())), Some(("•".into(), "a".into())));
        assert_eq!(list_item("12. b").map(|(m, r)| (m, r.to_string())), Some(("12.".into(), "b".into())));
        assert!(list_item("2024 was").is_none());
        assert!(list_item("**bold**").is_none());
    }

    #[test]
    fn tables_are_recognised() {
        assert!(is_table_rule("|---|:--:|"));
        assert!(!is_table_rule("| a | b |"));
        assert_eq!(cells("| a | b |"), vec!["a", "b"]);
    }
}
