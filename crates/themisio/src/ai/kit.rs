//! Shared pieces of Themisio's AI panels and Settings: the docked side pane, painted icon
//! buttons, primary/secondary buttons in the logo's colours, pill toggles and chips. One look
//! for the CalcSheet assistant, the writing assistant and Settings › AI.

use egui::{Color32, CornerRadius, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::theme::{BRAND, BRAND_DEEP, CREAM, GOLD, GOLD_DEEP, Palette};

/// The width of the docked pane (the CalcSheet assistant, the writing assistant).
pub const PANE_W: f32 = 392.0;

pub fn palette(ui: &Ui) -> Palette {
    Palette::of(ui.visuals().dark_mode)
}

/// The fill and text colour of primary actions: navy on light, gold on dark (the logo's colours).
pub fn primary_colors(dark: bool) -> (Color32, Color32, Color32) {
    if dark { (GOLD, GOLD_DEEP, BRAND_DEEP) } else { (BRAND, Color32::from_rgb(0x0E, 0x3A, 0x6E), CREAM) }
}

/// A filled button for the main action of a card or dialog.
pub fn primary(ui: &mut Ui, text: impl Into<String>) -> Response {
    primary_enabled(ui, true, text)
}

pub fn primary_enabled(ui: &mut Ui, enabled: bool, text: impl Into<String>) -> Response {
    let (fill, hover, fg) = primary_colors(ui.visuals().dark_mode);
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text.into()).color(fg).font(crate::theme::bold_font(ui.ctx(), 13.0)))
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(7))
            .min_size(vec2(0.0, 30.0)),
    );
    if resp.hovered() && enabled {
        ui.painter().rect_filled(resp.rect, CornerRadius::same(7), hover.gamma_multiply(0.35));
    }
    resp
}

/// An outlined button for the other actions.
pub fn secondary(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = palette(ui);
    ui.add(
        egui::Button::new(RichText::new(text.into()).color(p.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(CornerRadius::same(7))
            .min_size(vec2(0.0, 30.0)),
    )
}

/// A quiet text button (links in cards, "New chat").
pub fn quiet(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = palette(ui);
    ui.add(egui::Button::new(RichText::new(text.into()).color(p.accent).size(12.5)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).corner_radius(CornerRadius::same(6)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Close,
    Compose,
    Send,
    Stop,
    Chevron,
    ChevronUp,
    Check,
    Cross,
    Warning,
    Copy,
    Retry,
}

/// Draw `icon` centred at `c`.
pub fn paint_icon(p: &egui::Painter, icon: Icon, c: Pos2, color: Color32) {
    let st = Stroke::new(1.5, color);
    match icon {
        Icon::Close | Icon::Cross => {
            let r = if icon == Icon::Close { 4.5 } else { 3.5 };
            p.line_segment([c - vec2(r, r), c + vec2(r, r)], st);
            p.line_segment([c + vec2(-r, r), c + vec2(r, -r)], st);
        }
        Icon::Compose => {
            // A pencil over a square.
            let r = Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(11.0, 11.0));
            p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.3, color), egui::StrokeKind::Middle);
            p.line_segment([c + vec2(-0.5, 1.5), c + vec2(5.5, -4.5)], Stroke::new(1.8, color));
        }
        Icon::Send => {
            p.line_segment([c + vec2(0.0, 5.0), c + vec2(0.0, -5.0)], Stroke::new(1.9, color));
            p.line_segment([c + vec2(-4.5, -0.5), c + vec2(0.0, -5.0)], Stroke::new(1.9, color));
            p.line_segment([c + vec2(4.5, -0.5), c + vec2(0.0, -5.0)], Stroke::new(1.9, color));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(8.0, 8.0)), CornerRadius::same(2), color);
        }
        Icon::Chevron => {
            p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0)], st);
            p.line_segment([c + vec2(3.5, -1.5), c + vec2(0.0, 2.0)], st);
        }
        Icon::ChevronUp => {
            p.line_segment([c + vec2(-3.5, 1.5), c + vec2(0.0, -2.0)], st);
            p.line_segment([c + vec2(3.5, 1.5), c + vec2(0.0, -2.0)], st);
        }
        Icon::Check => {
            p.line_segment([c + vec2(-4.0, 0.3), c + vec2(-1.2, 3.2)], Stroke::new(1.8, color));
            p.line_segment([c + vec2(-1.2, 3.2), c + vec2(4.2, -3.0)], Stroke::new(1.8, color));
        }
        Icon::Warning => {
            let pts = vec![c + vec2(0.0, -6.0), c + vec2(6.5, 5.0), c + vec2(-6.5, 5.0)];
            p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
            p.line_segment([c + vec2(0.0, -2.2), c + vec2(0.0, 1.4)], Stroke::new(1.6, Color32::WHITE));
            p.circle_filled(c + vec2(0.0, 3.3), 0.9, Color32::WHITE);
        }
        Icon::Copy => {
            p.rect_stroke(Rect::from_center_size(c + vec2(1.5, 1.5), vec2(8.0, 9.0)), CornerRadius::same(2), Stroke::new(1.2, color), egui::StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(c + vec2(-1.5, -1.5), vec2(8.0, 9.0)), CornerRadius::same(2), Stroke::new(1.2, color), egui::StrokeKind::Middle);
        }
        Icon::Retry => {
            let n = 18;
            let pts: Vec<Pos2> = (0..=n)
                .map(|k| {
                    let a = 0.6 + 4.9 * k as f32 / n as f32;
                    c + vec2(a.cos(), a.sin()) * 4.8
                })
                .collect();
            let tip = *pts.last().unwrap_or(&c);
            p.add(egui::Shape::line(pts, st));
            p.line_segment([tip, tip + vec2(-0.5, -3.2)], st);
            p.line_segment([tip, tip + vec2(3.0, -0.6)], st);
        }
    }
}

/// A small square icon button with a hover fill and a tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let p = palette(ui);
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), p.surface_hover);
    }
    paint_icon(ui.painter(), icon, rect.center(), if resp.hovered() { p.text } else { p.muted });
    resp.on_hover_text(tip)
}

/// A round button: Send (accent fill) or Stop while a reply streams.
pub fn round_button(ui: &mut Ui, icon: Icon, enabled: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    let (fill, hover, fg) = primary_colors(ui.visuals().dark_mode);
    let p = palette(ui);
    let bg = if !enabled { p.border } else if resp.hovered() { hover } else { fill };
    ui.painter().circle_filled(rect.center(), 15.0, bg);
    paint_icon(ui.painter(), icon, rect.center(), if enabled { fg } else { p.muted });
    resp.on_hover_text(tip)
}

/// A pill that toggles (Thinking, Web); on = accent-tinted.
pub fn pill_toggle(ui: &mut Ui, on: &mut bool, text: &str) -> Response {
    let p = palette(ui);
    let fg = if *on { p.accent } else { p.muted };
    let fill = if *on { p.accent.gamma_multiply(0.14) } else { Color32::TRANSPARENT };
    let stroke = if *on { Stroke::new(1.0, p.accent.gamma_multiply(0.55)) } else { Stroke::new(1.0, p.border) };
    let resp = ui.add(egui::Button::new(RichText::new(text).size(12.0).color(fg)).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(12)).min_size(vec2(0.0, 24.0)));
    if resp.clicked() {
        *on = !*on;
    }
    resp
}

/// A rounded chip with a label and a value, opening a menu of choices (model, effort).
pub fn chip_menu(ui: &mut Ui, id: &str, text: &str, width: f32, add: impl FnOnce(&mut Ui)) {
    let p = palette(ui);
    let galley = ui.painter().layout_no_wrap(text.to_string(), crate::theme::ui_font(ui.ctx(), 12.0), p.text);
    let w = (galley.size().x + 30.0).min(width);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let fill = if resp.hovered() { p.surface_hover } else { Color32::TRANSPARENT };
    ui.painter().rect(rect, CornerRadius::same(12), fill, Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    let text_rect = Rect::from_min_max(rect.min + vec2(10.0, 0.0), rect.max - vec2(20.0, 0.0));
    ui.painter().with_clip_rect(text_rect).galley(pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0), galley, p.text);
    paint_icon(ui.painter(), Icon::Chevron, pos2(rect.right() - 11.0, rect.center().y), p.muted);
    let popup_id = ui.make_persistent_id(id);
    egui::Popup::menu(&resp).id(popup_id).width(width.max(200.0)).show(|ui| {
        ui.set_min_width(width.max(200.0));
        egui::ScrollArea::vertical().max_height(360.0).show(ui, add);
    });
}

/// A section heading in a pane or Settings.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).font(crate::theme::bold_font(ui.ctx(), 13.5)).color(palette(ui).text));
}

/// A muted explanation under a heading or control.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(palette(ui).muted)).wrap());
}

/// A bordered card.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ui);
    egui::Frame::NONE
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A tinted notice (warning, error, success) with an icon.
pub fn notice(ui: &mut Ui, color: Color32, icon: Icon, add: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(color.gamma_multiply(0.10))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                if icon == Icon::Warning {
                    paint_icon(ui.painter(), icon, r.center(), color);
                } else {
                    ui.painter().circle_filled(r.center(), 7.5, color);
                    paint_icon(ui.painter(), icon, r.center(), Color32::WHITE);
                }
                ui.vertical(add);
            });
        });
}

pub const RED: Color32 = Color32::from_rgb(0xE5, 0x48, 0x4D);
pub const GREEN: Color32 = Color32::from_rgb(0x2B, 0xA8, 0x62);
pub const AMBER: Color32 = Color32::from_rgb(0xE0, 0x8E, 0x0B);

/// The docked pane at `rect`: the app's surface colour, a hairline on its left edge, and
/// `add` laid out inside with the shell's style.
pub fn pane<R>(ctx: &egui::Context, id: &str, rect: Rect, style: &std::sync::Arc<egui::Style>, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Area::new(egui::Id::unique(id))
        .order(egui::Order::Middle)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_style(style.clone());
            let p = palette(ui);
            ui.painter().rect_filled(rect, CornerRadius::ZERO, p.surface);
            ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, p.border));
            let inner = rect.shrink2(vec2(0.0, 0.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(inner);
            child.set_min_size(inner.size());
            add(&mut child)
        })
        .inner
}

/// The pane's header: an AI mark, a title, and buttons on the right (added by `right`, laid out
/// right to left). Returns the header's bottom edge.
pub fn pane_header(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    let p = palette(ui);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 48.0), Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(14.0, 10.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let (r, _) = child.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    super::assist::draw_sparkle(child.painter(), r.center(), 8.0, GOLD);
    child.add_space(4.0);
    child.label(RichText::new(title).font(crate::theme::bold_font(child.ctx(), 14.0)).color(p.text));
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    ui.painter().line_segment([pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())], Stroke::new(1.0, p.border));
}

/// An on/off switch (Windows 11 style). Returns true when it was flipped.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.14);
    let p = palette(ui);
    let (fill, _, knob_on) = primary_colors(ui.visuals().dark_mode);
    let track = p.border.lerp_to_gamma(fill, t);
    ui.painter().rect(rect, CornerRadius::same(11), track, Stroke::new(1.0, if *on { fill } else { p.muted.gamma_multiply(0.6) }), egui::StrokeKind::Inside);
    let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
    let knob = if *on { knob_on } else { p.muted };
    ui.painter().circle_filled(pos2(x, rect.center().y), if resp.hovered() { 7.0 } else { 6.0 }, knob);
    resp
}

/// One row of a settings card: the name and an explanation on the left, the control on the right.
pub fn setting_row<R>(ui: &mut Ui, title: &str, desc: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ui);
    let w = ui.available_width();
    let label_w = (w * 0.46).max(180.0);
    ui.horizontal(|ui| {
        ui.set_min_height(36.0);
        ui.allocate_ui_with_layout(vec2(label_w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(label_w);
            ui.add(egui::Label::new(RichText::new(title).size(13.0).color(p.text)).wrap());
            if !desc.is_empty() {
                ui.add(egui::Label::new(RichText::new(desc).size(11.5).color(p.muted)).wrap());
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control).inner
    })
    .inner
}

/// A hairline between rows of a card.
pub fn row_rule(ui: &mut Ui) {
    let p = palette(ui);
    ui.add_space(6.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([pos2(r.left(), r.top()), pos2(r.right(), r.top())], Stroke::new(1.0, p.border.gamma_multiply(0.7)));
    ui.add_space(6.0);
}

/// An item of a settings sidebar.
pub fn nav_item(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let p = palette(ui);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    if selected {
        ui.painter().rect_filled(rect, CornerRadius::same(7), p.surface_hover);
        ui.painter().rect_filled(Rect::from_min_size(pos2(rect.left(), rect.center().y - 8.0), vec2(3.0, 16.0)), CornerRadius::same(2), p.accent);
    } else if hover > 0.0 {
        ui.painter().rect_filled(rect, CornerRadius::same(7), p.surface_hover.gamma_multiply(hover * 0.7));
    }
    let font = if selected { crate::theme::bold_font(ui.ctx(), 13.0) } else { crate::theme::ui_font(ui.ctx(), 13.0) };
    ui.painter().text(pos2(rect.left() + 14.0, rect.center().y), egui::Align2::LEFT_CENTER, text, font, if selected { p.text } else { p.text.gamma_multiply(0.85) });
    resp
}

/// A page title and its explanation, at the top of a settings page.
pub fn page_title(ui: &mut Ui, title: &str, desc: &str) {
    let p = palette(ui);
    ui.label(RichText::new(title).font(crate::theme::bold_font(ui.ctx(), 17.0)).color(p.text));
    if !desc.is_empty() {
        ui.add_space(2.0);
        ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(p.muted)).wrap());
    }
    ui.add_space(10.0);
}

/// A status dot and text (Ready, Needs a key, Off).
pub fn status(ui: &mut Ui, color: Color32, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.5, color);
        ui.label(RichText::new(text).size(11.5).color(color));
    });
}

/// A small removable chip (a model id, a dictionary word). True when its × was clicked.
pub fn chip_removable(ui: &mut Ui, text: &str) -> bool {
    let p = palette(ui);
    let galley = ui.painter().layout_no_wrap(text.to_string(), crate::theme::ui_font(ui.ctx(), 12.0), p.text);
    let size = vec2(galley.size().x + 30.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(12), p.surface_hover, Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    ui.painter().galley(pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, p.text);
    let x = Rect::from_center_size(pos2(rect.right() - 11.0, rect.center().y), vec2(16.0, 16.0));
    let resp = ui.interact(x, ui.scope_id().with(("chip-x", text)), Sense::click());
    if resp.hovered() {
        ui.painter().circle_filled(x.center(), 8.0, p.border);
    }
    paint_icon(ui.painter(), Icon::Cross, x.center(), if resp.hovered() { p.text } else { p.muted });
    resp.clicked()
}

/// A checkbox in the accent colour (clearer than egui's on both themes).
pub fn check(ui: &mut Ui, on: &mut bool, enabled: bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(18.0, 18.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let p = palette(ui);
    let (fill, _, fg) = primary_colors(ui.visuals().dark_mode);
    let r = Rect::from_center_size(rect.center(), vec2(16.0, 16.0));
    let alpha = if enabled { 1.0 } else { 0.45 };
    if *on {
        ui.painter().rect_filled(r, CornerRadius::same(4), fill.gamma_multiply(alpha));
        paint_icon(ui.painter(), Icon::Check, r.center(), fg.gamma_multiply(alpha));
    } else {
        let stroke = if resp.hovered() { p.text } else { p.muted };
        ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.3, stroke.gamma_multiply(alpha)), egui::StrokeKind::Inside);
    }
    resp
}
