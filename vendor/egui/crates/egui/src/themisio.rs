//! Themisio additions to egui (see `vendor/egui/THEMISIO.md`). Both are inert until a host turns
//! them on, so every other user of this fork behaves exactly as before.
//!
//! - An interface-string translator: while [`set_translating`] is on, plain [`crate::WidgetText`]
//!   and [`crate::RichText`] labels and [`crate::Painter::text`]/`layout` strings are offered to
//!   the translator, which may return a replacement. Text-edit buffers and prebuilt
//!   [`crate::text::LayoutJob`]s never pass through it, so document text is never changed.
//! - Tunable wheel smoothing (egui's `WheelState::after_events` constant, which had a TODO to
//!   parameterise it).

use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

/// Returns a replacement for an interface string, or `None` to keep it.
pub type Translator = dyn Fn(&str) -> Option<String> + Send + Sync;

static TRANSLATOR: RwLock<Option<Arc<Translator>>> = RwLock::new(None);
static TRANSLATING: AtomicBool = AtomicBool::new(false);

/// Install (or remove) the translator. It runs only while [`set_translating`] is on.
pub fn set_translator(translator: Option<Arc<Translator>>) {
    if let Ok(mut slot) = TRANSLATOR.write() {
        *slot = translator;
    }
}

/// Turn translation on or off; returns the previous state, so callers can scope it.
pub fn set_translating(on: bool) -> bool {
    TRANSLATING.swap(on, Ordering::Relaxed)
}

/// Whether interface strings are being translated now.
pub fn is_translating() -> bool {
    TRANSLATING.load(Ordering::Relaxed)
}

/// The translation of `text`, when translation is on and the translator has one.
pub fn translate(text: &str) -> Option<String> {
    if !is_translating() || text.is_empty() {
        return None;
    }
    let slot = TRANSLATOR.read().ok()?;
    let translator = slot.as_ref()?;
    translator(text)
}

pub(crate) fn translate_owned(text: String) -> String {
    translate(&text).unwrap_or(text)
}

// Wheel smoothing: reach `REACH` (per mille) of a wheel notch's distance in `SECONDS` (ms).
static WHEEL_REACH_PER_MILLE: AtomicU32 = AtomicU32::new(900);
static WHEEL_MILLIS: AtomicU32 = AtomicU32::new(100);

/// How mouse-wheel notches glide: the fraction of the distance covered (0.5..0.999) within
/// `seconds` (0.01..1.0). egui's default is `(0.90, 0.1)`.
pub fn set_wheel_smoothing(reach: f32, seconds: f32) {
    let reach = if reach.is_finite() { reach.clamp(0.5, 0.999) } else { 0.9 };
    let seconds = if seconds.is_finite() { seconds.clamp(0.01, 1.0) } else { 0.1 };
    WHEEL_REACH_PER_MILLE.store((reach * 1000.0).round() as u32, Ordering::Relaxed);
    WHEEL_MILLIS.store((seconds * 1000.0).round() as u32, Ordering::Relaxed);
}

/// `(reach, seconds)` for [`emath::exponential_smooth_factor`].
pub(crate) fn wheel_smoothing() -> (f32, f32) {
    (
        WHEEL_REACH_PER_MILLE.load(Ordering::Relaxed) as f32 / 1000.0,
        WHEEL_MILLIS.load(Ordering::Relaxed) as f32 / 1000.0,
    )
}

// Root-window close interception: a host that shows several apps in one window decides itself
// when the window really closes. While on, `ViewportCommand::Close` sent to the root viewport is
// recorded instead of queued; the host reads it with [`take_root_close_request`].
static INTERCEPT_ROOT_CLOSE: AtomicBool = AtomicBool::new(false);
static ROOT_CLOSE_ASKED: AtomicBool = AtomicBool::new(false);

/// Turn root-close interception on or off; returns the previous state.
pub fn set_root_close_intercept(on: bool) -> bool {
    INTERCEPT_ROOT_CLOSE.swap(on, Ordering::Relaxed)
}

/// Whether something asked to close the root window since the last call.
pub fn take_root_close_request() -> bool {
    ROOT_CLOSE_ASKED.swap(false, Ordering::Relaxed)
}

/// `true` when the close was recorded and must not be queued.
pub(crate) fn intercept_root_close() -> bool {
    if INTERCEPT_ROOT_CLOSE.load(Ordering::Relaxed) {
        ROOT_CLOSE_ASKED.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

static ROOT_CANCEL_SEEN: AtomicBool = AtomicBool::new(false);

/// Whether `ViewportCommand::CancelClose` was sent to the root viewport since the last call:
/// an app answering a close request with a question of its own (unsaved changes).
pub fn take_root_cancel_close() -> bool {
    ROOT_CANCEL_SEEN.swap(false, Ordering::Relaxed)
}

pub(crate) fn note_root_cancel_close() {
    ROOT_CANCEL_SEEN.store(true, Ordering::Relaxed);
}

/// Rewrites the root window's title as apps set it (`"Report - WordCraft"` → `"Report - Themisio"`).
pub type TitleFilter = dyn Fn(&str) -> String + Send + Sync;

static TITLE_FILTER: RwLock<Option<Arc<TitleFilter>>> = RwLock::new(None);

/// Install (or remove) the root-window title filter.
pub fn set_title_filter(filter: Option<Arc<TitleFilter>>) {
    if let Ok(mut slot) = TITLE_FILTER.write() {
        *slot = filter;
    }
}

pub(crate) fn filter_title(title: String) -> String {
    match TITLE_FILTER.read().ok().and_then(|slot| slot.clone()) {
        Some(filter) => filter(&title),
        None => title,
    }
}

// Motion: the small animations a host may add across every app it shows (all off by default).
// - widget hover: a widget's background and stroke ease between its idle and hovered look,
//   instead of switching in one frame ([`crate::style::Widgets::style`]);
// - popups, menus and tooltips glide a few points into place as they fade in;
// - [`switch_progress`]: a toolbar whose contents change (a ribbon tab, a tool panel) eases in.
static MOTION: AtomicBool = AtomicBool::new(false);

/// Turn Themisio's motion on or off (follow the system's "Animation effects").
pub fn set_motion(on: bool) {
    MOTION.store(on, Ordering::Relaxed);
}

/// Whether Themisio's motion is on.
pub fn motion() -> bool {
    MOTION.load(Ordering::Relaxed)
}

const HOVER_STEPS: u32 = 10;
const HOVER_CACHE_CAP: usize = 4096;

type BlendKey = (u64, u64, u32);
static BLENDS: RwLock<Option<std::collections::HashMap<BlendKey, &'static crate::style::WidgetVisuals>>> = RwLock::new(None);

fn visuals_key(v: &crate::style::WidgetVisuals) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.bg_fill.hash(&mut h);
    v.weak_bg_fill.hash(&mut h);
    v.bg_stroke.width.to_bits().hash(&mut h);
    v.bg_stroke.color.hash(&mut h);
    v.fg_stroke.width.to_bits().hash(&mut h);
    v.fg_stroke.color.hash(&mut h);
    v.corner_radius.hash(&mut h);
    v.expansion.to_bits().hash(&mut h);
    h.finish()
}

fn lerp_visuals(a: &crate::style::WidgetVisuals, b: &crate::style::WidgetVisuals, t: f32) -> crate::style::WidgetVisuals {
    let stroke = |x: epaint::Stroke, y: epaint::Stroke| epaint::Stroke::new(x.width + (y.width - x.width) * t, x.color.lerp_to_gamma(y.color, t));
    crate::style::WidgetVisuals {
        bg_fill: a.bg_fill.lerp_to_gamma(b.bg_fill, t),
        weak_bg_fill: a.weak_bg_fill.lerp_to_gamma(b.weak_bg_fill, t),
        bg_stroke: stroke(a.bg_stroke, b.bg_stroke),
        corner_radius: if t < 0.5 { a.corner_radius } else { b.corner_radius },
        fg_stroke: stroke(a.fg_stroke, b.fg_stroke),
        expansion: a.expansion + (b.expansion - a.expansion) * t,
    }
}

/// The look `t` of the way from `idle` to `hovered`, in a few steps (each step is made once and
/// kept: a bounded set, as styles are few).
pub(crate) fn blend_visuals<'a>(idle: &'a crate::style::WidgetVisuals, hovered: &'a crate::style::WidgetVisuals, t: f32) -> &'a crate::style::WidgetVisuals {
    let step = (t.clamp(0.0, 1.0) * HOVER_STEPS as f32).round() as u32;
    if step == 0 || idle == hovered {
        return idle;
    }
    if step >= HOVER_STEPS {
        return hovered;
    }
    let key = (visuals_key(idle), visuals_key(hovered), step);
    if let Ok(map) = BLENDS.read()
        && let Some(found) = map.as_ref().and_then(|m| m.get(&key))
    {
        return found;
    }
    let Ok(mut map) = BLENDS.write() else { return if t < 0.5 { idle } else { hovered } };
    let map = map.get_or_insert_with(Default::default);
    if let Some(found) = map.get(&key) {
        return found;
    }
    if map.len() >= HOVER_CACHE_CAP {
        return if t < 0.5 { idle } else { hovered };
    }
    let made: &'static crate::style::WidgetVisuals = Box::leak(Box::new(lerp_visuals(idle, hovered, step as f32 / HOVER_STEPS as f32)));
    map.insert(key, made);
    made
}

/// How far (0 → 1, eased) a toolbar is into showing what `key` names, since `key` last changed
/// for `id`. Always 1 with motion off. While below 1 it asks for another frame.
pub fn switch_progress(ctx: &crate::Context, id: crate::Id, key: impl std::hash::Hash) -> f32 {
    if !motion() {
        return 1.0;
    }
    let key = {
        use std::hash::Hasher as _;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        h.finish()
    };
    let now = ctx.input(|i| i.time);
    let slot = id.with("__themisio_switch");
    let (last, since) = ctx.data(|d| d.get_temp::<(u64, f64)>(slot)).unwrap_or((key, f64::NEG_INFINITY));
    let since = if last == key {
        since
    } else {
        ctx.data_mut(|d| d.insert_temp(slot, (key, now)));
        now
    };
    if since == f64::NEG_INFINITY {
        ctx.data_mut(|d| d.insert_temp(slot, (key, f64::NEG_INFINITY)));
        return 1.0;
    }
    let p = ((now - since) / 0.2).clamp(0.0, 1.0) as f32;
    if p < 1.0 {
        ctx.request_repaint();
    }
    emath::easing::cubic_out(p)
}

/// Ease a toolbar's contents in when `key` changes: fades `ui` and returns the eased progress,
/// so the caller can also slide the contents by `(1 - p) * distance`.
pub fn toolbar_transition(ui: &mut crate::Ui, key: impl std::hash::Hash) -> f32 {
    let p = switch_progress(ui.ctx(), ui.scope_id(), key);
    if p < 1.0 {
        ui.multiply_opacity(0.15 + 0.85 * p);
    }
    p
}

// Context-menu extras: a host adds its own items (Translate, Check writing…) at the end of the
// apps' right-click menus. The apps call `context_menu_extras` where their menus end.
type MenuHook = std::sync::Arc<dyn Fn(&mut crate::Ui) -> bool + Send + Sync>;
static MENU_HOOK: RwLock<Option<MenuHook>> = RwLock::new(None);

/// Set (or clear) the items added to the apps' right-click menus. The hook returns whether one
/// of its items was clicked (it closes the menu itself).
pub fn set_context_menu_hook(hook: Option<MenuHook>) {
    if let Ok(mut h) = MENU_HOOK.write() {
        *h = hook;
    }
}

/// Draw the host's extra menu items, if any; true when one was clicked.
pub fn context_menu_extras(ui: &mut crate::Ui) -> bool {
    let hook = MENU_HOOK.read().ok().and_then(|h| h.clone());
    hook.is_some_and(|h| h(ui))
}
