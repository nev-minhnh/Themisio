//! Themisio's Home: create something new in any of the four apps, open a file, or pick up a recent
//! one. Drawn with the shell's own fonts and styles.

use std::path::PathBuf;

use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app_kind::AppKind;
use crate::i18n::{Lang, t, tf};
use crate::rail;
use crate::settings::Recent;
use crate::theme::{Palette, SEMIBOLD};

pub enum HomeAction {
    New(AppKind),
    OpenDialog,
    OpenRecent(PathBuf, AppKind),
    RemoveRecent(PathBuf),
    ClearRecent,
    RevealRecent(PathBuf),
}

#[derive(Default)]
pub struct HomeState {
}

fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

fn greeting(lang: Lang) -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=11 => t(lang, "Good morning"),
        12..=17 => t(lang, "Good afternoon"),
        _ => t(lang, "Good evening"),
    }
}

fn ago(lang: Lang, then: i64) -> String {
    let secs = (chrono::Utc::now().timestamp() - then).max(0);
    let n = |v: i64| v.to_string();
    match secs {
        0..60 => t(lang, "just now").to_string(),
        60..3600 => tf(lang, "{n} min ago", &[("n", &n(secs / 60))]),
        3600..86400 => tf(lang, "{n} h ago", &[("n", &n(secs / 3600))]),
        86400..172800 => t(lang, "yesterday").to_string(),
        _ => tf(lang, "{n} days ago", &[("n", &n(secs / 86400))]),
    }
}

impl HomeState {
    pub fn logo(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        crate::rail::logo(ctx)
    }

    pub fn show(&mut self, ui: &mut Ui, lang: Lang, recent: &[Recent]) -> Option<HomeAction> {
        let p = Palette::current(ui.ctx());
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, p.bg);
        let mut action = None;
        let width = (rect.width() - 96.0).clamp(320.0, 1040.0);
        egui::ScrollArea::vertical().id_salt("themisio-home").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(36.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    // Header: logo, name, slogan.
                    ui.horizontal(|ui| {
                        if let Some(logo) = self.logo(ui.ctx()) {
                            ui.add(egui::Image::from_texture(&logo).fit_to_exact_size(vec2(60.0, 60.0)));
                        }
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new("Themisio").font(semibold(30.0)).color(p.text));
                            ui.label(RichText::new(format!("Themisio – {}", t(lang, "Make your documents glow."))).size(15.0).color(p.muted));
                        });
                    });
                    ui.add_space(30.0);
                    ui.label(RichText::new(greeting(lang)).font(semibold(20.0)).color(p.text));
                    ui.add_space(14.0);
                    ui.label(RichText::new(t(lang, "New")).size(13.0).color(p.muted));
                    ui.add_space(4.0);
                    // Four "new" cards, wrapping on narrow windows.
                    let cards = [
                        (AppKind::Word, t(lang, "Blank document")),
                        (AppKind::Deck, t(lang, "Blank presentation")),
                        (AppKind::Grid, t(lang, "Blank workbook")),
                        (AppKind::Pdf, t(lang, "Blank PDF")),
                    ];
                    let gap = 14.0;
                    let per_row = if width > 760.0 { 4 } else { 2 };
                    let card_w = (width - gap * (per_row as f32 - 1.0)) / per_row as f32;
                    for row in cards.chunks(per_row) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (kind, title) in row {
                                if new_card(ui, &p, *kind, title, rail::description(lang, *kind), card_w) {
                                    action = Some(HomeAction::New(*kind));
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let open = egui::Button::new(RichText::new(t(lang, "Open…")).size(14.0).color(p.accent_text))
                            .fill(p.accent)
                            .corner_radius(CornerRadius::same(10))
                            .min_size(vec2(120.0, 36.0));
                        if ui.add(open).clicked() {
                            action = Some(HomeAction::OpenDialog);
                        }
                        ui.add_space(10.0);
                        ui.label(RichText::new(t(lang, "Drop files anywhere to open them.")).size(13.0).color(p.muted));
                    });
                    ui.add_space(30.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t(lang, "Recent")).font(semibold(17.0)).color(p.text));
                        if !recent.is_empty() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new(t(lang, "Clear list")).size(12.5).color(p.muted)).frame(false)).clicked() {
                                    action = Some(HomeAction::ClearRecent);
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                    if recent.is_empty() {
                        ui.label(
                            RichText::new(t(lang, "No recent files yet. Open or create something to get started.")).size(13.5).color(p.muted),
                        );
                    }
                    for r in recent {
                        if let Some(a) = recent_row(ui, &p, lang, r, width) {
                            action = Some(a);
                        }
                    }
                    ui.add_space(24.0);
                    ui.label(RichText::new(t(lang, "Keyboard: Ctrl+Alt+1…5 switch apps, Ctrl+Alt+O opens a file.")).size(12.0).color(p.muted));
                    ui.add_space(36.0);
                });
            });
        });
        action
    }
}

fn new_card(ui: &mut Ui, p: &Palette, kind: AppKind, title: &str, desc: &str, width: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 142.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("lift"), response.hovered(), 0.16);
    let rect = rect.translate(vec2(0.0, -3.0 * hover));
    let painter = ui.painter();
    // Soft shadow that grows as the card lifts.
    for i in 0..3 {
        let spread = 2.0 + i as f32 * 3.0 + hover * 4.0;
        let alpha = ((10.0 - i as f32 * 3.0) * (0.6 + hover)) as u8;
        painter.rect_filled(rect.expand(spread * 0.5).translate(vec2(0.0, 2.0 + hover * 2.0)), CornerRadius::same(16), Color32::from_black_alpha(alpha));
    }
    painter.rect_filled(rect, CornerRadius::same(14), p.surface);
    let border = if hover > 0.0 { kind.accent().gamma_multiply(0.4 + 0.6 * hover) } else { p.border };
    painter.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, border), StrokeKind::Inside);
    // A page preview with the app's colour.
    let preview = Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(44.0, 44.0));
    rail::draw_app_icon(painter, preview.center(), kind, Color32::WHITE, true);
    painter.text(rect.min + vec2(16.0, 76.0), Align2::LEFT_TOP, title, semibold(15.0), p.text);
    let galley = painter.layout(desc.to_string(), FontId::proportional(12.0), p.muted, width - 32.0);
    painter.galley(rect.min + vec2(16.0, 98.0), galley, p.muted);
    let plus = pos2(rect.right() - 26.0, rect.top() + 28.0);
    let plus_color = kind.accent().gamma_multiply(0.5 + 0.5 * hover);
    painter.circle_filled(plus, 12.0, kind.accent().gamma_multiply(0.12 + 0.1 * hover));
    painter.line_segment([plus - vec2(5.0, 0.0), plus + vec2(5.0, 0.0)], Stroke::new(1.8, plus_color));
    painter.line_segment([plus - vec2(0.0, 5.0), plus + vec2(0.0, 5.0)], Stroke::new(1.8, plus_color));
    response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn recent_row(ui: &mut Ui, p: &Palette, lang: Lang, r: &Recent, width: f32) -> Option<HomeAction> {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 48.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    let painter = ui.painter();
    if hover > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(10), p.surface_hover.gamma_multiply(hover));
    }
    rail::draw_app_icon(painter, pos2(rect.left() + 24.0, rect.center().y), r.app, Color32::WHITE, true);
    let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = r.path.parent().map(|d| d.display().to_string()).unwrap_or_default();
    let exists = r.path.exists();
    painter.text(pos2(rect.left() + 50.0, rect.top() + 8.0), Align2::LEFT_TOP, &name, FontId::proportional(14.0), if exists { p.text } else { p.muted });
    painter.text(pos2(rect.left() + 50.0, rect.top() + 27.0), Align2::LEFT_TOP, &folder, FontId::proportional(11.5), p.muted);
    painter.text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, ago(lang, r.opened), FontId::proportional(12.0), p.muted);
    let mut action = None;
    let response = if exists { response } else { response.on_hover_text(t(lang, "The file no longer exists.")) };
    if response.clicked() && exists {
        action = Some(HomeAction::OpenRecent(r.path.clone(), r.app));
    }
    response.context_menu(|ui| {
        if ui.button(t(lang, "Open")).clicked() {
            action = Some(HomeAction::OpenRecent(r.path.clone(), r.app));
            ui.close();
        }
        if ui.button(t(lang, "Open file location")).clicked() {
            action = Some(HomeAction::RevealRecent(r.path.clone()));
            ui.close();
        }
        if ui.button(t(lang, "Remove from list")).clicked() {
            action = Some(HomeAction::RemoveRecent(r.path.clone()));
            ui.close();
        }
    });
    action
}
