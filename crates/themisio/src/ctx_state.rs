//! Each app styles the whole egui context: its fonts (with named families its widgets rely on),
//! light and dark styles, theme, zoom, keyboard options. They all share Themisio's one context, so
//! the shell saves an app's context state when it leaves the app and puts it back on return.
//!
//! Fonts set with `Context::set_fonts` take effect at the next pass, so a switch happens at the
//! end of a pass: the app being left has drawn with its own fonts, and the next pass draws the
//! app being shown with its own.

use egui::{Context, FontDefinitions, Options};

#[derive(Clone)]
pub struct Snapshot {
    options: Options,
    fonts: FontDefinitions,
}

impl Snapshot {
    /// Everything an app may have set on the context, including fonts queued this pass.
    pub fn capture(ctx: &Context) -> Self {
        Self { options: ctx.options(Clone::clone), fonts: ctx.themisio_font_definitions() }
    }

    /// Put the state back. Fonts apply from the next pass (or now, before any text is laid out).
    pub fn restore(&self, ctx: &Context) {
        let options = self.options.clone();
        ctx.options_mut(move |o| *o = options);
        ctx.set_fonts(self.fonts.clone());
    }
}
