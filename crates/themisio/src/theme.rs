//! Themisio's look: the colours of the logo (deep navy tile, the ruler's gold, the four
//! coloured strips), and the fonts and styles of the shell's own pages (Home, dialogs). The apps keep
//! their own look.

use egui::{Color32, Context, FontData, FontDefinitions, FontFamily, Theme, Visuals};
use std::sync::Arc;

/// Navy of the logo tile (top).
pub const BRAND: Color32 = Color32::from_rgb(0x06, 0x26, 0x4D);
/// The tile's deeper navy (bottom), for the app bar's gradient and edge.
pub const BRAND_DEEP: Color32 = Color32::from_rgb(0x02, 0x12, 0x2C);
/// The ruler's gold: selection, the bar's hint line, accents on navy.
pub const GOLD: Color32 = Color32::from_rgb(0xF6, 0xC2, 0x4A);
/// Text and icons on navy.
pub const CREAM: Color32 = Color32::from_rgb(0xF3, 0xF6, 0xFB);

/// The family name of Themisio's semibold face (Home headings). Only the shell's own pages use it:
/// the apps' font definitions don't have it.
pub const SEMIBOLD: &str = "themisio-semibold";

/// Colours of the shell's pages in the current theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
}

impl Palette {
    pub fn of(dark: bool) -> Self {
        if dark {
            Self {
                bg: Color32::from_rgb(0x0C, 0x13, 0x20),
                surface: Color32::from_rgb(0x14, 0x1D, 0x2E),
                surface_hover: Color32::from_rgb(0x1C, 0x28, 0x3D),
                border: Color32::from_rgb(0x26, 0x33, 0x4A),
                text: Color32::from_rgb(0xEA, 0xEF, 0xF7),
                muted: Color32::from_rgb(0x96, 0xA4, 0xBA),
                accent: GOLD,
                accent_text: BRAND_DEEP,
            }
        } else {
            Self {
                bg: Color32::from_rgb(0xF4, 0xF6, 0xFA),
                surface: Color32::WHITE,
                surface_hover: Color32::from_rgb(0xEC, 0xF0, 0xF6),
                border: Color32::from_rgb(0xDC, 0xE2, 0xEC),
                text: Color32::from_rgb(0x10, 0x1A, 0x2C),
                muted: Color32::from_rgb(0x58, 0x66, 0x7E),
                accent: Color32::from_rgb(0x1D, 0x4F, 0x91),
                accent_text: Color32::WHITE,
            }
        }
    }

    pub fn current(ctx: &Context) -> Self {
        Self::of(ctx.global_style().visuals.dark_mode)
    }
}

/// Fonts for the shell's pages: Inter (OFL-1.1, from PdfCraft's assets; it covers Vietnamese),
/// then egui's defaults for symbols and emoji.
pub fn font_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        "themisio-inter".into(),
        Arc::new(FontData::from_static(include_bytes!("../../../upstream/pdfcraft/assets/fonts/Inter-Regular.ttf"))),
    );
    defs.font_data.insert(
        SEMIBOLD.into(),
        Arc::new(FontData::from_static(include_bytes!("../../../upstream/pdfcraft/assets/fonts/Inter-SemiBold.ttf"))),
    );
    if let Some(list) = defs.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, "themisio-inter".into());
    }
    defs.families.insert(FontFamily::Name(UI_FAMILY.into()), ui_family_list());
    let mut semibold = vec![SEMIBOLD.to_string()];
    semibold.extend(defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    defs.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    defs
}

/// The shell's styles: egui's, in Themisio's colours, with softer corners.
pub fn apply(ctx: &Context) {
    for theme in [Theme::Light, Theme::Dark] {
        let p = Palette::of(theme == Theme::Dark);
        let mut v = if theme == Theme::Dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = p.bg;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.surface;
        v.faint_bg_color = p.surface_hover;
        v.window_stroke = egui::Stroke::new(1.0, p.border);
        v.selection.bg_fill = p.accent.gamma_multiply(0.35);
        v.selection.stroke = egui::Stroke::new(1.0, p.accent);
        v.hyperlink_color = p.accent;
        v.window_corner_radius = egui::CornerRadius::same(12);
        v.menu_corner_radius = egui::CornerRadius::same(8);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
            w.corner_radius = egui::CornerRadius::same(8);
        }
        v.widgets.inactive.weak_bg_fill = p.surface_hover;
        v.widgets.hovered.weak_bg_fill = p.surface_hover;
        ctx.set_visuals_of(theme, v);
    }
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.animation_time = 0.18;
    });
    ctx.set_fonts(font_definitions());
}

/// The name of Inter in the apps' font definitions (added by [`ensure_fonts`]).
const INTER: &str = "themisio-inter";

/// Themisio's own interface family (Inter, then egui's symbol faces). It is added to every app's
/// fonts too, so the shell's dialogs, tab bar and app bar look the same over any app.
pub const UI_FAMILY: &str = "themisio-ui";

fn ui_family_list() -> Vec<String> {
    let mut list = vec![INTER.to_string()];
    list.extend(FontDefinitions::default().families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    list
}

/// The family for the shell's text: [`UI_FAMILY`] once the current fonts have it (always from
/// the pass after [`ensure_fonts`]), else the proportional one.
pub fn ui_font(ctx: &Context, size: f32) -> egui::FontId {
    let name = FontFamily::Name(UI_FAMILY.into());
    let has = ctx.fonts(|f| f.families().contains(&name));
    egui::FontId::new(size, if has { name } else { FontFamily::Proportional })
}

/// Make sure the context's fonts have Themisio's interface family ([`UI_FAMILY`]) and, in
/// Vietnamese (`vietnamese`), can draw Vietnamese anywhere: Inter (which covers it) is added to
/// every family as a fallback, so translated strings never show missing-glyph boxes, whatever
/// app's fonts are in effect. `primary`: Inter first in the proportional family (DeckCraft in
/// Vietnamese, whose own interface faces lack Vietnamese letters). Only ever adds fonts, so no
/// family an app relies on goes away. Applies from the next pass.
pub fn ensure_fonts(ctx: &Context, vietnamese: bool, primary: bool) {
    let mut defs = ctx.themisio_font_definitions();
    let mut changed = false;
    if !defs.font_data.contains_key(INTER) {
        defs.font_data.insert(
            INTER.into(),
            Arc::new(FontData::from_static(include_bytes!("../../../upstream/pdfcraft/assets/fonts/Inter-Regular.ttf"))),
        );
        changed = true;
    }
    let ui = FontFamily::Name(UI_FAMILY.into());
    if !defs.families.contains_key(&ui) {
        let base = FontDefinitions::default();
        let list = ui_family_list();
        for name in &list {
            if !defs.font_data.contains_key(name)
                && let Some(data) = base.font_data.get(name)
            {
                defs.font_data.insert(name.clone(), data.clone());
            }
        }
        defs.families.insert(ui.clone(), list);
        changed = true;
    }
    if vietnamese {
        for (family, list) in defs.families.iter_mut() {
            if *family == ui {
                continue;
            }
            let at = list.iter().position(|n| n == INTER);
            let want_first = primary && *family == FontFamily::Proportional;
            match (at, want_first) {
                (None, true) => {
                    list.insert(0, INTER.into());
                    changed = true;
                }
                (None, false) => {
                    list.push(INTER.into());
                    changed = true;
                }
                (Some(i), true) if i != 0 => {
                    let name = list.remove(i);
                    list.insert(0, name);
                    changed = true;
                }
                (Some(0), false) if list.len() > 1 && *family != FontFamily::Name(SEMIBOLD.into()) => {
                    let name = list.remove(0);
                    list.push(name);
                    changed = true;
                }
                _ => {}
            }
        }
    }
    if changed {
        ctx.set_fonts(defs);
    }
}

/// The style of the shell's dialogs, the same over every app: Themisio's colours, the interface
/// family at compact sizes, and roomy but not oversized controls.
pub fn dialog_style(ctx: &Context, dark: bool) -> egui::Style {
    let p = Palette::of(dark);
    let mut style = egui::Style::default();
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.surface_hover;
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.30 } else { 0.18 });
    v.selection.stroke = egui::Stroke::new(1.0, p.text);
    v.hyperlink_color = p.accent;
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(6);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    v.widgets.inactive.weak_bg_fill = p.surface_hover;
    v.widgets.inactive.bg_fill = p.surface_hover;
    v.widgets.hovered.weak_bg_fill = p.border;
    v.widgets.hovered.bg_fill = p.border;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.border;
    style.visuals = v;
    let family = ui_font(ctx, 13.0).family;
    use egui::TextStyle;
    style.text_styles = [
        (TextStyle::Small, 11.5),
        (TextStyle::Body, 13.0),
        (TextStyle::Button, 13.0),
        (TextStyle::Monospace, 12.5),
        (TextStyle::Heading, 17.0),
    ]
    .into_iter()
    .map(|(s, size)| {
        let f = if s == TextStyle::Monospace { FontFamily::Monospace } else { family.clone() };
        (s, egui::FontId::new(size, f))
    })
    .collect();
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size = egui::vec2(24.0, 26.0);
    style.spacing.window_margin = egui::Margin::same(18);
    style.animation_time = 0.14;
    style
}
