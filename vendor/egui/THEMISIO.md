# Themisio changes to storytold/egui 6d518fc

This is storytold/egui at `6d518fc799681571963842f95b55a6b68431b15a` (the fork WordCraft,
DeckCraft, GridCraft and PdfCraft all pin) with six small, opt-in additions, all in
`crates/egui/src/themisio.rs` and its call sites. Nothing changes unless a host turns them on.

1. **Interface-string translator** (`themisio::set_translator`, `themisio::set_translating`):
   `WidgetText::Text`, `RichText` and `Painter::{text, layout, layout_no_wrap}` strings are offered
   to the translator while translating is on. Themisio turns it on only while DeckCraft (which has no
   string catalog of its own yet) draws, to show its interface in Vietnamese. Text-edit buffers and
   prebuilt `LayoutJob`s never pass through it.
2. **Wheel smoothing** (`themisio::set_wheel_smoothing`): the constant in
   `WheelState::after_events` (`exponential_smooth_factor(0.90, 0.1, dt)`, marked
   `TODO(emilk): parameterize`) becomes a setting. Default unchanged.
3. **Root-close interception** (`themisio::set_root_close_intercept`,
   `themisio::take_root_close_request`): `Context::send_viewport_cmd_to(ROOT, Close)` is recorded
   instead of queued, so the host decides when the shared window closes (one app's Exit must not
   discard another app's unsaved work).

`THEMISIO.patch` beside this file is the full change against the fork.

   `themisio::take_root_cancel_close` reports whether an app answered a close request with
   `CancelClose` (it is asking about unsaved work), so the host knows to wait for it.
4. **`Context::themisio_font_definitions`**: the font definitions in effect from the next pass on,
   including queued `set_fonts`/`add_font` changes, so a host can save and restore each app's
   fonts when it switches between apps sharing one context.
5. **Root-window title filter** (`themisio::set_title_filter`): `ViewportCommand::Title` sent to the
   root viewport passes through the filter, so "Report - WordCraft" reads "Report - Themisio".
6. **Motion** (`themisio::set_motion`, `themisio::switch_progress`, `themisio::toolbar_transition`):
   while on, `Widgets::style` eases a widget between its idle and hovered look (a bounded cache
   of blended `WidgetVisuals`, so the `&WidgetVisuals` API is unchanged); popups, menus and
   tooltips (`Order::Foreground`/`Tooltip` areas that fade in) glide 6 points into place; and a
   toolbar can ease its contents in when its tab changes (used by Themisio's patches to the
   four apps' ribbons and PdfCraft's tool panel). Themisio follows Windows' Animation effects.
