//! The Themisio window: the rail, Home, and the app being shown, with animated switches, file
//! routing, and one close flow for all four apps.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::{Color32, CornerRadius, Id, Rect, RichText, Sense, UiBuilder, ViewportCommand, ViewportId, pos2, vec2};

use crate::app_kind::{AppKind, Page};
use crate::ctx_state::Snapshot;
use crate::home::{HomeAction, HomeState};
use crate::hosts::Hosts;
use crate::i18n::{Lang, LangPref, t, tf};
use crate::motion::{MotionState, ease_in, ease_out};
use crate::rail::{self, RailAction};
use crate::route::{self, Intent, Target};
use crate::settings::{Motion, Presentation, RailMode, Renderer, Settings, Smoothness, ThemePref};
use crate::theme::Palette;

/// How the window was started.
#[derive(Default)]
pub struct Launch {
    pub files: Vec<PathBuf>,
    /// `--app <id>`: show this app.
    pub app: Option<AppKind>,
    /// `--new <id>`: a new blank document in this app.
    pub new: Option<AppKind>,
}

/// Something waiting for its app to be shown.
enum Queued {
    Open(PathBuf, Intent),
    New,
    StartScreen,
}

/// Cover the old page, swap, reveal the new one.
struct Switch {
    to: Page,
    started: f64,
}

/// Seconds to cover the page being left, and to reveal the page being shown.
const COVER: f64 = 0.085;
const REVEAL: f64 = 0.22;
/// A switch asked for at start-up is already fully covered on the first pass (no Home flash).
const STARTUP: f64 = -10.0;

/// After the close flow: close the window, or only the app(s) and go Home.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AfterClose {
    Window,
    Home,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CloseStage {
    /// Show the next app in the queue.
    Next,
    /// Give the app a close request in the next pass.
    Inject(AppKind),
    /// The app saw the request this pass; see whether it asks about unsaved work.
    Judge(AppKind),
    /// It asked; wait until it closes (or the user cancels).
    Waiting(AppKind),
}

struct Closing {
    queue: VecDeque<AppKind>,
    stage: CloseStage,
    after: AfterClose,
}

/// "How do you want to open this?" for files several apps open.
struct Chooser {
    path: PathBuf,
    targets: Vec<Target>,
    picked: usize,
    remember: bool,
    rest: Vec<PathBuf>,
}

#[derive(PartialEq, Eq)]
enum Dialog {
    Settings,
    About,
}

pub struct Themisio {
    hosts: Hosts,
    active: Page,
    switch: Option<Switch>,
    /// When the current page was swapped in (for the reveal).
    revealed: f64,
    snapshots: HashMap<Page, Snapshot>,
    /// egui's untouched state: where an app that starts sets itself up from.
    pristine: Snapshot,
    settings: Settings,
    lang: Lang,
    queue: HashMap<AppKind, Vec<Queued>>,
    chooser: Option<Chooser>,
    dialog: Option<Dialog>,
    closing: Option<Closing>,
    toast: Option<(String, f64)>,
    motion: MotionState,
    home: HomeState,
    open_dialog: Option<Receiver<Vec<PathBuf>>>,
    /// An app being started (shown on the cover).
    starting: Option<AppKind>,
    /// Pass counter, for "first pass" set-up.
    passes: u64,
    /// Files from later launches (`instance`).
    instance: Option<Receiver<Vec<PathBuf>>>,
    /// Every app is done: let the window close.
    closing_window: bool,
    /// The app bar is out (auto-hide), and when the pointer was last near it.
    rail_open: bool,
    rail_seen: f64,
    /// The light/dark choice last given to the apps.
    applied_dark: Option<bool>,
    trim: crate::memory::Trim,
}

/// Height of the document tab bar.
const TAB_BAR: f32 = 34.0;

/// What the user did on the tab bar.
enum TabAction {
    Activate(u64),
    Close(u64),
    New,
}

/// Files from later launches, and where to put the context so they can wake the window.
pub type InstanceServer = (Receiver<Vec<PathBuf>>, std::sync::Arc<std::sync::Mutex<Option<egui::Context>>>);

impl Themisio {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch, server: Option<InstanceServer>) -> Self {
        let ctx = &cc.egui_ctx;
        let instance = server.map(|(rx, slot)| {
            if let Ok(mut s) = slot.lock() {
                *s = Some(ctx.clone());
            }
            rx
        });
        let pristine = Snapshot::capture(ctx);
        crate::theme::apply(ctx);
        let settings = Settings::load();
        let lang = settings.language.effective();
        // One window shared by four apps: Themisio decides when it closes.
        egui::themisio::set_root_close_intercept(true);
        egui::themisio::set_translator(Some(std::sync::Arc::new(crate::deck_i18n::translate)));
        egui::themisio::set_title_filter(Some(std::sync::Arc::new(crate::window_title)));
        let (reach, secs) = settings.smoothness.wheel();
        egui::themisio::set_wheel_smoothing(reach, secs);
        let mut shell = Self {
            hosts: Hosts::default(),
            active: Page::Home,
            switch: None,
            revealed: f64::NEG_INFINITY,
            snapshots: HashMap::new(),
            pristine,
            settings,
            lang,
            queue: HashMap::new(),
            chooser: None,
            dialog: None,
            closing: None,
            toast: None,
            motion: MotionState::default(),
            home: HomeState::default(),
            open_dialog: None,
            starting: None,
            passes: 0,
            instance,
            closing_window: false,
            rail_open: false,
            rail_seen: f64::NEG_INFINITY,
            applied_dark: None,
            trim: crate::memory::Trim::default(),
        };
        shell.snapshots.insert(Page::Home, Snapshot::capture(ctx));
        if let Some(kind) = launch.new {
            shell.enqueue(kind, Queued::New);
            shell.go(Page::App(kind), STARTUP);
        }
        if !launch.files.is_empty() {
            shell.open_paths(launch.files, STARTUP);
        } else if let Some(kind) = launch.app
            && launch.new.is_none()
        {
            shell.go(Page::App(kind), STARTUP);
        }
        shell
    }

    fn effective_motion(&mut self, now: f64) -> bool {
        self.motion.animate(self.settings.motion, now)
    }

    /// The page on screen, or the one being switched to.
    fn target_page(&self) -> Page {
        self.switch.as_ref().map_or(self.active, |s| s.to)
    }

    /// Show `page` (animated unless motion is off).
    fn go(&mut self, page: Page, now: f64) {
        if self.target_page() == page {
            return;
        }
        if let Some(kind) = page.app()
            && !self.hosts.is_loaded(kind)
        {
            self.starting = Some(kind);
        }
        self.switch = Some(Switch { to: page, started: now });
    }

    fn enqueue(&mut self, kind: AppKind, q: Queued) {
        self.queue.entry(kind).or_default().push(q);
    }

    /// Open files, each in its app; files several apps open ask first.
    fn open_paths(&mut self, paths: Vec<PathBuf>, now: f64) {
        let mut paths = paths.into_iter();
        while let Some(path) = paths.next() {
            let targets = route::targets(&path);
            let ext = route::extension(&path);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if targets.is_empty() {
                self.toast(tf(self.lang, "Themisio can't open {name}.", &[("name", &name)]), now);
                continue;
            }
            let remembered = self.settings.open_with.get(&ext).and_then(|app| targets.iter().find(|t| t.app == *app).copied());
            let chosen = if targets.len() == 1 || matches!(ext.as_str(), "ppsx") { Some(targets[0]) } else { remembered };
            match chosen {
                Some(target) => self.open_with(&path, target, now),
                None => {
                    self.chooser = Some(Chooser { path, targets, picked: 0, remember: false, rest: paths.collect() });
                    return;
                }
            }
        }
    }

    fn open_with(&mut self, path: &Path, target: Target, now: f64) {
        self.settings.add_recent(path, target.app);
        self.enqueue(target.app, Queued::Open(path.to_path_buf(), target.intent));
        self.go(Page::App(target.app), now);
    }

    fn toast(&mut self, text: String, now: f64) {
        self.toast = Some((text, now));
    }

    fn set_language(&mut self, pref: LangPref) {
        self.settings.language = pref;
        self.lang = pref.effective();
        for kind in self.hosts.loaded() {
            if let Some(h) = self.hosts.get(kind) {
                h.set_language(self.lang);
            }
        }
        self.settings.save();
    }

    /// Hand queued opens and "new" requests to the app now shown.
    fn drain_queue(&mut self, ctx: &egui::Context, fresh: bool) {
        let Some(kind) = self.active.app() else { return };
        let Some(items) = self.queue.remove(&kind) else { return };
        let Some(host) = self.hosts.get(kind) else { return };
        for item in items {
            match item {
                Queued::Open(path, intent) => host.open(ctx, &path, intent),
                // A freshly started app already has its blank document (PdfCraft has none).
                Queued::New if fresh && kind != AppKind::Pdf => {}
                Queued::New => host.new_document(ctx),
                Queued::StartScreen => host.show_start_screen(),
            }
        }
    }

    /// Swap pages at the end of a pass: save the context state of the page left, restore (or
    /// start) the page shown.
    fn swap(&mut self, ctx: &egui::Context, frame: &eframe::Frame, to: Page, now: f64) {
        let from = self.active;
        self.snapshots.insert(from, Snapshot::capture(ctx));
        let mut fresh = false;
        match to {
            Page::App(kind) if !self.hosts.is_loaded(kind) => {
                // Start from egui's own defaults, as the app would in its own window.
                self.pristine.restore(ctx);
                let dark = self.applied_dark.unwrap_or(false);
                self.hosts.ensure(kind, ctx, frame, self.lang, dark);
                fresh = true;
                // Started from the rail with nothing to open: WordCraft's own start screen.
                if kind == AppKind::Word && self.queue.get(&kind).is_none_or(Vec::is_empty) {
                    self.enqueue(kind, Queued::StartScreen);
                }
            }
            _ => {
                if let Some(s) = self.snapshots.get(&to) {
                    s.restore(ctx);
                }
            }
        }
        self.active = to;
        self.starting = None;
        self.revealed = now;
        if to == Page::Home {
            ctx.send_viewport_cmd(ViewportCommand::Title("Themisio".into()));
        }
        if let Some(k) = to.app() {
            self.settings.last_app = Some(k);
        }
        self.drain_queue(ctx, fresh);
        // Files dropped or passed for several apps: visit each app that still has some.
        if let Some(next) = AppKind::ALL.into_iter().find(|k| Page::App(*k) != to && self.queue.get(k).is_some_and(|q| !q.is_empty())) {
            self.go(Page::App(next), now);
        }
    }

    /// Stop an app (it already answered its own save questions) and drop its saved state.
    fn unload(&mut self, kind: AppKind, now: f64) {
        log::info!("{} closed", kind.name());
        self.hosts.unload(kind);
        crate::memory::trim_working_set();
        self.snapshots.remove(&Page::App(kind));
        if self.active == Page::App(kind) && self.closing.as_ref().is_none_or(|c| c.after == AfterClose::Home) {
            self.go(Page::Home, now);
        }
    }

    fn start_closing(&mut self, apps: Vec<AppKind>, after: AfterClose) {
        // The app on screen answers first.
        let mut queue: VecDeque<AppKind> = apps.into_iter().collect();
        if let Some(active) = self.active.app()
            && let Some(i) = queue.iter().position(|k| *k == active)
            && let Some(k) = queue.remove(i)
        {
            queue.push_front(k);
        }
        self.closing = Some(Closing { queue, stage: CloseStage::Next, after });
    }

    fn close_window(&mut self, ctx: &egui::Context) {
        if self.closing_window {
            return;
        }
        self.closing_window = true;
        self.settings.save();
        egui::themisio::set_root_close_intercept(false);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// Advance the close flow (once per pass, in `logic`).
    fn step_closing(&mut self, ctx: &egui::Context, now: f64) {
        let Some(closing) = self.closing.as_mut() else { return };
        if closing.stage == CloseStage::Next {
            match closing.queue.pop_front() {
                Some(kind) if self.hosts.is_loaded(kind) => {
                    log::info!("closing: asking {}", kind.name());
                    closing.stage = CloseStage::Inject(kind);
                    self.go(Page::App(kind), now);
                }
                Some(_) => {}
                None => {
                    let after = closing.after;
                    self.closing = None;
                    match after {
                        AfterClose::Window => {
                            log::info!("closing: every app is done; closing the window");
                            self.close_window(ctx);
                        }
                        AfterClose::Home => self.go(Page::Home, now),
                    }
                }
            }
        }
    }

    fn handle_rail(&mut self, action: RailAction, now: f64) {
        match action {
            RailAction::Show(page) => {
                // Moving on while an app asks about unsaved work cancels the close.
                if self.closing.is_some() {
                    self.closing = None;
                }
                self.go(page, now);
            }
            RailAction::Settings => self.dialog = Some(Dialog::Settings),
            RailAction::CloseApp(kind) => self.start_closing(vec![kind], AfterClose::Home),
            RailAction::TogglePin => {
                self.settings.rail = if self.settings.rail == RailMode::Pinned { RailMode::AutoHide } else { RailMode::Pinned };
                self.settings.save();
            }
        }
    }

    fn handle_home(&mut self, action: HomeAction, ctx: &egui::Context, now: f64) {
        match action {
            HomeAction::New(kind) => {
                self.enqueue(kind, Queued::New);
                self.go(Page::App(kind), now);
            }
            HomeAction::OpenDialog => self.open_dialog(ctx),
            HomeAction::OpenRecent(path, app) => {
                let intent = route::targets(&path).into_iter().find(|t| t.app == app).map_or(Intent::Edit, |t| t.intent);
                self.open_with(&path, Target { app, intent }, now);
            }
            HomeAction::RemoveRecent(path) => {
                self.settings.recent.retain(|r| r.path != path);
                self.settings.save();
            }
            HomeAction::ClearRecent => {
                self.settings.recent.clear();
                self.settings.save();
            }
            HomeAction::RevealRecent(path) => reveal(&path),
        }
    }

    /// Themisio's Open dialog, on a worker thread so the window keeps drawing.
    fn open_dialog(&mut self, ctx: &egui::Context) {
        if self.open_dialog.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let lang = self.lang;
        let spawned = std::thread::Builder::new().name("themisio-open".into()).spawn(move || {
            let mut dialog = rfd::FileDialog::new().set_title(t(lang, "Open a file"));
            for (name, exts) in route::open_filters() {
                let label = match name {
                    "*" => "Themisio",
                    "word" => t(lang, "Documents"),
                    "deck" => t(lang, "Presentations"),
                    "grid" => t(lang, "Spreadsheets"),
                    _ => t(lang, "PDF documents"),
                };
                dialog = dialog.add_filter(label, &exts);
            }
            let picked = dialog.pick_files().unwrap_or_default();
            let _ = tx.send(picked);
            ctx.request_repaint();
        });
        if spawned.is_ok() {
            self.open_dialog = Some(rx);
        }
    }

    /// The app bar stays docked on Home and when pinned; elsewhere it tucks away.
    fn rail_docked(&self) -> bool {
        self.settings.rail == RailMode::Pinned || self.active == Page::Home
    }

    /// Light or dark now, from Themisio's theme setting (and Windows' app mode for "System").
    fn want_dark(&self, ctx: &egui::Context) -> bool {
        match self.settings.theme {
            ThemePref::Light => false,
            ThemePref::Dark => true,
            ThemePref::System => ctx.system_theme().map_or(self.applied_dark.unwrap_or(false), |t| t == egui::Theme::Dark),
        }
    }

    /// Give every app (and Home) the same light or dark.
    fn sync_theme(&mut self, ctx: &egui::Context) {
        let dark = self.want_dark(ctx);
        if self.applied_dark != Some(dark) {
            self.applied_dark = Some(dark);
            for kind in self.hosts.loaded() {
                if let Some(h) = self.hosts.get(kind) {
                    h.set_dark(dark);
                }
            }
            ctx.request_repaint();
        }
        if self.active == Page::Home {
            let pref = if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light };
            if ctx.options(|o| o.theme_preference) != pref {
                ctx.set_theme(pref);
            }
        }
    }

    /// Edge-style document tabs for the app shown: click to switch, × or middle-click to close,
    /// + for a new document.
    fn tab_bar(&mut self, ui: &mut egui::Ui, rect: Rect, kind: AppKind, tabs: &[crate::hosts::Tab]) -> Option<TabAction> {
        let v = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let strip = if v.dark_mode { v.extreme_bg_color.gamma_multiply(0.85) } else { v.faint_bg_color.lerp_to_gamma(Color32::BLACK, 0.05) };
        painter.rect_filled(rect, CornerRadius::ZERO, strip);
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color));
        let mut action = None;
        let plus_w = 34.0;
        let avail = (rect.width() - plus_w - 12.0).max(80.0);
        let w = (avail / tabs.len().max(1) as f32).clamp(96.0, 232.0);
        let mut x = rect.left() + 6.0;
        let font = crate::theme::ui_font(ui.ctx(), 12.5);
        for tab in tabs {
            let r = Rect::from_min_size(pos2(x, rect.top() + 5.0), vec2(w - 4.0, rect.height() - 5.0));
            x += w;
            if r.left() > rect.right() - plus_w {
                break;
            }
            let id = ui.scope_id().with(("themisio-tab", kind, tab.id));
            let resp = ui.interact(r, id, Sense::click());
            let hover = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered(), 0.12);
            let top = CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
            if tab.active {
                painter.rect_filled(r, top, v.panel_fill);
                painter.rect_filled(Rect::from_min_size(r.left_top() + vec2(10.0, 0.0), vec2(r.width() - 20.0, 2.0)), CornerRadius::same(1), kind.accent());
            } else if hover > 0.0 {
                painter.rect_filled(r.shrink2(vec2(0.0, 2.0)), CornerRadius::same(7), v.widgets.hovered.weak_bg_fill.gamma_multiply(hover));
            }
            let text_color = if tab.active { v.strong_text_color() } else { v.text_color() };
            painter.circle_filled(pos2(r.left() + 14.0, r.center().y), 4.0, kind.accent().gamma_multiply(if tab.active { 1.0 } else { 0.6 }));
            let close = Rect::from_center_size(pos2(r.right() - 14.0, r.center().y), vec2(18.0, 18.0));
            let title_w = close.left() - (r.left() + 26.0) - 4.0;
            let mut job = egui::text::LayoutJob::simple_singleline(tab.title.clone(), font.clone(), text_color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(title_w.max(10.0));
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            painter.galley(pos2(r.left() + 26.0, r.center().y - galley.size().y / 2.0), galley, text_color);
            let close_resp = ui.interact(close, id.with("x"), Sense::click());
            if tab.dirty && !close_resp.hovered() && !resp.hovered() {
                painter.circle_filled(close.center(), 3.5, text_color.gamma_multiply(0.8));
            } else if tab.active || resp.hovered() || close_resp.hovered() {
                if close_resp.hovered() {
                    painter.rect_filled(close, CornerRadius::same(4), v.widgets.hovered.bg_fill);
                }
                let c = close.center();
                let s = 3.8;
                let st = egui::Stroke::new(1.3, text_color);
                painter.line_segment([c - vec2(s, s), c + vec2(s, s)], st);
                painter.line_segment([c + vec2(-s, s), c + vec2(s, -s)], st);
            }
            if close_resp.clicked() || resp.middle_clicked() {
                action = Some(TabAction::Close(tab.id));
            } else if resp.clicked() && !tab.active {
                action = Some(TabAction::Activate(tab.id));
            }
            resp.on_hover_text(&tab.title);
        }
        let plus = Rect::from_center_size(pos2((x + 16.0).min(rect.right() - plus_w / 2.0), rect.center().y + 2.0), vec2(26.0, 26.0));
        let presp = ui.interact(plus, ui.scope_id().with(("themisio-tab-new", kind)), Sense::click());
        if presp.hovered() {
            painter.rect_filled(plus, CornerRadius::same(6), v.widgets.hovered.weak_bg_fill);
        }
        let c = plus.center();
        let st = egui::Stroke::new(1.5, v.text_color());
        painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], st);
        painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], st);
        if presp.on_hover_text(t(self.lang, "New")).clicked() {
            action = Some(TabAction::New);
        }
        action
    }

    /// Deck draws with Themisio's Vietnamese strings; everything else as is.
    fn translating_for(&self, page: Page) -> bool {
        page == Page::App(AppKind::Deck) && self.lang == Lang::Vi
    }

    fn draw_overlay(&mut self, ctx: &egui::Context, rect: Rect, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let p = Palette::current(ctx);
        let layer = egui::LayerId::new(egui::Order::Debug, Id::unique("themisio-cover"));
        let painter = ctx.layer_painter(layer);
        painter.rect_filled(rect, CornerRadius::ZERO, p.bg.gamma_multiply(alpha.clamp(0.0, 1.0)));
        if let Some(kind) = self.starting
            && alpha > 0.6
        {
            let a = ((alpha - 0.6) / 0.4).clamp(0.0, 1.0);
            let c = rect.center();
            rail::draw_app_icon(&painter, c - vec2(0.0, 22.0), kind, Color32::WHITE, true);
            painter.text(
                c + vec2(0.0, 12.0),
                egui::Align2::CENTER_CENTER,
                tf(self.lang, "Opening {app}…", &[("app", kind.name())]),
                crate::theme::ui_font(ctx, 14.0),
                p.muted.gamma_multiply(a),
            );
        }
    }

    fn chooser_ui(&mut self, ctx: &egui::Context, now: f64) {
        let lang = self.lang;
        let Some(ch) = self.chooser.as_ref() else { return };
        let name = ch.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = route::extension(&ch.path);
        let mut decided: Option<Option<Target>> = None;
        let (modal, style) = {
            let style = std::sync::Arc::new(crate::theme::dialog_style(ctx, self.applied_dark.unwrap_or_else(|| ctx.global_style().visuals.dark_mode)));
            let frame = egui::Frame::popup(&style).inner_margin(egui::Margin::same(20)).corner_radius(CornerRadius::same(12));
            (egui::Modal::new(Id::unique("themisio-chooser")).frame(frame).backdrop_color(Color32::from_black_alpha(96)), style)
        };
        let Some(ch) = self.chooser.as_mut() else { return };
        modal.show(ctx, |ui| {
            ui.set_style(style.clone());
            ui.set_width(400.0);
            ui.heading(t(lang, "Open with"));
            ui.label(tf(lang, "How do you want to open {name}?", &[("name", &name)]));
            ui.add_space(8.0);
            for (i, target) in ch.targets.iter().enumerate() {
                let what = match (target.app, target.intent) {
                    (_, Intent::ConvertToPdf) => t(lang, "Convert to PDF"),
                    (_, Intent::SlideShow) => t(lang, "Start the slide show"),
                    (AppKind::Word, _) => t(lang, "Edit as a document"),
                    (AppKind::Deck, _) => t(lang, "Import as slides (outline)"),
                    (AppKind::Grid, _) => t(lang, "Open as data in a workbook"),
                    (AppKind::Pdf, _) => t(lang, "PDF"),
                };
                let text = format!("{}  ·  {}", what, target.app.name());
                ui.radio_value(&mut ch.picked, i, text);
            }
            ui.add_space(6.0);
            ui.checkbox(&mut ch.remember, tf(lang, "Always do this for .{ext} files", &[("ext", &ext)]));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(t(lang, "Open")).clicked() {
                    decided = Some(ch.targets.get(ch.picked).copied());
                }
                if ui.button(t(lang, "Cancel")).clicked() {
                    decided = Some(None);
                }
            });
        });
        if let Some(choice) = decided
            && let Some(ch) = self.chooser.take()
        {
            if let Some(target) = choice {
                if ch.remember {
                    self.settings.open_with.insert(ext, target.app);
                }
                self.open_with(&ch.path, target, now);
            }
            if !ch.rest.is_empty() {
                self.open_paths(ch.rest, now);
            }
        }
    }

    /// The style the shell's dialogs use over any app.
    fn dialog_style(&self, ctx: &egui::Context) -> std::sync::Arc<egui::Style> {
        std::sync::Arc::new(crate::theme::dialog_style(ctx, self.applied_dark.unwrap_or_else(|| ctx.global_style().visuals.dark_mode)))
    }

    /// A modal in the shell's own style.
    fn modal(&self, ctx: &egui::Context, id: &str) -> (egui::Modal, std::sync::Arc<egui::Style>) {
        let style = self.dialog_style(ctx);
        let frame = egui::Frame::popup(&style).inner_margin(egui::Margin::same(20)).corner_radius(CornerRadius::same(12));
        (egui::Modal::new(Id::unique(id)).frame(frame).backdrop_color(Color32::from_black_alpha(96)), style)
    }

    fn settings_ui(&mut self, ctx: &egui::Context, now: f64) {
        let lang = self.lang;
        let mut open = true;
        let mut language = self.settings.language;
        let mut smooth = self.settings.smoothness;
        let mut motion = self.settings.motion;
        let mut presentation = self.settings.presentation;
        let mut theme = self.settings.theme;
        let mut rail_mode = self.settings.rail;
        let mut renderer = self.settings.renderer;
        let mut register: Option<Result<(), String>> = None;
        let mut about = false;
        let (modal, style) = self.modal(ctx, "themisio-settings");
        let restart = |p: Presentation, r: Renderer| p != self.settings.presentation || r != self.settings.renderer;
        let response = modal.show(ctx, |ui| {
            ui.set_style(style.clone());
            ui.set_width(520.0);
            if dialog_header(ui, t(lang, "Settings")) {
                open = false;
            }
            ui.add_space(10.0);
            egui::Grid::new("themisio-settings-grid").num_columns(2).spacing([18.0, 12.0]).min_col_width(128.0).show(ui, |ui| {
                row_label(ui, t(lang, "Language"));
                segmented(ui, &mut language, &[(LangPref::System, t(lang, "Same as Windows")), (LangPref::En, "English"), (LangPref::Vi, "Tiếng Việt")]);
                ui.end_row();
                row_label(ui, t(lang, "Theme"));
                segmented(ui, &mut theme, &[(ThemePref::System, t(lang, "Same as Windows")), (ThemePref::Light, t(lang, "Light")), (ThemePref::Dark, t(lang, "Dark"))]);
                ui.end_row();
                row_label(ui, t(lang, "App bar"));
                segmented(ui, &mut rail_mode, &[(RailMode::AutoHide, t(lang, "Auto-hide")), (RailMode::Pinned, t(lang, "Always show"))]);
                ui.end_row();
                row_label(ui, t(lang, "Smooth scrolling"));
                segmented(ui, &mut smooth, &[(Smoothness::Off, t(lang, "Off")), (Smoothness::Standard, t(lang, "Standard")), (Smoothness::Fluid, t(lang, "Fluid"))])
                    .on_hover_text(t(lang, "Fluid (like Microsoft Edge)"));
                ui.end_row();
                row_label(ui, t(lang, "Animations"));
                segmented(ui, &mut motion, &[(Motion::System, t(lang, "Same as Windows")), (Motion::On, t(lang, "On")), (Motion::Off, t(lang, "Off"))])
                    .on_hover_text(t(lang, "Same as Windows (Animation effects)"));
                ui.end_row();
                row_label(ui, t(lang, "Rendering"));
                segmented(ui, &mut renderer, &[(Renderer::Gpu, "GPU (Direct3D 12)"), (Renderer::OpenGl, t(lang, "OpenGL (least memory)"))]);
                ui.end_row();
                row_label(ui, t(lang, "Frame pacing"));
                segmented(ui, &mut presentation, &[(Presentation::Balanced, t(lang, "Balanced (VSync)")), (Presentation::LowLatency, t(lang, "Low latency"))])
                    .on_hover_text(t(lang, "Low-latency (higher power use)"));
                ui.end_row();
                if restart(presentation, renderer) {
                    ui.label("");
                    ui.label(RichText::new(t(lang, "Restart Themisio to apply this.")).small().color(ui.visuals().hyperlink_color));
                    ui.end_row();
                }
                row_label(ui, t(lang, "File types"));
                ui.horizontal(|ui| {
                    if ui.button(t(lang, "Register file types")).on_hover_text(t(lang, "Open documents, presentations, workbooks and PDFs with Themisio")).clicked() {
                        register = Some(crate::assoc::register());
                    }
                    if cfg!(windows) && ui.button(t(lang, "Choose default apps…")).clicked() {
                        crate::assoc::open_default_apps();
                    }
                });
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui.link(t(lang, "About Themisio")).clicked() {
                    about = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("Themisio {}", crate::VERSION)).small().weak());
                });
            });
        });
        if response.should_close() {
            open = false;
        }
        if language != self.settings.language {
            self.set_language(language);
        }
        if smooth != self.settings.smoothness {
            self.settings.smoothness = smooth;
            let (reach, secs) = smooth.wheel();
            egui::themisio::set_wheel_smoothing(reach, secs);
            self.settings.save();
        }
        if motion != self.settings.motion
            || presentation != self.settings.presentation
            || theme != self.settings.theme
            || rail_mode != self.settings.rail
            || renderer != self.settings.renderer
        {
            self.settings.motion = motion;
            self.settings.presentation = presentation;
            self.settings.theme = theme;
            self.settings.rail = rail_mode;
            self.settings.renderer = renderer;
            self.settings.save();
        }
        match register {
            Some(Ok(())) => self.toast(t(self.lang, "File types registered. Pick Themisio in Default apps to make it the default.").into(), now),
            Some(Err(e)) => self.toast(tf(self.lang, "Couldn't register file types: {error}", &[("error", &e)]), now),
            None => {}
        }
        if about {
            self.dialog = Some(Dialog::About);
        } else if !open {
            self.dialog = None;
        }
    }

    fn about_ui(&mut self, ctx: &egui::Context) {
        let lang = self.lang;
        let mut close = false;
        let (modal, style) = self.modal(ctx, "themisio-about");
        let response = modal.show(ctx, |ui| {
            ui.set_style(style.clone());
            ui.set_width(420.0);
            if dialog_header(ui, t(lang, "About Themisio")) {
                close = true;
            }
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if let Some(logo) = crate::rail::logo(ctx) {
                    ui.add(egui::Image::from_texture(&logo).fit_to_exact_size(vec2(84.0, 84.0)));
                }
                ui.add_space(4.0);
                ui.label(RichText::new("Themisio").size(22.0).strong());
                ui.label(t(lang, "Make your documents glow."));
                ui.label(RichText::new(format!("{} {}", t(lang, "Version"), crate::VERSION)).small().weak());
            });
            ui.add_space(10.0);
            ui.label(t(lang, "Documents, presentations, spreadsheets and PDFs in one window."));
            ui.add_space(4.0);
            egui::CollapsingHeader::new(t(lang, "Open-source licences")).show(ui, |ui| {
                ui.label(RichText::new(t(lang, "Themisio is built on open-source engines by the ArtCraft team (MIT OR Apache-2.0):")).small());
                for (name, url) in crate::UPSTREAMS {
                    ui.hyperlink_to(RichText::new(*name).small(), *url);
                }
                ui.hyperlink_to(RichText::new("github.com/nev-minhnh/Themisio").small(), "https://github.com/nev-minhnh/Themisio");
            });
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t(lang, "Close")).clicked() {
                    close = true;
                }
            });
        });
        if close || response.should_close() {
            self.dialog = None;
        }
    }

    fn closing_banner(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let Some(closing) = &self.closing else { return };
        let CloseStage::Waiting(kind) = closing.stage else { return };
        let lang = self.lang;
        let area = egui::Area::new(Id::unique("themisio-closing-banner"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(rect.center().x - 210.0, rect.bottom() - 64.0));
        let mut cancel = false;
        let style = self.dialog_style(ui.ctx());
        area.show(ui.ctx(), |ui| {
            ui.set_style(style.clone());
            egui::Frame::popup(&style).inner_margin(12.0).corner_radius(CornerRadius::same(10)).show(ui, |ui| {
                ui.set_width(400.0);
                ui.horizontal(|ui| {
                    let title = if closing.after == AfterClose::Window { t(lang, "Closing Themisio…") } else { "" };
                    ui.label(RichText::new(title).strong());
                    ui.label(tf(lang, "{app} is asking about unsaved changes.", &[("app", kind.name())]));
                    if ui.button(t(lang, "Don't close")).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if cancel {
            self.closing = None;
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context, rect: Rect, now: f64) {
        let Some((text, at)) = &self.toast else { return };
        let age = now - at;
        if age > 4.5 {
            self.toast = None;
            return;
        }
        let fade = (if age < 0.2 { age / 0.2 } else if age > 4.0 { (4.5 - age) / 0.5 } else { 1.0 }) as f32;
        let p = Palette::current(ctx);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::unique("themisio-toast")));
        let galley = painter.layout(text.clone(), crate::theme::ui_font(ctx, 13.5), p.accent_text, 520.0);
        let size = galley.size() + vec2(28.0, 18.0);
        let pos = pos2(rect.center().x - size.x / 2.0, rect.bottom() - 32.0 - size.y + (1.0 - ease_out(fade)) * 12.0);
        let r = Rect::from_min_size(pos, size);
        painter.rect_filled(r, CornerRadius::same(10), p.accent.gamma_multiply(fade));
        painter.galley(r.min + vec2(14.0, 9.0), galley, p.accent_text.gamma_multiply(fade));
        ctx.request_repaint();
    }

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let mods = Modifiers::CTRL | Modifiers::ALT;
        let pages = [
            (Key::Num1, Page::Home),
            (Key::Num2, Page::App(AppKind::Word)),
            (Key::Num3, Page::App(AppKind::Deck)),
            (Key::Num4, Page::App(AppKind::Grid)),
            (Key::Num5, Page::App(AppKind::Pdf)),
        ];
        for (key, page) in pages {
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, key))) {
                self.go(page, now);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(mods, Key::O))) {
            self.open_dialog(ctx);
        }
    }
}

/// Show `path` selected in Explorer (or its folder elsewhere).
fn reveal(path: &Path) {
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(not(windows))]
    let result = std::process::Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();
    if let Err(e) = result {
        log::warn!("reveal {}: {e}", path.display());
    }
}

impl eframe::App for Themisio {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if raw.viewport_id == ViewportId::ROOT && !self.closing_window {
            // The window's close button: Themisio runs the close flow (each app asks about its own
            // unsaved work) instead of letting eframe close at once.
            if let Some(info) = raw.viewports.get_mut(&ViewportId::ROOT) {
                let before = info.events.len();
                info.events.retain(|e| !matches!(e, egui::ViewportEvent::Close));
                if info.events.len() != before && self.closing.is_none() {
                    let loaded = self.hosts.loaded();
                    self.start_closing(loaded, AfterClose::Window);
                }
                // The close flow's turn for the app shown: give it the close request.
                if let Some(closing) = self.closing.as_mut()
                    && let CloseStage::Inject(kind) = closing.stage
                    && self.active == Page::App(kind)
                    && self.switch.is_none()
                {
                    info.events.push(egui::ViewportEvent::Close);
                    closing.stage = CloseStage::Judge(kind);
                    let _ = egui::themisio::take_root_cancel_close();
                }
            }
            // Files dropped on the window: the app shown keeps what it takes (a picture into a
            // document, pages into a PDF); Themisio opens the rest in their own apps.
            if !raw.dropped_files.is_empty() {
                let shown = self.active.app();
                let mut mine = Vec::new();
                raw.dropped_files.retain(|f| {
                    let path = f.path().to_path_buf();
                    if path.as_os_str().is_empty() {
                        return true;
                    }
                    match shown {
                        Some(app) if route::app_takes_drop(app, &path) => true,
                        _ => {
                            mine.push(path);
                            false
                        }
                    }
                });
                if !mine.is_empty() {
                    let now = raw.time.unwrap_or_default();
                    self.open_paths(mine, now);
                }
            }
        }
        if let Some(kind) = self.active.app()
            && let Some(host) = self.hosts.get(kind)
        {
            host.raw_input_hook(ctx, raw);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.passes += 1;
        let now = ctx.input(|i| i.time);
        // An app asked to close the window (File ▸ Exit, its save prompt finished, its last
        // document closed): close that app; during the close flow, move on to the next one.
        if egui::themisio::take_root_close_request() {
            match self.closing.as_ref().map(|c| c.stage) {
                Some(CloseStage::Waiting(k) | CloseStage::Judge(k) | CloseStage::Inject(k)) => {
                    self.unload(k, now);
                    if let Some(c) = self.closing.as_mut() {
                        c.stage = CloseStage::Next;
                    }
                }
                // Left over from an app already closed.
                Some(CloseStage::Next) => {}
                None => {
                    if let Some(kind) = self.active.app() {
                        self.unload(kind, now);
                    }
                }
            }
        }
        self.step_closing(ctx, now);
        if let Some(rx) = &self.open_dialog
            && let Ok(paths) = rx.try_recv()
        {
            self.open_dialog = None;
            self.open_paths(paths, now);
        }
        if let Some(rx) = &self.instance
            && let Ok(paths) = rx.try_recv()
        {
            self.open_paths(paths, now);
        }
        if self.dialog.is_none() && self.chooser.is_none() {
            self.shortcuts(ctx, now);
        }
        // Files opened for the app already on screen.
        if self.switch.is_none()
            && let Some(kind) = self.active.app()
            && self.queue.get(&kind).is_some_and(|q| !q.is_empty())
        {
            self.drain_queue(ctx, false);
        }
        self.sync_theme(ctx);
        let motion = self.effective_motion(now);
        egui::themisio::set_motion(motion);
        self.trim.tick(ctx, now);
        crate::deck_i18n::set_deck_vietnamese(self.translating_for(self.active));
        let translating = self.active.app().is_some();
        if let Some(kind) = self.active.app()
            && let Some(host) = self.hosts.get(kind)
        {
            let was = egui::themisio::set_translating(translating);
            host.logic(ctx, frame);
            egui::themisio::set_translating(was);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let full = ui.max_rect();
        // The app bar: docked (Home, or pinned), or tucked away and slid out over the app when the
        // pointer reaches the window's left edge.
        let docked = self.rail_docked();
        if !docked {
            let pointer = ctx.pointer_hover_pos();
            let near = pointer.is_some_and(|p| {
                full.contains(p) && (p.x <= full.left() + 6.0 || (self.rail_open && p.x <= full.left() + rail::WIDTH + 12.0))
            });
            if near {
                self.rail_open = true;
                self.rail_seen = now;
            } else if self.rail_open && now - self.rail_seen > 0.35 {
                self.rail_open = false;
            } else if self.rail_open {
                ctx.request_repaint_after(std::time::Duration::from_millis(60));
            }
        }
        let app_rect = if docked { Rect::from_min_max(pos2(full.left() + rail::WIDTH, full.top()), full.max) } else { full };
        let tabs = self.active.app().and_then(|k| self.hosts.get(k)).and_then(|h| h.tabs());
        let content_rect = if tabs.is_some() { Rect::from_min_max(pos2(app_rect.left(), app_rect.top() + TAB_BAR), app_rect.max) } else { app_rect };

        // The page on screen.
        let mut child = ui.new_child(UiBuilder::new().max_rect(content_rect).id_salt(("themisio-page", self.active)));
        child.set_clip_rect(content_rect);
        match self.active {
            Page::Home => {
                if let Some(action) = self.home.show(&mut child, self.lang, &self.settings.recent) {
                    self.handle_home(action, &ctx, now);
                }
            }
            Page::App(kind) => {
                crate::deck_i18n::set_deck_vietnamese(self.translating_for(self.active));
                if let Some(host) = self.hosts.get(kind) {
                    let was = egui::themisio::set_translating(true);
                    host.ui(&mut child, frame);
                    egui::themisio::set_translating(was);
                }
                // Themisio's interface family in the app's fonts, and in Vietnamese the letters
                // for the rail's labels and translated strings (DeckCraft's own faces have none,
                // so Inter leads there).
                crate::theme::ensure_fonts(&ctx, self.lang == Lang::Vi, kind == AppKind::Deck);
                if let Some(tabs) = &tabs {
                    let bar = Rect::from_min_size(app_rect.min, vec2(app_rect.width(), TAB_BAR));
                    let mut bar_ui = ui.new_child(UiBuilder::new().max_rect(bar).id_salt(("themisio-tabs", kind)));
                    let was = egui::themisio::set_translating(true);
                    let action = self.tab_bar(&mut bar_ui, bar, kind, tabs);
                    egui::themisio::set_translating(was);
                    if let Some(host) = self.hosts.get(kind) {
                        match action {
                            Some(TabAction::Activate(id)) => host.activate_tab(id),
                            Some(TabAction::Close(id)) => host.close_tab(id),
                            Some(TabAction::New) => host.new_document(&ctx),
                            None => {}
                        }
                    }
                }
            }
        }

        // The app bar, above the page.
        let loaded = self.hosts.loaded();
        let pinned = self.settings.rail == RailMode::Pinned;
        let reveal = if docked { 1.0 } else { ctx.animate_bool_with_time(Id::unique("themisio-rail-reveal"), self.rail_open, 0.16) };
        if reveal > 0.0 {
            let x = full.left() - rail::WIDTH * (1.0 - ease_out(reveal));
            let rail_rect = Rect::from_min_size(pos2(x, full.top()), vec2(rail::WIDTH, full.height()));
            let action = egui::Area::new(Id::unique("themisio-rail-area"))
                .order(if docked { egui::Order::Background } else { egui::Order::Foreground })
                .fixed_pos(rail_rect.min)
                .interactable(true)
                .show(&ctx, |ui| {
                    ui.set_min_size(rail_rect.size());
                    if !docked {
                        // A soft shadow where the bar lies over the app.
                        let p = ui.painter();
                        for i in 0..6 {
                            let a = (18.0 - i as f32 * 3.0) * reveal;
                            p.rect_filled(
                                Rect::from_min_size(pos2(rail_rect.right() + i as f32 * 2.0, rail_rect.top()), vec2(2.0, rail_rect.height())),
                                CornerRadius::ZERO,
                                Color32::from_black_alpha(a as u8),
                            );
                        }
                    }
                    rail::show(ui, rail_rect, self.target_page(), &loaded, self.lang, pinned)
                })
                .inner;
            if let Some(action) = action {
                self.handle_rail(action, now);
            }
        }
        if !docked && reveal < 1.0 {
            // Where the tucked-away bar is: a thin gold line down the left edge, a little brighter
            // and wider when the pointer comes near, so it is clear where to point.
            let near = ctx.pointer_hover_pos().is_some_and(|p| full.contains(p) && p.x <= full.left() + 28.0);
            let glow = ctx.animate_bool_with_time(Id::unique("themisio-rail-edge-glow"), near, 0.15);
            let fade = 1.0 - reveal;
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::unique("themisio-rail-edge")));
            let w = 2.0 + 1.5 * glow;
            let line = Rect::from_min_size(pos2(full.left(), full.top() + 6.0), vec2(w, full.height() - 12.0));
            p.rect_filled(line, CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, crate::theme::GOLD.gamma_multiply((0.55 + 0.45 * glow) * fade));
        }

        // Shell dialogs.
        if self.chooser.is_some() {
            self.chooser_ui(&ctx, now);
        }
        match self.dialog {
            Some(Dialog::Settings) => self.settings_ui(&ctx, now),
            Some(Dialog::About) => self.about_ui(&ctx),
            None => {}
        }
        self.closing_banner(ui, app_rect);
        self.toast_ui(&ctx, app_rect, now);

        // Close flow: did the app shown ask about unsaved work?
        if let Some(closing) = self.closing.as_mut()
            && let CloseStage::Judge(kind) = closing.stage
        {
            let asked = egui::themisio::take_root_cancel_close();
            // eframe saw a close request this pass; the window stays until the flow ends.
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            let _ = egui::themisio::take_root_cancel_close();
            if asked {
                log::info!("closing: {} asks about unsaved work", kind.name());
                closing.stage = CloseStage::Waiting(kind);
            } else {
                // Nothing unsaved: the app is done (a close it sent itself is part of this).
                let _ = egui::themisio::take_root_close_request();
                self.unload(kind, now);
                if let Some(c) = self.closing.as_mut() {
                    c.stage = CloseStage::Next;
                }
            }
        }

        // Page switch: cover, swap at full cover, reveal.
        let animate = self.effective_motion(now);
        let (cover, reveal) = if animate { (COVER, REVEAL) } else { (0.0, 0.0) };
        let mut cover_alpha = 0.0f32;
        if let Some(switch) = &self.switch {
            let t = if cover > 0.0 { ((now - switch.started) / cover) as f32 } else { 1.0 };
            cover_alpha = ease_in(t.min(1.0));
            if t >= 1.0 {
                let to = switch.to;
                self.switch = None;
                // Drawn at full cover this pass; the next pass draws the new page.
                cover_alpha = 1.0;
                self.draw_overlay(&ctx, app_rect, cover_alpha);
                self.swap(&ctx, frame, to, now);
                ctx.request_repaint();
                return;
            }
            ctx.request_repaint();
        } else if reveal > 0.0 && now - self.revealed < reveal {
            let t = ((now - self.revealed) / reveal) as f32;
            cover_alpha = 1.0 - ease_out(t);
            ctx.request_repaint();
        }
        self.draw_overlay(&ctx, app_rect, cover_alpha);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(pdf) = self.hosts.pdf.as_mut() {
            crate::hosts::Host::save(pdf, storage);
        }
        self.settings.save();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for kind in self.hosts.loaded() {
            self.hosts.unload(kind);
        }
        self.settings.save();
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

/// A dialog's title row: the title, and a close button drawn as a cross (no font glyph, so it
/// looks the same over every app). True when the close button was clicked.
fn dialog_header(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).heading());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
            let v = ui.visuals();
            if hover > 0.0 {
                ui.painter().rect_filled(rect, CornerRadius::same(6), v.widgets.hovered.bg_fill.gamma_multiply(hover));
            }
            let c = rect.center();
            let s = 5.0;
            let stroke = egui::Stroke::new(1.5, v.text_color());
            ui.painter().line_segment([c - vec2(s, s), c + vec2(s, s)], stroke);
            ui.painter().line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
            if resp.clicked() {
                close = true;
            }
        });
    });
    close
}

/// A setting's name in the left column, vertically centred on its row.
fn row_label(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(ui.visuals().weak_text_color().lerp_to_gamma(ui.visuals().text_color(), 0.6))).selectable(false));
}

/// A row of mutually exclusive choices in one rounded track; the chosen one is raised.
fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> egui::Response {
    let v = ui.visuals().clone();
    let track = v.faint_bg_color;
    let raised = if v.dark_mode { v.widgets.hovered.bg_fill } else { Color32::WHITE };
    egui::Frame::NONE
        .fill(track)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (option, label) in options {
                    let selected = *value == *option;
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let galley = ui.painter().layout_no_wrap(label.to_string(), font, v.text_color());
                    let size = vec2(galley.size().x + 22.0, 26.0);
                    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && !selected, 0.12);
                    let on = ui.ctx().animate_bool_with_time(resp.id.with("on"), selected, 0.16);
                    if on > 0.0 {
                        ui.painter().rect_filled(rect, CornerRadius::same(6), raised.gamma_multiply(on));
                        if !v.dark_mode {
                            ui.painter().rect_stroke(rect, CornerRadius::same(6), egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color.gamma_multiply(on)), egui::StrokeKind::Inside);
                        }
                    } else if hover > 0.0 {
                        ui.painter().rect_filled(rect, CornerRadius::same(6), raised.gamma_multiply(0.45 * hover));
                    }
                    let color = if selected { v.strong_text_color() } else { v.text_color().gamma_multiply(0.85) };
                    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
                    if resp.clicked() {
                        *value = *option;
                    }
                }
            })
            .response
        })
        .inner
}
