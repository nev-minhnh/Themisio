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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    General,
    Performance,
    Files,
    Providers,
    Features,
    Translation,
    Skills,
    About,
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
    /// Pinned tabs (app, document id): shown first, compact, without a close button.
    pinned_tabs: std::collections::HashSet<(AppKind, u64)>,
    /// The order the user dragged each app's tabs into (ids; new tabs go at the end).
    tab_order: HashMap<AppKind, Vec<u64>>,
    /// The tab being dragged: (app, tab, pointer offset from the tab's left edge).
    tab_drag: Option<(AppKind, u64, f32)>,
    /// How many tabs each app showed last pass (closing the last one goes Home).
    tab_count: HashMap<AppKind, usize>,
    /// Success and failure popups for everything that finishes, in every app.
    notices: crate::notices::Notices,
    /// The writing assistant (check, ask, translate) and its auto-check.
    assist: crate::ai::assist::Assist,
    dock_last: Option<DockKind>,
    writer: crate::ai::writer::Writer,
    /// The CalcSheet assistant (chat).
    sheet_chat: crate::ai::sheet_chat::SheetChat,
    /// Settings › AI's state, and which page of Settings is shown.
    ai_ui: crate::ai::settings_ui::AiSettingsUi,
    settings_page: SettingsPage,
    /// Each app's tabs last pass, to notice a document that was saved or opened.
    last_tabs: HashMap<AppKind, Vec<crate::hosts::Tab>>,
    close_queue: CloseQueue,
}

/// Height of the document tab bar.
const TAB_BAR: f32 = 34.0;

/// What the user did on the tab bar.
enum TabAction {
    Activate(u64),
    Close(u64),
    New,
    /// Save, Save As or Print document `id` (its app's own command).
    Command(u64, crate::hosts::DocCommand),
    TogglePin(u64),
    CloseOthers(u64),
    CloseRight(u64),
}

/// Tabs waiting to be closed one after another (Close other tabs, Close tabs to the right):
/// each app asks about unsaved changes in its own dialog, so the next one waits until the
/// previous tab has gone. Given up when a tab stays (the user kept it) for a while.
#[derive(Default)]
struct CloseQueue {
    kind: Option<AppKind>,
    ids: Vec<u64>,
    waiting: Option<(u64, f64)>,
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
        crate::ai::assist::install_menu_hook();
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
            pinned_tabs: Default::default(),
            tab_order: HashMap::new(),
            tab_drag: None,
            tab_count: HashMap::new(),
            notices: Default::default(),
            assist: Default::default(),
            dock_last: None,
            writer: Default::default(),
            sheet_chat: Default::default(),
            ai_ui: Default::default(),
            settings_page: SettingsPage::General,
            last_tabs: HashMap::new(),
            close_queue: Default::default(),
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
        // An app whose documents were all closed opens on its start screen (new, recent).
        if let Some(kind) = page.app()
            && self.hosts.is_loaded(kind)
            && let Some(host) = self.hosts.get(kind)
            && host.tabs().is_some_and(|t| t.is_empty())
        {
            host.show_start_screen();
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
        self.notices.say(None, text, now);
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
    /// Whether the writing assistant is on for `kind` (and there is a model to use).
    fn writing_on(&self, kind: AppKind) -> bool {
        let a = &self.settings.ai.apps;
        match kind {
            AppKind::Word => a.documents,
            AppKind::Deck => a.presentation,
            AppKind::Grid => a.calcsheet,
            AppKind::Pdf => false,
        }
    }

    /// The writing assistant, translation and the CalcSheet assistant, over app `kind`.
    /// Whether the AI pane (the CalcSheet assistant or the writing assistant) is open over `kind`.
    fn dock_open(&self, kind: AppKind) -> bool {
        (kind == AppKind::Grid && self.settings.ai.apps.calcsheet && self.sheet_chat.open) || (kind != AppKind::Pdf && self.writing_on(kind) && self.writer.panel)
    }

    /// `area`: the app's page; `dock`: the docked pane's rectangle while it is (being) shown.
    fn ai_frame(&mut self, ctx: &egui::Context, kind: AppKind, area: Rect, dock: Option<Rect>, now: f64) {
        use crate::ai::assist::{Act, Mode};
        let lang = self.lang;
        let writing = self.writing_on(kind);
        crate::ai::assist::set_menu_context(lang, writing);
        if kind == AppKind::Pdf {
            return;
        }
        // A right-click menu item or a shortcut.
        let mut request = crate::ai::assist::take_request();
        let mods = egui::Modifiers::CTRL | egui::Modifiers::ALT;
        for (key, mode) in [(egui::Key::G, Mode::Check), (egui::Key::K, Mode::Ask), (egui::Key::T, Mode::Translate), (egui::Key::D, Mode::Define)] {
            if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(mods, key))) {
                request = Some(mode);
            }
        }
        if kind == AppKind::Grid && ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(mods, egui::Key::A))) {
            self.sheet_chat.open = !self.sheet_chat.open;
        }
        // One pane at a time: the one opened last stays.
        if self.sheet_chat.open && self.writer.panel {
            if self.dock_last == Some(DockKind::Chat) {
                self.sheet_chat.open = false;
            } else {
                self.writer.panel = false;
            }
        }
        self.dock_last = if self.sheet_chat.open && kind == AppKind::Grid {
            Some(DockKind::Chat)
        } else if self.writer.panel {
            Some(DockKind::Writer)
        } else {
            None
        };
        let dock_rect = dock.unwrap_or(Rect::from_min_size(pos2(area.right(), area.top()), vec2(crate::ai::kit::PANE_W, area.height())));
        // Double-click a word: its meaning (the app has selected the word in this pass already).
        if writing
            && request.is_none()
            && self.settings.ai.define_on_double_click
            && !self.assist.open
            && ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary))
            && ctx.pointer_latest_pos().is_some_and(|p| area.contains(p))
            && let Some(target) = self.hosts.get(kind).and_then(|h| h.ai_target())
            && target.selection
            && !target.text.trim().is_empty()
            && target.text.trim().split_whitespace().count() <= 3
            && target.text.chars().count() <= 48
        {
            let pos = ctx.pointer_latest_pos().unwrap_or(area.center());
            self.assist.open_at(ctx, kind, Some(target), Mode::Define, pos, &self.settings.ai, lang);
        }
        if request == Some(Mode::Check) {
            // The writing assistant checks the whole document and opens its panel.
            if writing {
                self.writer.check_now();
            }
        } else if let Some(mode) = request
            && (matches!(mode, Mode::Translate | Mode::Define) || writing)
        {
            let pos = ctx.pointer_latest_pos().unwrap_or(area.center());
            let target = self.hosts.get(kind).and_then(|h| h.ai_target());
            self.assist.open_at(ctx, kind, target, mode, pos, &self.settings.ai, lang);
        }
        let style = self.dialog_style(ctx);
        // The writing assistant: underlines, cards, the score and its panel.
        if writing && let Some(host) = self.hosts.get(kind) {
            let out = self.writer.frame(ctx, host, kind, &mut self.settings.ai, lang, now, area, dock_rect, &style);
            for (level, text) in out.notices {
                self.notices.push(level, Some(kind.name()), text, now);
            }
            if out.save {
                self.settings.save();
            }
        }
        if self.assist.open && self.assist.kind != Some(kind) {
            self.assist.close();
        }
        let (acts, changed) = self.assist.ui(ctx, area, &self.settings.ai, lang, &style);
        if changed {
            // The popup's choices become the defaults.
            self.settings.ai.write_model = self.assist.model.clone();
            self.settings.ai.effort = self.assist.effort.clone();
            self.settings.ai.thinking = self.assist.thinking;
            self.settings.save();
        }
        for act in acts {
            let Some(target) = self.assist.target.clone() else { continue };
            match act {
                Act::Replace(text) => match self.hosts.get(kind).map_or(Err("closed".into()), |h| h.ai_replace_all(&target, &text)) {
                    Ok(()) => {
                        self.notices.push(crate::notices::Level::Success, Some(kind.name()), t(lang, "Text replaced"), now);
                        self.assist.close();
                    }
                    Err(e) => self.notices.push(crate::notices::Level::Error, Some(kind.name()), tf(lang, "Couldn't replace the text: {error}", &[("error", &e)]), now),
                },
                Act::Copy(text) => {
                    ctx.copy_text(text);
                    self.notices.push(crate::notices::Level::Success, None, t(lang, "Copied to the clipboard"), now);
                }
            }
        }
        // The CalcSheet assistant.
        if kind == AppKind::Grid && self.settings.ai.apps.calcsheet {
            if !self.sheet_chat.open {
                self.chat_button(ctx, area, lang, &style);
            }
            let acts = self.sheet_chat.ui(ctx, dock_rect, &self.settings.ai, lang, &style);
            for act in acts {
                match act {
                    crate::ai::sheet_chat::ChatAct::Send(prompt) => {
                        let (context, docs) = match self.hosts.get(kind) {
                            Some(h) => (h.sheet_context().unwrap_or_default(), h.command_docs(crate::ai::sheet_chat::ALLOWED)),
                            None => (String::new(), String::new()),
                        };
                        self.sheet_chat.send(ctx, &self.settings.ai, lang, prompt, context, docs);
                    }
                    crate::ai::sheet_chat::ChatAct::Run(i) => {
                        let actions = self.sheet_chat.actions(i);
                        let mut results = Vec::new();
                        let mut errors = Vec::new();
                        if let Some(h) = self.hosts.get(kind) {
                            for a in &actions {
                                results.push(h.run_json(&a.command, a.params.clone()).map(|_| ()));
                            }
                            // Check what was written: formulas that show errors are reported.
                            let ranges: Vec<String> = actions.iter().flat_map(|a| crate::hosts::sheet_context::written_ranges(&a.command, &a.params)).collect();
                            errors = h.sheet_errors(&ranges);
                        }
                        let failed = results.iter().filter(|r| r.is_err()).count();
                        let done = results.len() - failed;
                        if failed == 0 && !errors.is_empty() {
                            self.notices.push(
                                crate::notices::Level::Error,
                                Some(kind.name()),
                                tf(lang, "{n} changes made, but {e} cells show errors", &[("n", &done.to_string()), ("e", &errors.len().to_string())]),
                                now,
                            );
                        } else if failed == 0 {
                            self.notices.push(crate::notices::Level::Success, Some(kind.name()), tf(lang, "{n} changes made — Ctrl+Z undoes them", &[("n", &done.to_string())]), now);
                        } else {
                            self.notices.push(
                                crate::notices::Level::Error,
                                Some(kind.name()),
                                tf(lang, "{done} changes made, {failed} failed", &[("done", &done.to_string()), ("failed", &failed.to_string())]),
                                now,
                            );
                        }
                        self.sheet_chat.ran(i, results, errors);
                    }
                }
            }
        }
    }

    /// The round button that opens the CalcSheet assistant.
    fn chat_button(&mut self, ctx: &egui::Context, area: Rect, lang: Lang, style: &std::sync::Arc<egui::Style>) {
        let size = vec2(132.0, 34.0);
        let at = pos2(area.right() - size.x - 22.0, area.bottom() - size.y - 76.0);
        let mut clicked = false;
        egui::Area::new(Id::unique("themisio-chat-button")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            ui.set_style(style.clone());
            let (r, resp) = ui.allocate_exact_size(size, Sense::click());
            let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
            let fill = crate::theme::BRAND_DEEP.lerp_to_gamma(crate::theme::BRAND, hover);
            ui.painter().add(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color: Color32::from_black_alpha(70) }.as_shape(r, CornerRadius::same(17)));
            ui.painter().rect(r, CornerRadius::same(17), fill, egui::Stroke::new(1.0, crate::theme::GOLD.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            crate::ai::assist::draw_sparkle(ui.painter(), pos2(r.left() + 18.0, r.center().y), 7.5, crate::theme::GOLD);
            ui.painter().text(pos2(r.left() + 32.0, r.center().y), egui::Align2::LEFT_CENTER, t(lang, "Ask AI"), crate::theme::ui_font(ctx, 13.0), crate::theme::CREAM);
            clicked = resp.on_hover_text(t(lang, "CalcSheet assistant (Ctrl+Alt+A)")).clicked();
        });
        if clicked {
            self.sheet_chat.open = true;
        }
    }

    fn tab_action(&mut self, ctx: &egui::Context, kind: AppKind, tabs: &[crate::hosts::Tab], action: Option<TabAction>, now: f64) {
        let Some(action) = action else { return };
        let lang = self.lang;
        let ordered: Vec<u64> = self.ordered_tabs(kind, tabs).iter().map(|t| t.id).collect();
        let mut toast = None;
        if let Some(host) = self.hosts.get(kind) {
            match action {
                TabAction::Activate(id) => host.activate_tab(id),
                TabAction::Close(id) => host.close_tab(id),
                TabAction::New => host.new_document(ctx),
                TabAction::Command(id, cmd) => {
                    host.activate_tab(id);
                    if let Err(e) = host.command(cmd) {
                        toast = Some(tf(lang, "Couldn't do that: {error}", &[("error", &e)]));
                    }
                }
                TabAction::TogglePin(id) => {
                    if !self.pinned_tabs.remove(&(kind, id)) {
                        self.pinned_tabs.insert((kind, id));
                    }
                }
                TabAction::CloseOthers(id) => {
                    self.close_queue = CloseQueue { kind: Some(kind), ids: ordered.iter().copied().filter(|t| *t != id).collect(), waiting: None };
                }
                TabAction::CloseRight(id) => {
                    let after = ordered.iter().position(|t| *t == id).map_or(Vec::new(), |i| ordered[i + 1..].to_vec());
                    self.close_queue = CloseQueue { kind: Some(kind), ids: after, waiting: None };
                }
            }
        }
        if let Some(text) = toast {
            self.toast(text, now);
        }
    }

    /// The apps' own messages, and the saves and opens they don't announce, as notifications.
    fn gather_notices(&mut self, now: f64) {
        let lang = self.lang;
        for kind in AppKind::ALL {
            let Some(host) = self.hosts.get(kind) else {
                self.last_tabs.remove(&kind);
                continue;
            };
            let said = host.take_notices();
            let tabs = host.tabs().unwrap_or_default();
            let mut extra = Vec::new();
            if said.is_empty()
                && let Some(prev) = self.last_tabs.get(&kind)
            {
                for tab in &tabs {
                    match prev.iter().find(|p| p.id == tab.id) {
                        Some(p) if p.dirty && !tab.dirty && p.title.trim_end_matches('*') != "" => {
                            extra.push(tf(lang, "Saved {name}", &[("name", tab.title.trim_end_matches(['*', ' ']))]));
                        }
                        None if !prev.is_empty() && !looks_untitled(&tab.title) => {
                            extra.push(tf(lang, "Opened {name}", &[("name", &tab.title)]));
                        }
                        _ => {}
                    }
                }
            }
            self.last_tabs.insert(kind, tabs);
            for text in said.into_iter().chain(extra) {
                self.notices.say(Some(kind.name()), text, now);
            }
        }
    }

    /// Close the queued tabs one at a time (see [`CloseQueue`]).
    fn drain_close_queue(&mut self, now: f64) {
        let Some(kind) = self.close_queue.kind else { return };
        let Some(host) = self.hosts.get(kind) else {
            self.close_queue = CloseQueue::default();
            return;
        };
        let open: Vec<u64> = host.tabs().unwrap_or_default().iter().map(|t| t.id).collect();
        if let Some((id, since)) = self.close_queue.waiting {
            if open.contains(&id) {
                // Still asking about unsaved changes, or kept: give up after a while.
                if now - since > 60.0 {
                    self.close_queue = CloseQueue::default();
                }
                return;
            }
            self.close_queue.waiting = None;
        }
        while let Some(id) = (!self.close_queue.ids.is_empty()).then(|| self.close_queue.ids.remove(0)) {
            if !open.contains(&id) {
                continue;
            }
            self.pinned_tabs.remove(&(kind, id));
            host.close_tab(id);
            self.close_queue.waiting = Some((id, now));
            return;
        }
        self.close_queue = CloseQueue::default();
    }

    /// The tabs in the order shown: pinned first, then the rest, each group in the order the
    /// user dragged them into (tabs not placed yet keep the app's order, at the end).
    fn ordered_tabs(&mut self, kind: AppKind, tabs: &[crate::hosts::Tab]) -> Vec<crate::hosts::Tab> {
        let order = self.tab_order.entry(kind).or_default();
        order.retain(|id| tabs.iter().any(|t| t.id == *id));
        for t in tabs {
            if !order.contains(&t.id) {
                order.push(t.id);
            }
        }
        let pos = |id: u64| order.iter().position(|x| *x == id).unwrap_or(usize::MAX);
        let mut list: Vec<crate::hosts::Tab> = tabs.to_vec();
        list.sort_by_key(|t| (!self.pinned_tabs.contains(&(kind, t.id)), pos(t.id)));
        list
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, rect: Rect, kind: AppKind, tabs: &[crate::hosts::Tab]) -> Option<TabAction> {
        let ordered = self.ordered_tabs(kind, tabs);
        let lang = self.lang;
        let v = ui.visuals().clone();
        let painter = ui.painter_at(rect);
        let strip = if v.dark_mode { v.extreme_bg_color.gamma_multiply(0.85) } else { v.faint_bg_color.lerp_to_gamma(Color32::BLACK, 0.05) };
        painter.rect_filled(rect, CornerRadius::ZERO, strip);
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color));
        let mut action = None;
        let plus_w = 34.0;
        let avail = (rect.width() - plus_w - 12.0).max(80.0);
        const PINNED_W: f32 = 44.0;
        let n_pinned = ordered.iter().filter(|t| self.pinned_tabs.contains(&(kind, t.id))).count();
        let rest = ordered.len() - n_pinned;
        let w = ((avail - n_pinned as f32 * PINNED_W) / rest.max(1) as f32).clamp(96.0, 232.0);
        let font = crate::theme::ui_font(ui.ctx(), 12.5);
        let all_ids: Vec<u64> = ordered.iter().map(|t| t.id).collect();
        // Slots: where each tab belongs.
        let mut slots = Vec::with_capacity(ordered.len());
        let mut x = rect.left() + 6.0;
        for tab in &ordered {
            let pinned = self.pinned_tabs.contains(&(kind, tab.id));
            let tw = if pinned { PINNED_W } else { w };
            slots.push((x, tw, pinned));
            x += tw;
        }
        // Dragging: the tab follows the pointer within its group, and the others make room.
        let pointer = ui.ctx().pointer_latest_pos();
        let dragging = self.tab_drag.filter(|(k, id, _)| *k == kind && all_ids.contains(id));
        if let (Some((_, did, grab)), Some(p)) = (dragging, pointer)
            && let Some(from) = all_ids.iter().position(|i| *i == did)
        {
            let pinned = slots[from].2;
            let center = p.x - grab + slots[from].1 / 2.0;
            let target = slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.2 == pinned)
                .min_by(|a, b| ((a.1.0 + a.1.1 / 2.0) - center).abs().total_cmp(&((b.1.0 + b.1.1 / 2.0) - center).abs()))
                .map_or(from, |(i, _)| i);
            if target != from {
                let mut ids = all_ids.clone();
                let id = ids.remove(from);
                ids.insert(target, id);
                self.tab_order.insert(kind, ids);
                ui.ctx().request_repaint();
            }
        }
        let mut drag_started = None;
        let mut drag_stopped = false;
        let mut draw_order: Vec<usize> = (0..ordered.len()).collect();
        if let Some((_, did, _)) = dragging
            && let Some(i) = all_ids.iter().position(|x| *x == did)
        {
            draw_order.retain(|k| *k != i);
            draw_order.push(i);
        }
        for index in draw_order {
            let tab = &ordered[index];
            let (slot_x, tw, pinned) = slots[index];
            let id = ui.scope_id().with(("themisio-tab", kind, tab.id));
            let is_dragged = dragging.is_some_and(|(_, d, _)| d == tab.id);
            let x = if is_dragged {
                let group: Vec<&(f32, f32, bool)> = slots.iter().filter(|s| s.2 == pinned).collect();
                let lo = group.first().map_or(slot_x, |s| s.0);
                let hi = group.last().map_or(slot_x, |s| s.0);
                let px = pointer.map_or(slot_x, |p| p.x - dragging.map_or(0.0, |d| d.2));
                let x = px.clamp(lo, hi);
                ui.ctx().animate_value_with_time(id.with("x"), x, 0.0)
            } else {
                ui.ctx().animate_value_with_time(id.with("x"), slot_x, 0.14)
            };
            let r = Rect::from_min_size(pos2(x, rect.top() + 5.0), vec2(tw - 4.0, rect.height() - 5.0));
            if r.left() > rect.right() - plus_w {
                continue;
            }
            let resp = ui.interact(r, id, Sense::click_and_drag());
            if resp.drag_started()
                && let Some(p) = resp.interact_pointer_pos()
            {
                drag_started = Some((tab.id, p.x - r.left()));
            }
            if resp.drag_stopped() {
                drag_stopped = true;
            }
            if resp.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            let hover = ui.ctx().animate_bool_with_time(id.with("h"), resp.hovered() || is_dragged, 0.12);
            let top = CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
            if is_dragged {
                painter.add(egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(50) }.as_shape(r, top));
                painter.rect_filled(r, top, v.panel_fill);
            }
            if tab.active {
                painter.rect_filled(r, top, v.panel_fill);
                painter.rect_filled(Rect::from_min_size(r.left_top() + vec2(10.0, 0.0), vec2(r.width() - 20.0, 2.0)), CornerRadius::same(1), kind.accent());
            } else if hover > 0.0 && !is_dragged {
                painter.rect_filled(r.shrink2(vec2(0.0, 2.0)), CornerRadius::same(7), v.widgets.hovered.weak_bg_fill.gamma_multiply(hover));
            }
            let text_color = if tab.active { v.strong_text_color() } else { v.text_color() };
            if pinned {
                // A pinned tab: the app's mark and the first letters of the name, no close button.
                let initials: String = tab.title.chars().filter(|c| !c.is_whitespace()).take(2).collect();
                painter.circle_filled(pos2(r.left() + 11.0, r.center().y), 3.5, kind.accent());
                painter.text(pos2(r.left() + 25.0, r.center().y), egui::Align2::CENTER_CENTER, initials, font.clone(), text_color);
                if tab.dirty {
                    painter.circle_filled(pos2(r.right() - 5.0, r.top() + 7.0), 2.5, text_color.gamma_multiply(0.8));
                }
                if resp.clicked() && !tab.active {
                    action = Some(TabAction::Activate(tab.id));
                }
                let resp = resp.on_hover_text(&tab.title);
                tab_menu(&resp, lang, tab.id, true, index + 1 < all_ids.len(), all_ids.len() > 1, &mut action);
                continue;
            }
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
            let resp = resp.on_hover_text(&tab.title);
            tab_menu(&resp, lang, tab.id, false, index + 1 < all_ids.len(), all_ids.len() > 1, &mut action);
        }
        if let Some((id, grab)) = drag_started {
            self.tab_drag = Some((kind, id, grab));
            // Dragging a tab also brings it to the front, as in Edge.
            if ordered.iter().any(|t| t.id == id && !t.active) {
                action = Some(TabAction::Activate(id));
            }
        }
        if drag_stopped || (self.tab_drag.is_some() && !ui.ctx().input(|i| i.pointer.any_down())) {
            self.tab_drag = None;
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
        let mut ai_changed = false;
        let (modal, style) = self.modal(ctx, "themisio-settings");
        let (cur_presentation, cur_renderer) = (self.settings.presentation, self.settings.renderer);
        let restart = move |p: Presentation, r: Renderer| p != cur_presentation || r != cur_renderer;
        let response = modal.show(ctx, |ui| {
            use crate::ai::kit;
            ui.set_style(style.clone());
            let size = vec2(880.0, 600.0);
            ui.set_width(size.x);
            ui.set_height(size.y);
            let full = ui.max_rect();
            let pal = kit::palette(ui);
            // The sidebar.
            let side = Rect::from_min_size(full.min, vec2(200.0, size.y));
            let mut page = self.settings_page;
            ui.scope_builder(UiBuilder::new().max_rect(side), |ui| {
                ui.horizontal(|ui| {
                    if let Some(logo) = crate::rail::logo(ui.ctx()) {
                        ui.add(egui::Image::from_texture(&logo).fit_to_exact_size(vec2(26.0, 26.0)));
                    }
                    ui.label(RichText::new(t(lang, "Settings")).font(crate::theme::bold_font(ui.ctx(), 17.0)).color(pal.text));
                });
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                let groups: [(&str, &[(SettingsPage, &'static str)]); 2] = [
                    ("Themisio", &[(SettingsPage::General, "General"), (SettingsPage::Performance, "Performance"), (SettingsPage::Files, "Files"), (SettingsPage::About, "About")]),
                    ("AI", &[(SettingsPage::Providers, "AI providers"), (SettingsPage::Features, "AI features"), (SettingsPage::Translation, "Translation"), (SettingsPage::Skills, "Skills")]),
                ];
                for (g, items) in groups {
                    ui.add_space(8.0);
                    ui.label(RichText::new(g).size(11.0).color(pal.muted));
                    ui.add_space(2.0);
                    for (pg, label) in items {
                        if kit::nav_item(ui, page == *pg, t(lang, label)).clicked() {
                            page = *pg;
                        }
                    }
                }
            });
            if page != self.settings_page {
                self.settings_page = page;
                self.ai_ui.refresh();
            }
            ui.painter().line_segment([pos2(side.right() + 12.0, full.top()), pos2(side.right() + 12.0, full.top() + size.y)], egui::Stroke::new(1.0, pal.border));
            // The page.
            let content = Rect::from_min_max(pos2(side.right() + 28.0, full.top()), pos2(full.left() + size.x, full.top() + size.y));
            ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
                ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
                    egui::ScrollArea::vertical().id_salt(("themisio-settings-page", self.settings_page as u8)).auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_width(ui.available_width() - 14.0);
                        match self.settings_page {
                            SettingsPage::General => {
                                kit::page_title(ui, t(lang, "General"), t(lang, "Language, look and the app bar."));
                                kit::card(ui, |ui| {
                                    kit::setting_row(ui, t(lang, "Language"), t(lang, "For Themisio and all four apps."), |ui| {
                                        segmented(ui, &mut language, &[(LangPref::System, t(lang, "Same as Windows")), (LangPref::En, "English"), (LangPref::Vi, "Tiếng Việt")]);
                                    });
                                    kit::row_rule(ui);
                                    kit::setting_row(ui, t(lang, "Theme"), "", |ui| {
                                        segmented(ui, &mut theme, &[(ThemePref::System, t(lang, "Same as Windows")), (ThemePref::Light, t(lang, "Light")), (ThemePref::Dark, t(lang, "Dark"))]);
                                    });
                                    kit::row_rule(ui);
                                    kit::setting_row(ui, t(lang, "App bar"), t(lang, "Auto-hide leaves a thin line at the window's left edge; point at it to show the bar."), |ui| {
                                        segmented(ui, &mut rail_mode, &[(RailMode::AutoHide, t(lang, "Auto-hide")), (RailMode::Pinned, t(lang, "Always show"))]);
                                    });
                                });
                            }
                            SettingsPage::Performance => {
                                kit::page_title(ui, t(lang, "Performance"), t(lang, "Scrolling, animation and how Themisio draws."));
                                kit::card(ui, |ui| {
                                    kit::setting_row(ui, t(lang, "Smooth scrolling"), t(lang, "Fluid (like Microsoft Edge)"), |ui| {
                                        segmented(ui, &mut smooth, &[(Smoothness::Off, t(lang, "Off")), (Smoothness::Standard, t(lang, "Standard")), (Smoothness::Fluid, t(lang, "Fluid"))]);
                                    });
                                    kit::row_rule(ui);
                                    kit::setting_row(ui, t(lang, "Animations"), t(lang, "Same as Windows (Animation effects)"), |ui| {
                                        segmented(ui, &mut motion, &[(Motion::System, t(lang, "Same as Windows")), (Motion::On, t(lang, "On")), (Motion::Off, t(lang, "Off"))]);
                                    });
                                    kit::row_rule(ui);
                                    kit::setting_row(ui, t(lang, "Rendering"), t(lang, "OpenGL uses the least memory."), |ui| {
                                        segmented(ui, &mut renderer, &[(Renderer::Gpu, "Direct3D 12"), (Renderer::OpenGl, "OpenGL")]);
                                    });
                                    kit::row_rule(ui);
                                    kit::setting_row(ui, t(lang, "Frame pacing"), t(lang, "Low-latency (higher power use)"), |ui| {
                                        segmented(ui, &mut presentation, &[(Presentation::Balanced, t(lang, "Balanced (VSync)")), (Presentation::LowLatency, t(lang, "Low latency"))]);
                                    });
                                });
                                if restart(presentation, renderer) {
                                    ui.add_space(8.0);
                                    kit::notice(ui, kit::AMBER, kit::Icon::Warning, |ui| {
                                        ui.label(t(lang, "Restart Themisio to apply this."));
                                    });
                                }
                            }
                            SettingsPage::Files => {
                                kit::page_title(ui, t(lang, "Files"), t(lang, "Open documents, presentations, workbooks and PDFs with Themisio"));
                                kit::card(ui, |ui| {
                                    kit::setting_row(ui, t(lang, "Register file types"), t(lang, "For your Windows account only; no administrator rights needed."), |ui| {
                                        if kit::primary(ui, t(lang, "Register")).clicked() {
                                            register = Some(crate::assoc::register());
                                        }
                                    });
                                    if cfg!(windows) {
                                        kit::row_rule(ui);
                                        kit::setting_row(ui, t(lang, "Default apps"), t(lang, "Then pick Themisio in Windows' Default apps."), |ui| {
                                            if kit::secondary(ui, t(lang, "Choose default apps…")).clicked() {
                                                crate::assoc::open_default_apps();
                                            }
                                        });
                                    }
                                });
                            }
                            SettingsPage::About => {
                                kit::page_title(ui, t(lang, "About"), "");
                                kit::card(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if let Some(logo) = crate::rail::logo(ui.ctx()) {
                                            ui.add(egui::Image::from_texture(&logo).fit_to_exact_size(vec2(56.0, 56.0)));
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(RichText::new("Themisio").font(crate::theme::bold_font(ui.ctx(), 18.0)).color(pal.text));
                                            ui.label(RichText::new(t(lang, "Make your documents glow.")).color(pal.muted));
                                            ui.label(RichText::new(format!("{} {}", t(lang, "Version"), crate::VERSION)).size(12.0).color(pal.muted));
                                        });
                                    });
                                    ui.add_space(8.0);
                                    if kit::secondary(ui, t(lang, "About Themisio")).clicked() {
                                        about = true;
                                    }
                                });
                            }
                            SettingsPage::Providers => ai_changed |= self.ai_ui.providers_page(ui, &mut self.settings.ai, lang),
                            SettingsPage::Features => ai_changed |= self.ai_ui.features_page(ui, &mut self.settings.ai, lang),
                            SettingsPage::Translation => ai_changed |= self.ai_ui.translation_page(ui, &mut self.settings.ai, lang),
                            SettingsPage::Skills => ai_changed |= self.ai_ui.skills_page(ui, &mut self.settings.ai, lang),
                        }
                        ui.add_space(12.0);
                    });
                });
            });
            let close_rect = Rect::from_min_size(pos2(content.right() - 30.0, content.top() - 4.0), vec2(28.0, 28.0));
            ui.scope_builder(UiBuilder::new().max_rect(close_rect), |ui| {
                if kit::icon_button(ui, kit::Icon::Close, t(lang, "Close")).clicked() {
                    open = false;
                }
            });
        });
        if response.should_close() {
            open = false;
        }
        if ai_changed {
            self.settings.save();
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

/// Open a folder in Explorer.
pub fn open_folder(path: &Path) {
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(not(windows))]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();
    if let Err(e) = result {
        log::warn!("open {}: {e}", path.display());
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
        if self.passes == 2
            && let Some(start) = crate::STARTED.get()
        {
            // The first frame has been presented: the window is on screen.
            log::info!("startup: first frame shown {} ms after main", start.elapsed().as_millis());
        }
        let now = ctx.input(|i| i.time);
        // An app asked to close the window (File › Exit, its save prompt finished, its last
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
        self.drain_close_queue(now);
        self.gather_notices(now);
        crate::deck_i18n::set_deck_vietnamese(self.translating_for(self.active));
        let translating = self.active.app().is_some();
        if let Some(kind) = self.active.app()
            && let Some(host) = self.hosts.get(kind)
        {
            let was = egui::themisio::set_translating(translating);
            let t0 = std::time::Instant::now();
            host.logic(ctx, frame);
            profile("logic", t0);
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

        // The AI pane docks at the right and the app gives it room.
        let dock_target = self.active.app().is_some_and(|k| self.dock_open(k));
        let dock_t = ctx.animate_bool_with_time(Id::unique("themisio-dock"), dock_target, 0.18);
        let dock_w = crate::ai::kit::PANE_W * ease_out(dock_t);
        let page_rect = if dock_w > 0.5 { Rect::from_min_max(content_rect.min, pos2(content_rect.right() - dock_w, content_rect.bottom())) } else { content_rect };
        let dock_rect = Rect::from_min_max(pos2(content_rect.right() - dock_w, content_rect.top()), pos2(content_rect.right() - dock_w + crate::ai::kit::PANE_W, content_rect.bottom()));

        // The page on screen.
        let mut child = ui.new_child(UiBuilder::new().max_rect(page_rect).id_salt(("themisio-page", self.active)));
        child.set_clip_rect(page_rect);
        match self.active {
            Page::Home => {
                if let Some(action) = self.home.show(&mut child, self.lang, &self.settings.recent) {
                    self.handle_home(action, &ctx, now);
                }
            }
            Page::App(kind) => {
                crate::deck_i18n::set_deck_vietnamese(self.translating_for(self.active));
                balance_type(&ctx);
                if let Some(host) = self.hosts.get(kind) {
                    let was = egui::themisio::set_translating(true);
                    let t0 = std::time::Instant::now();
                    host.ui(&mut child, frame);
                    profile("ui", t0);
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
                    self.tab_action(&ctx, kind, tabs, action, now);
                    // The last document closed: back to Home, as a browser with no tabs left.
                    let before = self.tab_count.insert(kind, tabs.len()).unwrap_or(0);
                    if tabs.is_empty() && before > 0 && self.closing.is_none() {
                        self.go(Page::Home, now);
                    }
                }
                let t0 = std::time::Instant::now();
                self.ai_frame(&ctx, kind, page_rect, (dock_w > 0.5).then_some(dock_rect), now);
                profile("ai", t0);
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
            let w = 3.0 + 2.0 * glow;
            let line = Rect::from_min_size(pos2(full.left(), full.top() + 6.0), vec2(w, full.height() - 12.0));
            let gold = crate::theme::GOLD_DEEP.lerp_to_gamma(crate::theme::GOLD, glow);
            p.rect_filled(line, CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, gold.gamma_multiply((0.9 + 0.1 * glow) * fade));
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
        let style = self.dialog_style(&ctx);
        // Notices sit beside the AI pane, not over it.
        let notice_rect = if dock_w > 0.5 { Rect::from_min_max(app_rect.min, pos2(app_rect.right() - dock_w, app_rect.bottom())) } else { app_rect };
        self.notices.ui(&ctx, notice_rect, now, &style);

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
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
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

/// A tab's right-click menu: Save, Save As, Print; Pin; Close, Close other tabs, Close tabs to
/// the right.
fn tab_menu(resp: &egui::Response, lang: Lang, id: u64, pinned: bool, has_right: bool, has_others: bool, action: &mut Option<TabAction>) {
    use crate::hosts::DocCommand;
    resp.context_menu(|ui| {
        ui.set_min_width(210.0);
        let mut item = |ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool, a: TabAction| {
            let button = egui::Button::new(label).shortcut_text(shortcut);
            if ui.add_enabled(enabled, button).clicked() {
                *action = Some(a);
                ui.close();
            }
        };
        item(ui, t(lang, "Save"), "Ctrl+S", true, TabAction::Command(id, DocCommand::Save));
        item(ui, t(lang, "Save As…"), "F12", true, TabAction::Command(id, DocCommand::SaveAs));
        item(ui, t(lang, "Print…"), "Ctrl+P", true, TabAction::Command(id, DocCommand::Print));
        ui.separator();
        item(ui, if pinned { t(lang, "Unpin tab") } else { t(lang, "Pin tab") }, "", true, TabAction::TogglePin(id));
        ui.separator();
        item(ui, t(lang, "Close tab"), "Ctrl+W", true, TabAction::Close(id));
        item(ui, t(lang, "Close other tabs"), "", has_others, TabAction::CloseOthers(id));
        item(ui, t(lang, "Close tabs to the right"), "", has_right, TabAction::CloseRight(id));
    });
}

/// A new document's name (Document1, Presentation2, Book1…), which needs no "Opened" notice.
fn looks_untitled(title: &str) -> bool {
    let t = title.trim().trim_end_matches(['*', ' ']);
    let stem: String = t.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let digits = t[stem.len()..].chars().all(|c| c.is_ascii_digit()) && t.len() > stem.len();
    digits
        && [
            "Document", "Presentation", "Book", "Workbook", "Untitled", "Tài liệu", "Bản trình bày", "Sổ làm việc", "Bảng tính", "Trình chiếu",
        ]
        .iter()
        .any(|w| stem.trim() == *w)
}

/// The apps' headings (dialog titles, page headings) at most this size, so dialogs over any app
/// keep the proportions of the rest of the interface instead of shouting their title.
/// Which AI pane is docked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DockKind {
    Chat,
    Writer,
}

/// With THEMISIO_PROFILE set, log the parts of a frame that take longer than 4 ms.
fn profile(what: &str, since: std::time::Instant) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("THEMISIO_PROFILE").is_some()) {
        let ms = since.elapsed().as_secs_f64() * 1000.0;
        if ms > 4.0 {
            log::info!("profile: {what} {ms:.1} ms");
        }
    }
}

const MAX_HEADING: f32 = 15.0;

/// Keep the apps' type in proportion (they set their styles when their theme changes; this
/// runs every pass, so it holds after any change).
fn balance_type(ctx: &egui::Context) {
    let too_big = ctx.global_style().text_styles.get(&egui::TextStyle::Heading).is_some_and(|f| f.size > MAX_HEADING);
    if too_big {
        ctx.all_styles_mut(|s| {
            if let Some(f) = s.text_styles.get_mut(&egui::TextStyle::Heading)
                && f.size > MAX_HEADING
            {
                f.size = MAX_HEADING;
            }
        });
    }
}
