//! Themisio's notifications: one kind of popup for everything that finishes, whichever app did
//! it — saved, opened, exported, converted, printed, or failed. The apps' own toasts and status
//! messages are gathered here (each host hands them over), and Themisio adds the ones the apps
//! don't announce (a document that was saved, a file that finished opening). Cards stack at the
//! bottom right, ease in, stay while hovered, and leave on their own (errors stay longer).

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Success,
    Error,
    Info,
}

impl Level {
    fn color(self) -> Color32 {
        match self {
            Self::Success => Color32::from_rgb(0x2B, 0xB8, 0x6A),
            Self::Error => Color32::from_rgb(0xF0, 0x3C, 0x45),
            Self::Info => Color32::from_rgb(0x1A, 0x8C, 0xF8),
        }
    }
    fn lifetime(self) -> f64 {
        match self {
            Self::Error => 9.0,
            _ => 4.5,
        }
    }
}

/// A message's level from its words, in English or Vietnamese: failures read as errors, finished
/// work as success, anything else as information.
pub fn classify(text: &str) -> Level {
    let t = text.to_lowercase();
    let error = [
        "couldn't", "could not", "can't", "cannot", "can not", "failed", "failure", "error", "unable", "not available", "denied", "invalid",
        "missing", "not found", "không thể", "không mở", "không lưu", "không được", "lỗi", "thất bại", "không tìm thấy",
    ];
    if error.iter().any(|w| t.contains(w)) {
        return Level::Error;
    }
    let done = [
        "saved", "exported", "opened", "printed", "sent to", "converted", "created", "copied", "inserted", "done", "finished", "complete",
        "đã lưu", "đã xuất", "đã mở", "đã in", "đã chuyển", "đã tạo", "đã sao chép", "đã chèn", "xong", "hoàn tất",
    ];
    if done.iter().any(|w| t.contains(w)) {
        return Level::Success;
    }
    Level::Info
}

struct Notice {
    id: u64,
    level: Level,
    /// Which app it came from, shown small above the text.
    source: Option<String>,
    text: String,
    at: f64,
    /// Closed by the user (fades out).
    closing: Option<f64>,
}

#[derive(Default)]
pub struct Notices {
    list: Vec<Notice>,
    next: u64,
}

const MAX: usize = 4;
const WIDTH: f32 = 340.0;

impl Notices {
    /// Show `text` (an identical one shown in the last two seconds is not repeated).
    pub fn push(&mut self, level: Level, source: Option<&str>, text: impl Into<String>, now: f64) {
        let text = text.into();
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if self.list.iter().any(|n| n.text == text && now - n.at < 2.0) {
            return;
        }
        self.next += 1;
        self.list.push(Notice { id: self.next, level, source: source.map(str::to_string), text, at: now, closing: None });
        while self.list.len() > MAX {
            self.list.remove(0);
        }
    }

    /// [`Self::push`] with the level read from the words ([`classify`]).
    pub fn say(&mut self, source: Option<&str>, text: impl Into<String>, now: f64) {
        let text = text.into();
        let level = classify(&text);
        self.push(level, source, text, now);
    }

    pub fn ui(&mut self, ctx: &egui::Context, area: Rect, now: f64, style: &std::sync::Arc<egui::Style>) {
        if self.list.is_empty() {
            return;
        }
        let small = crate::theme::ui_font(ctx, 11.0);
        let body = crate::theme::ui_font(ctx, 13.0);
        let mut bottom = area.bottom() - 16.0;
        let mut gone = Vec::new();
        for n in self.list.iter_mut().rev() {
            let age = now - n.at;
            let life = n.level.lifetime();
            let out_at = n.closing.unwrap_or(n.at + life);
            let leaving = (now - out_at).max(0.0);
            if leaving > 0.25 {
                gone.push(n.id);
                continue;
            }
            let enter = crate::motion::ease_out((age / 0.22).clamp(0.0, 1.0) as f32);
            let fade = enter * (1.0 - (leaving / 0.25) as f32);
            let v = &style.visuals;
            let galley = ctx.fonts_mut(|f| f.layout(n.text.clone(), body.clone(), v.text_color(), WIDTH - 64.0));
            let src = n.source.as_ref().map(|s| ctx.fonts_mut(|f| f.layout_no_wrap(s.clone(), small.clone(), v.weak_text_color())));
            let h = galley.size().y + src.as_ref().map_or(0.0, |g| g.size().y + 2.0) + 24.0;
            let x = area.right() - 16.0 - WIDTH + (1.0 - enter) * 24.0;
            let r = Rect::from_min_size(pos2(x, bottom - h), vec2(WIDTH, h));
            bottom -= h + 8.0;
            let level = n.level;
            let mut clicked = false;
            let mut hovered = false;
            egui::Area::new(Id::unique("themisio-notice").with(n.id))
                .order(egui::Order::Tooltip)
                .fixed_pos(r.min)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.set_style(style.clone());
                    let (rect, resp) = ui.allocate_exact_size(r.size(), Sense::click());
                    hovered = resp.hovered();
                    clicked = resp.clicked();
                    let painter = ui.painter();
                    painter.add(
                        egui::epaint::Shadow { offset: [0, 4], blur: 18, spread: 0, color: Color32::from_black_alpha((60.0 * fade) as u8) }
                            .as_shape(rect, CornerRadius::same(10)),
                    );
                    painter.rect(
                        rect,
                        CornerRadius::same(10),
                        v.window_fill.gamma_multiply(fade),
                        Stroke::new(1.0, v.window_stroke.color.gamma_multiply(fade)),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_filled(
                        Rect::from_min_size(rect.min, vec2(4.0, rect.height())),
                        CornerRadius { nw: 10, sw: 10, ne: 0, se: 0 },
                        level.color().gamma_multiply(fade),
                    );
                    draw_mark(painter, pos2(rect.left() + 26.0, rect.top() + 23.0), level, fade);
                    let mut y = rect.top() + 12.0;
                    if let Some(g) = src {
                        let gh = g.size().y;
                        painter.galley(pos2(rect.left() + 46.0, y), g, v.weak_text_color().gamma_multiply(fade));
                        y += gh + 2.0;
                    }
                    painter.galley(pos2(rect.left() + 46.0, y), galley, v.text_color().gamma_multiply(fade));
                    if hovered {
                        let c = pos2(rect.right() - 14.0, rect.top() + 14.0);
                        let s = 3.8;
                        let st = Stroke::new(1.3, v.text_color().gamma_multiply(fade));
                        painter.line_segment([c - vec2(s, s), c + vec2(s, s)], st);
                        painter.line_segment([c + vec2(-s, s), c + vec2(s, -s)], st);
                    }
                });
            if hovered && n.closing.is_none() {
                // Kept while the pointer is on it.
                n.at = n.at.max(now - (life - 1.5).max(0.0));
            }
            if clicked && n.closing.is_none() {
                n.closing = Some(now);
            }
        }
        self.list.retain(|n| !gone.contains(&n.id));
        if !self.list.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}

fn draw_mark(p: &egui::Painter, c: Pos2, level: Level, fade: f32) {
    p.circle_filled(c, 10.0, level.color().gamma_multiply(fade));
    let st = Stroke::new(1.8, Color32::WHITE.gamma_multiply(fade));
    match level {
        Level::Success => {
            p.line_segment([c + vec2(-4.5, 0.2), c + vec2(-1.3, 3.4)], st);
            p.line_segment([c + vec2(-1.3, 3.4), c + vec2(4.6, -3.2)], st);
        }
        Level::Error => {
            p.line_segment([c + vec2(-3.6, -3.6), c + vec2(3.6, 3.6)], st);
            p.line_segment([c + vec2(-3.6, 3.6), c + vec2(3.6, -3.6)], st);
        }
        Level::Info => {
            p.circle_filled(c + vec2(0.0, -4.0), 1.3, Color32::WHITE.gamma_multiply(fade));
            p.line_segment([c + vec2(0.0, -1.2), c + vec2(0.0, 4.5)], st);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_read_from_the_words() {
        assert_eq!(classify("Saved Report.docx"), Level::Success);
        assert_eq!(classify("Couldn't save: access denied"), Level::Error);
        assert_eq!(classify("Đã xuất PDF"), Level::Success);
        assert_eq!(classify("Không thể mở tệp"), Level::Error);
        assert_eq!(classify("3 slides selected"), Level::Info);
    }
}
