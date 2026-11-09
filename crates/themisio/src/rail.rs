//! The app rail on the left edge: Themisio's mark, Home, the four apps, and Settings.
//!
//! It is drawn on top of whatever app is shown, with that app's fonts and styles in effect, so
//! it uses only shapes, explicit colours and the proportional font every app defines.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app_kind::{AppKind, Page};
use crate::i18n::{Lang, t, tf};
use crate::theme::{BRAND, BRAND_DEEP, CREAM, GOLD};

pub const WIDTH: f32 = 76.0;
const ITEM: f32 = 56.0;

/// What the user did on the rail this pass.
pub enum RailAction {
    Show(Page),
    Settings,
    CloseApp(AppKind),
    /// Keep the bar open, or let it tuck away.
    TogglePin,
}

pub fn label(lang: Lang, page: Page) -> &'static str {
    match (lang, page) {
        (_, Page::Home) => t(lang, "Home"),
        (Lang::En, Page::App(k)) => k.name(),
        (Lang::Vi, Page::App(AppKind::Word)) => "Văn bản",
        (Lang::Vi, Page::App(AppKind::Deck)) => "Trình chiếu",
        (Lang::Vi, Page::App(AppKind::Grid)) => "Bảng tính",
        (Lang::Vi, Page::App(AppKind::Pdf)) => "PDF",
        (Lang::Vi, Page::App(AppKind::Drawio)) => "Sơ đồ",
    }
}

pub fn description(lang: Lang, kind: AppKind) -> &'static str {
    match kind {
        AppKind::Word => t(lang, "Create, open and edit Word documents"),
        AppKind::Deck => t(lang, "Build and present slide shows"),
        AppKind::Grid => t(lang, "Calculate, chart and analyse data"),
        AppKind::Pdf => t(lang, "Read, annotate, sign and edit PDFs"),
        AppKind::Drawio => t(lang, "Draw flowcharts, UML and network diagrams"),
    }
}

/// Draw the rail in `rect`. `shown` is the page on screen (or being switched to); `loaded` tells
/// which apps are running (a dot beside the icon).
pub fn show(ui: &mut Ui, rect: Rect, shown: Page, loaded: &[AppKind], lang: Lang, pinned: bool) -> Option<RailAction> {
    let painter = ui.painter_at(rect);
    // The logo tile's navy, deepening towards the bottom.
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), BRAND);
    mesh.colored_vertex(rect.right_top(), BRAND);
    mesh.colored_vertex(rect.right_bottom(), BRAND_DEEP);
    mesh.colored_vertex(rect.left_bottom(), BRAND_DEEP);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
    painter.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, Color32::from_white_alpha(14)));
    let mut action = None;
    let label_font = crate::theme::ui_font(ui.ctx(), 10.0);
    let soft = |a: u8| Color32::from_rgba_unmultiplied(0xF3, 0xF6, 0xFB, a);

    // The logo, which also goes Home.
    let mark_center = pos2(rect.center().x, rect.top() + 32.0);
    let mark_rect = Rect::from_center_size(mark_center, vec2(36.0, 36.0));
    if let Some(logo) = logo(ui.ctx()) {
        painter.image(logo.id(), mark_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    let mark = ui.interact(mark_rect, ui.scope_id().with("themisio-rail-mark"), Sense::click()).on_hover_text("Themisio");
    if mark.clicked() {
        action = Some(RailAction::Show(Page::Home));
    }

    let pages = [Page::Home, Page::App(AppKind::Word), Page::App(AppKind::Deck), Page::App(AppKind::Grid), Page::App(AppKind::Drawio), Page::App(AppKind::Pdf)];
    let top = rect.top() + 66.0;
    let step = ITEM + 4.0;
    let index = pages.iter().position(|p| *p == shown).unwrap_or(0) as f32;
    // The selection glides to the page being shown.
    let pill_y = ui.ctx().animate_value_with_time(ui.scope_id().with("themisio-rail-pill"), top + index * step, 0.22);
    let pill = Rect::from_min_size(pos2(rect.left() + 7.0, pill_y), vec2(WIDTH - 14.0, ITEM));
    painter.rect_filled(pill, CornerRadius::same(10), soft(26));
    painter.rect_filled(Rect::from_min_size(pos2(rect.left(), pill_y + 16.0), vec2(3.0, ITEM - 32.0)), CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, GOLD);

    for (i, page) in pages.iter().enumerate() {
        let item = Rect::from_min_size(pos2(rect.left() + 7.0, top + i as f32 * step), vec2(WIDTH - 14.0, ITEM));
        let id = ui.scope_id().with(("themisio-rail", i));
        let response = ui.interact(item, id, Sense::click());
        let selected = *page == shown;
        let hover = ui.ctx().animate_bool_with_time(id.with("hover"), response.hovered() && !selected, 0.12);
        if hover > 0.0 {
            painter.rect_filled(item, CornerRadius::same(10), soft((16.0 * hover) as u8));
        }
        let fg = if selected { CREAM } else { soft(200) };
        let icon_center = pos2(item.center().x, item.top() + 21.0);
        match page {
            Page::Home => draw_home(&painter, icon_center, fg),
            Page::App(k) => draw_app_icon(&painter, icon_center, *k, fg, selected),
        }
        painter.text(pos2(item.center().x, item.bottom() - 10.0), egui::Align2::CENTER_CENTER, label(lang, *page), label_font.clone(), fg);
        if let Page::App(k) = page
            && loaded.contains(k)
            && !selected
        {
            painter.circle_filled(pos2(item.right() - 8.0, item.top() + 8.0), 2.5, GOLD);
        }
        let tip = match page {
            Page::Home => t(lang, "Home").to_string(),
            Page::App(k) => format!("{} — {}", label(lang, *page), description(lang, *k)),
        };
        let response = response.on_hover_text(tip);
        if response.clicked() {
            action = Some(RailAction::Show(*page));
        }
        if let Page::App(k) = page
            && loaded.contains(k)
        {
            response.context_menu(|ui| {
                if ui.button(tf(lang, "Go to {app}", &[("app", label(lang, *page))])).clicked() {
                    action = Some(RailAction::Show(*page));
                    ui.close();
                }
                if ui.button(tf(lang, "Close {app}", &[("app", label(lang, *page))])).clicked() {
                    action = Some(RailAction::CloseApp(*k));
                    ui.close();
                }
            });
        }
    }

    // Keep the bar open, or let it tuck away until the pointer reaches the window's edge.
    let dock_rect = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 84.0), vec2(WIDTH - 14.0, 40.0));
    let dock = ui.interact(dock_rect, ui.scope_id().with("themisio-rail-dock"), Sense::click());
    let dock_hover = ui.ctx().animate_bool_with_time(dock.id.with("hover"), dock.hovered(), 0.12);
    if dock_hover > 0.0 {
        painter.rect_filled(dock_rect, CornerRadius::same(10), soft((16.0 * dock_hover) as u8));
    }
    draw_dock(&painter, dock_rect.center(), if pinned { GOLD } else { soft(200) }, pinned);
    let tip = if pinned { t(lang, "Auto-hide the app bar") } else { t(lang, "Keep the app bar open") };
    if dock.on_hover_text(tip).clicked() {
        action = Some(RailAction::TogglePin);
    }

    // Settings, at the bottom.
    let gear_rect = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 38.0), vec2(WIDTH - 14.0, 40.0));
    let gear = ui.interact(gear_rect, ui.scope_id().with("themisio-rail-settings"), Sense::click());
    let gear_hover = ui.ctx().animate_bool_with_time(gear.id.with("hover"), gear.hovered(), 0.12);
    if gear_hover > 0.0 {
        painter.rect_filled(gear_rect, CornerRadius::same(10), soft((16.0 * gear_hover) as u8));
    }
    draw_gear(&painter, gear_rect.center(), soft(215));
    if gear.on_hover_text(t(lang, "Settings")).clicked() {
        action = Some(RailAction::Settings);
    }
    action
}

/// Themisio's logo as a texture (made once, mipmapped so it stays crisp when small).
pub fn logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique("themisio-logo-texture");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let img = image::load_from_memory(include_bytes!("../assets/logo-256.png")).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let options = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
    let handle = ctx.load_texture("themisio-logo", color, options);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

fn draw_home(p: &egui::Painter, c: Pos2, fg: Color32) {
    let s = Stroke::new(1.7, fg);
    let pts = vec![
        pos2(c.x - 8.0, c.y - 0.5),
        pos2(c.x, c.y - 8.0),
        pos2(c.x + 8.0, c.y - 0.5),
        pos2(c.x + 8.0, c.y + 8.0),
        pos2(c.x - 8.0, c.y + 8.0),
    ];
    p.add(egui::Shape::closed_line(pts, s));
    p.rect_filled(Rect::from_min_max(pos2(c.x - 2.2, c.y + 2.5), pos2(c.x + 2.2, c.y + 8.0)), CornerRadius { nw: 1, ne: 1, sw: 0, se: 0 }, fg);
}

/// The four app icons: a rounded tile in the app's strip colour from the logo, with a white glyph
/// centred on it.
pub fn draw_app_icon(p: &egui::Painter, c: Pos2, kind: AppKind, _fg: Color32, filled: bool) {
    let tile = Rect::from_center_size(c, vec2(26.0, 26.0));
    let accent = kind.accent();
    p.rect_filled(tile, CornerRadius::same(7), if filled { accent } else { accent.gamma_multiply(0.88) });
    let ink = Color32::WHITE;
    let s = Stroke::new(1.6, ink);
    match kind {
        AppKind::Word => {
            for (i, w) in [12.0f32, 12.0, 7.0].iter().enumerate() {
                let y = c.y - 4.5 + i as f32 * 4.5;
                p.line_segment([pos2(c.x - 6.0, y), pos2(c.x - 6.0 + w, y)], s);
            }
        }
        AppKind::Deck => {
            let screen = Rect::from_min_max(pos2(c.x - 7.0, c.y - 6.0), pos2(c.x + 7.0, c.y + 3.0));
            p.rect_stroke(screen, CornerRadius::same(1), s, StrokeKind::Middle);
            p.line_segment([pos2(c.x, c.y + 3.0), pos2(c.x, c.y + 6.5)], s);
            p.line_segment([pos2(c.x - 3.5, c.y + 6.5), pos2(c.x + 3.5, c.y + 6.5)], s);
        }
        AppKind::Grid => {
            let g = Rect::from_center_size(c, vec2(13.0, 13.0));
            p.rect_stroke(g, CornerRadius::same(1), s, StrokeKind::Middle);
            let thin = Stroke::new(1.1, ink);
            for k in 1..3 {
                let f = k as f32 / 3.0;
                p.line_segment([pos2(g.left() + g.width() * f, g.top()), pos2(g.left() + g.width() * f, g.bottom())], thin);
                p.line_segment([pos2(g.left(), g.top() + g.height() * f), pos2(g.right(), g.top() + g.height() * f)], thin);
            }
        }
        AppKind::Pdf => {
            let page = vec![
                pos2(c.x - 5.5, c.y - 7.0),
                pos2(c.x + 2.0, c.y - 7.0),
                pos2(c.x + 5.5, c.y - 3.5),
                pos2(c.x + 5.5, c.y + 7.0),
                pos2(c.x - 5.5, c.y + 7.0),
            ];
            p.add(egui::Shape::closed_line(page, s));
            let thin = Stroke::new(1.2, ink);
            p.line_segment([pos2(c.x + 2.0, c.y - 7.0), pos2(c.x + 2.0, c.y - 3.5)], thin);
            p.line_segment([pos2(c.x + 2.0, c.y - 3.5), pos2(c.x + 5.5, c.y - 3.5)], thin);
            p.line_segment([pos2(c.x - 2.5, c.y + 0.5), pos2(c.x + 2.5, c.y + 0.5)], s);
            p.line_segment([pos2(c.x - 2.5, c.y + 3.8), pos2(c.x + 1.0, c.y + 3.8)], s);
        }
        AppKind::Drawio => {
            // Three boxes joined: a little diagram.
            let a = Rect::from_center_size(pos2(c.x, c.y - 5.0), vec2(7.0, 5.0));
            let b = Rect::from_center_size(pos2(c.x - 5.0, c.y + 5.0), vec2(6.0, 4.5));
            let d = Rect::from_center_size(pos2(c.x + 5.0, c.y + 5.0), vec2(6.0, 4.5));
            for r in [a, b, d] {
                p.rect_filled(r, CornerRadius::same(1), ink);
            }
            let thin = Stroke::new(1.2, ink);
            p.line_segment([a.center_bottom(), pos2(c.x, c.y + 0.5)], thin);
            p.line_segment([pos2(b.center().x, c.y + 0.5), pos2(d.center().x, c.y + 0.5)], thin);
            p.line_segment([pos2(b.center().x, c.y + 0.5), b.center_top()], thin);
            p.line_segment([pos2(d.center().x, c.y + 0.5), d.center_top()], thin);
        }
    }
}

/// The app bar's own toggle: a window with its left column filled when the bar stays open, and
/// only outlined when it tucks away.
fn draw_dock(p: &egui::Painter, c: Pos2, fg: Color32, pinned: bool) {
    let frame = Rect::from_center_size(c, vec2(20.0, 16.0));
    p.rect_stroke(frame, CornerRadius::same(3), Stroke::new(1.6, fg), StrokeKind::Middle);
    let column = Rect::from_min_max(frame.min, pos2(frame.left() + 7.0, frame.bottom()));
    if pinned {
        p.rect_filled(column, CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, fg);
    } else {
        p.line_segment([pos2(column.right(), frame.top()), pos2(column.right(), frame.bottom())], Stroke::new(1.4, fg));
        // A small arrow: the bar slides out.
        let y = c.y;
        let x = c.x + 3.5;
        p.line_segment([pos2(x - 2.0, y - 3.0), pos2(x + 1.0, y)], Stroke::new(1.4, fg));
        p.line_segment([pos2(x + 1.0, y), pos2(x - 2.0, y + 3.0)], Stroke::new(1.4, fg));
    }
}

/// A gear: eight rounded teeth around a ring.
fn draw_gear(p: &egui::Painter, c: Pos2, fg: Color32) {
    let teeth = 8;
    let (outer, inner) = (9.0f32, 6.6f32);
    let mut pts = Vec::with_capacity(teeth * 4);
    for k in 0..teeth {
        let a = k as f32 * std::f32::consts::TAU / teeth as f32;
        let half = std::f32::consts::TAU / teeth as f32 / 2.0;
        for (da, r) in [(-half * 0.95, inner), (-half * 0.45, outer), (half * 0.45, outer), (half * 0.95, inner)] {
            let (sin, cos) = (a + da).sin_cos();
            pts.push(pos2(c.x + cos * r, c.y + sin * r));
        }
    }
    p.add(egui::Shape::closed_line(pts, Stroke::new(1.5, fg)));
    p.circle_stroke(c, 2.6, Stroke::new(1.5, fg));
}
