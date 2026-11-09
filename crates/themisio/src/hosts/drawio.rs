//! draw.io (jgraph/drawio, Apache-2.0): the diagram editor's own web app, unchanged, with its whole
//! interface (menus, toolbar, shape libraries, format panel, templates), in a WebView2 inside
//! Themisio's window. It runs in draw.io's embed mode (`embed=1&proto=json`), the way
//! drawio-desktop and the integrations drive it: Themisio hands it a diagram, it reports changes
//! and saves, and Themisio reads and writes the files.
//!
//! The web app is carried inside themisio.exe (build.rs zips upstream/drawio/src/main/webapp) and
//! served to the webview from memory, so it works offline and never loads anything remote.
//! `drawio_bridge.js` stands in for the "parent window" draw.io talks to.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, vec2};
use serde_json::{Value, json};

use super::Host;
use crate::app_kind::AppKind;
use crate::i18n::{Lang, t, tf};
use crate::route::Intent;

/// How a diagram file is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    /// `.drawio`, `.dio`, `.xml`: the diagram XML.
    Xml,
    /// `.drawio.svg` / `.svg`: an SVG picture with the diagram inside.
    Svg,
    /// `.drawio.png` / `.png`: a PNG picture with the diagram inside.
    Png,
}

impl Format {
    fn of(path: &Path) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if name.ends_with(".svg") {
            Self::Svg
        } else if name.ends_with(".png") {
            Self::Png
        } else {
            Self::Xml
        }
    }
}

/// What a diagram starts with when its editor is ready.
enum Start {
    /// The diagram (XML, or a PNG/SVG data URL draw.io reads the diagram out of).
    Load(String),
    Blank,
    /// draw.io's template picker.
    Template,
}

/// A save waiting for draw.io to make the SVG or PNG.
struct PendingSave {
    path: PathBuf,
    close: bool,
}

struct Diagram {
    id: u64,
    path: Option<PathBuf>,
    title: String,
    start: Option<Start>,
    dirty: bool,
    ready: bool,
    /// The diagram as draw.io last reported it (each change is reported).
    xml: Option<String>,
    pending: Option<PendingSave>,
    /// Save asked from Themisio's tab menu as "Save as": the next save event asks for a name.
    save_as: bool,
    /// Close once this save is written.
    close_after_save: bool,
    #[cfg(windows)]
    view: Option<web::View>,
}

impl Diagram {
    fn name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| self.title.clone(), |n| n.to_string_lossy().into_owned())
    }
}

/// An unsaved-changes question, asked by Themisio itself while the editors are hidden.
enum Ask {
    /// Closing one diagram.
    Tab(u64),
    /// Closing draw.io (or Themisio).
    All,
}

pub struct DrawioHost {
    diagrams: Vec<Diagram>,
    active: Option<u64>,
    next: u64,
    untitled: u32,
    lang: Lang,
    dark: bool,
    notices: Vec<String>,
    covered: bool,
    ask: Option<Ask>,
    /// Closing draw.io once every diagram is saved or discarded.
    closing: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    tx: Sender<(u64, String)>,
    rx: Receiver<(u64, String)>,
    ctx: egui::Context,
    /// The page's rectangle last frame, for the editors' bounds.
    area: Option<egui::Rect>,
    #[cfg(windows)]
    env: web::Env,
}

impl DrawioHost {
    pub fn new(ctx: &egui::Context, lang: Lang) -> Self {
        let (tx, rx) = channel();
        Self {
            diagrams: Vec::new(),
            active: None,
            next: 1,
            untitled: 0,
            lang,
            dark: false,
            notices: Vec::new(),
            covered: false,
            ask: None,
            closing: false,
            tx,
            rx,
            ctx: ctx.clone(),
            area: None,
            #[cfg(windows)]
            env: web::Env::new(),
        }
    }

    fn add(&mut self, path: Option<PathBuf>, start: Start) {
        let id = self.next;
        self.next += 1;
        let title = match &path {
            Some(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            None => {
                self.untitled += 1;
                if self.untitled == 1 { "Untitled Diagram.drawio".to_string() } else { format!("Untitled Diagram {}.drawio", self.untitled) }
            }
        };
        self.diagrams.push(Diagram {
            id,
            path,
            title,
            start: Some(start),
            dirty: false,
            ready: false,
            xml: None,
            pending: None,
            save_as: false,
            close_after_save: false,
            #[cfg(windows)]
            view: None,
        });
        self.active = Some(id);
    }

    fn get(&mut self, id: u64) -> Option<&mut Diagram> {
        self.diagrams.iter_mut().find(|d| d.id == id)
    }

    /// Send draw.io a message (it reads them as its parent window's postMessage).
    fn post(&mut self, id: u64, msg: &Value) {
        #[cfg(windows)]
        if let Some(view) = self.get(id).and_then(|d| d.view.as_ref()) {
            view.deliver(&msg.to_string());
        }
        #[cfg(not(windows))]
        let _ = (id, msg);
    }

    fn file_dialog(&self, suggested: &str) -> Option<PathBuf> {
        let lang = self.lang;
        let stem = suggested.trim_end_matches(".png").trim_end_matches(".svg").trim_end_matches(".drawio").trim_end_matches(".dio").trim_end_matches(".xml").to_string();
        rfd::FileDialog::new()
            .set_title(t(lang, "Save diagram"))
            .set_file_name(format!("{stem}.drawio"))
            .add_filter(t(lang, "draw.io diagram"), &["drawio"])
            .add_filter(t(lang, "Editable SVG picture (.drawio.svg)"), &["svg"])
            .add_filter(t(lang, "Editable PNG picture (.drawio.png)"), &["png"])
            .add_filter("XML", &["xml"])
            .save_file()
            .map(|p| {
                // A picture keeps the diagram: name it .drawio.svg / .drawio.png, as draw.io does.
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let lower = name.to_ascii_lowercase();
                for ext in [".svg", ".png"] {
                    if lower.ends_with(ext) && !lower.ends_with(&format!(".drawio{ext}")) {
                        return p.with_file_name(format!("{}.drawio{ext}", &name[..name.len() - ext.len()]));
                    }
                }
                p
            })
    }

    /// Save diagram `id` (asking for a name when it has none, or for Save as).
    fn save(&mut self, id: u64, save_as: bool, close: bool) {
        let Some(d) = self.get(id) else { return };
        let known = d.path.clone().filter(|_| !save_as);
        let suggested = d.name();
        let path = match known {
            Some(p) => p,
            None => match self.file_dialog(&suggested) {
                Some(p) => p,
                None => return,
            },
        };
        match Format::of(&path) {
            Format::Xml => {
                let Some(d) = self.get(id) else { return };
                let Some(xml) = d.xml.clone() else {
                    // Nothing reported yet: the file as it was opened.
                    if d.path.as_ref() == Some(&path) {
                        self.saved(id, &path, close);
                    }
                    return;
                };
                match std::fs::write(&path, xml) {
                    Ok(()) => self.saved(id, &path, close),
                    Err(e) => self.failed(&path, &e.to_string()),
                }
            }
            format => {
                // draw.io makes the picture (with the diagram inside); the export event writes it.
                if let Some(d) = self.get(id) {
                    d.pending = Some(PendingSave { path, close });
                }
                let format = if format == Format::Svg { "xmlsvg" } else { "xmlpng" };
                self.post(id, &json!({ "action": "export", "format": format, "spinKey": "saving", "embedImages": true, "currentPage": false }));
            }
        }
    }

    fn saved(&mut self, id: u64, path: &Path, close: bool) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(d) = self.get(id) {
            d.path = Some(path.to_path_buf());
            d.title = name.clone();
            d.dirty = false;
        }
        self.post(id, &json!({ "action": "status", "messageKey": "allChangesSaved", "modified": false }));
        self.notices.push(tf(self.lang, "Saved {name}", &[("name", &name)]));
        if close {
            self.remove(id);
        }
    }

    fn failed(&mut self, path: &Path, error: &str) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.notices.push(tf(self.lang, "Couldn't save {name}: {error}", &[("name", &name), ("error", error)]));
    }

    fn remove(&mut self, id: u64) {
        let Some(i) = self.diagrams.iter().position(|d| d.id == id) else { return };
        self.diagrams.remove(i);
        if self.active == Some(id) {
            self.active = self.diagrams.get(i.min(self.diagrams.len().saturating_sub(1))).map(|d| d.id);
        }
    }

    /// Messages from the editors.
    fn pump(&mut self) {
        while let Ok((id, text)) = self.rx.try_recv() {
            let Ok(msg) = serde_json::from_str::<Value>(&text) else { continue };
            let event = msg.get("event").and_then(Value::as_str).unwrap_or_default().to_string();
            match event.as_str() {
                "init" => {
                    let Some(d) = self.get(id) else { continue };
                    d.ready = true;
                    let title = d.name();
                    let start = d.start.take().unwrap_or(Start::Blank);
                    let reply = match start {
                        Start::Load(xml) => json!({ "action": "load", "xml": xml, "autosave": 1, "title": title, "noExitBtn": 1 }),
                        Start::Blank => json!({ "action": "load", "xml": "", "autosave": 1, "title": title, "noExitBtn": 1 }),
                        Start::Template => json!({ "action": "template", "autosave": 1 }),
                    };
                    self.post(id, &reply);
                }
                "autosave" => {
                    if let Some(d) = self.get(id) {
                        d.xml = msg.get("xml").and_then(Value::as_str).map(str::to_string);
                        d.dirty = true;
                    }
                }
                "save" => {
                    let (save_as, close) = match self.get(id) {
                        Some(d) => {
                            d.xml = msg.get("xml").and_then(Value::as_str).map(str::to_string).or(d.xml.take());
                            (std::mem::take(&mut d.save_as), std::mem::take(&mut d.close_after_save))
                        }
                        None => continue,
                    };
                    self.save(id, save_as, close);
                }
                "export" => {
                    let Some(pending) = self.get(id).and_then(|d| d.pending.take()) else { continue };
                    if let Some(xml) = msg.get("xml").and_then(Value::as_str)
                        && let Some(d) = self.get(id)
                    {
                        d.xml = Some(xml.to_string());
                    }
                    let data = msg.get("data").and_then(Value::as_str).unwrap_or_default();
                    match data_url_bytes(data) {
                        Some(bytes) => match std::fs::write(&pending.path, bytes) {
                            Ok(()) => self.saved(id, &pending.path, pending.close),
                            Err(e) => self.failed(&pending.path, &e.to_string()),
                        },
                        None => self.failed(&pending.path, t(self.lang, "draw.io didn't make the picture")),
                    }
                }
                "exit" => self.close_tab(id),
                "themisio-open-url" => {
                    if let Some(url) = msg.get("url").and_then(Value::as_str)
                        && (url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:"))
                    {
                        open_link(url);
                    }
                }
                "themisio-key" => {
                    // Ctrl+Tab inside the editor: the next (or previous) diagram.
                    if msg.get("key").and_then(Value::as_str) == Some("Tab") && self.diagrams.len() > 1 {
                        let i = self.diagrams.iter().position(|d| Some(d.id) == self.active).unwrap_or(0);
                        let n = self.diagrams.len();
                        let j = if msg.get("shift").and_then(Value::as_bool).unwrap_or(false) { (i + n - 1) % n } else { (i + 1) % n };
                        self.active = Some(self.diagrams[j].id);
                    }
                }
                _ => {}
            }
            self.ctx.request_repaint();
        }
    }

    /// Themisio's own start page for draw.io: new diagram, from a template, open.
    fn start_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.lang;
        let p = crate::theme::Palette::of(ui.visuals().dark_mode);
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, p.bg);
        let accent = AppKind::Drawio.accent();
        let mut action = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(vec2((rect.width() - 760.0).max(48.0) / 2.0, 48.0))), |ui| {
            ui.label(RichText::new("draw.io").font(crate::theme::bold_font(ctx, 28.0)).color(p.text));
            ui.label(RichText::new(t(lang, "Flowcharts, org charts, network and UML diagrams, mind maps and more.")).size(14.0).color(p.muted));
            ui.add_space(24.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                for (i, (title, sub)) in [
                    (t(lang, "Blank diagram"), t(lang, "Start from an empty page")),
                    (t(lang, "From a template"), t(lang, "Flowcharts, UML, network, business…")),
                    (t(lang, "Open a diagram…"), t(lang, ".drawio, .drawio.svg, .drawio.png, .xml")),
                ]
                .into_iter()
                .enumerate()
                {
                    let (r, resp) = ui.allocate_exact_size(vec2(236.0, 150.0), Sense::click());
                    let fill = if resp.hovered() { p.surface_hover } else { p.surface };
                    ui.painter().rect(r, CornerRadius::same(12), fill, Stroke::new(1.0, if resp.hovered() { accent } else { p.border }), egui::StrokeKind::Inside);
                    let art = egui::Rect::from_min_size(r.min + vec2(16.0, 16.0), vec2(r.width() - 32.0, 72.0));
                    ui.painter().rect_filled(art, CornerRadius::same(8), accent.gamma_multiply(0.12));
                    paint_art(ui.painter(), art, i, accent);
                    ui.painter().text(r.left_bottom() + vec2(16.0, -38.0), egui::Align2::LEFT_CENTER, title, crate::theme::bold_font(ctx, 14.5), p.text);
                    ui.painter().text(r.left_bottom() + vec2(16.0, -18.0), egui::Align2::LEFT_CENTER, sub, egui::FontId::proportional(12.0), p.muted);
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        action = Some(i);
                    }
                }
            });
            ui.add_space(22.0);
            if !web_available() {
                ui.label(RichText::new(t(lang, "The draw.io editor runs on Windows 10 and 11 (Microsoft Edge WebView2).")).size(12.5).color(p.muted));
            } else if !bundle_present() {
                ui.label(RichText::new(t(lang, "draw.io isn't part of this build (scripts/fetch-upstream).")).size(12.5).color(crate::ai::kit::RED));
            }
        });
        match action {
            Some(0) => self.add(None, Start::Blank),
            Some(1) => self.add(None, Start::Template),
            Some(2) => {
                let lang = self.lang;
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(t(lang, "Open a diagram"))
                    .add_filter(t(lang, "Diagrams"), &["drawio", "dio", "svg", "png", "xml"])
                    .pick_file()
                {
                    self.open(ctx, &path, Intent::Edit);
                }
            }
            _ => {}
        }
    }

    /// The unsaved-changes question (the editors are hidden while it shows).
    fn ask_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ask) = &self.ask else { return };
        let lang = self.lang;
        let names: Vec<String> = match ask {
            Ask::Tab(id) => self.diagrams.iter().filter(|d| d.id == *id).map(Diagram::name).collect(),
            Ask::All => self.diagrams.iter().filter(|d| d.dirty).map(Diagram::name).collect(),
        };
        let p = crate::theme::Palette::of(ui.visuals().dark_mode);
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, p.bg);
        let mut answer = None;
        egui::Area::new(egui::Id::unique("themisio-drawio-ask")).order(egui::Order::Foreground).fixed_pos(rect.center() - vec2(210.0, 90.0)).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(18.0).corner_radius(CornerRadius::same(12)).show(ui, |ui| {
                ui.set_width(420.0);
                ui.label(RichText::new(t(lang, "Save your changes?")).font(crate::theme::bold_font(ui.ctx(), 16.0)).color(p.text));
                ui.add_space(6.0);
                ui.label(RichText::new(tf(lang, "{names} has changes that aren't saved.", &[("names", &names.join(", "))])).size(13.0).color(p.muted));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if crate::ai::kit::primary(ui, t(lang, "Save")).clicked() {
                        answer = Some(0);
                    }
                    if crate::ai::kit::secondary(ui, t(lang, "Don't save")).clicked() {
                        answer = Some(1);
                    }
                    if crate::ai::kit::quiet(ui, t(lang, "Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        answer = Some(2);
                    }
                });
            });
        });
        let Some(answer) = answer else { return };
        let ask = self.ask.take();
        let ids: Vec<u64> = match ask {
            Some(Ask::Tab(id)) => vec![id],
            Some(Ask::All) => self.diagrams.iter().filter(|d| d.dirty).map(|d| d.id).collect(),
            None => Vec::new(),
        };
        match answer {
            0 => {
                for id in ids {
                    self.save(id, false, true);
                }
                // A name not given (Save dialog cancelled) keeps the diagram open and stops closing.
                if self.diagrams.iter().any(|d| d.dirty && d.pending.is_none()) {
                    self.closing = false;
                }
            }
            1 => {
                for id in ids {
                    self.remove(id);
                }
            }
            _ => self.closing = false,
        }
    }

    #[cfg(windows)]
    fn place_views(&mut self, frame: &eframe::Frame, area: Option<egui::Rect>) {
        let ppp = self.ctx.pixels_per_point();
        let show = !self.covered && self.ask.is_none();
        let lang = self.lang;
        let dark = self.dark;
        let active = self.active;
        for d in &mut self.diagrams {
            let visible = show && Some(d.id) == active && area.is_some();
            if d.view.is_none() && visible && let Some(area) = area {
                match web::View::new(frame, &mut self.env, d.id, &url(lang, dark), self.tx.clone(), self.ctx.clone(), area, ppp) {
                    Ok(v) => d.view = Some(v),
                    Err(e) => {
                        log::error!("draw.io: webview: {e}");
                        self.notices.push(tf(lang, "draw.io couldn't start: {error}", &[("error", &e)]));
                        d.start = None;
                        continue;
                    }
                }
            }
            if let Some(view) = &mut d.view {
                view.show(visible, area, ppp);
            }
        }
        // Clicks in Themisio (tab bar, rail) take the keyboard back from the editor.
        if self.ctx.input(|i| i.pointer.any_pressed())
            && let Some(view) = self.diagrams.iter().find(|d| Some(d.id) == active).and_then(|d| d.view.as_ref())
        {
            view.focus_parent();
        }
    }
}

/// draw.io's address with the options Themisio runs it with (see the module docs).
#[cfg_attr(not(windows), allow(dead_code))]
fn url(lang: Lang, dark: bool) -> String {
    let lang = if lang == Lang::Vi { "vi" } else { "en" };
    format!(
        "{}index.html?embed=1&proto=json&spin=1&libraries=1&noExitBtn=1&saveAndExit=0&noSaveBtn=0&modified=unsavedChanges&keepmodified=1&ui=kennedy&lang={lang}&offline=1&stealth=1&pwa=0&gapi=0&db=0&od=0&gh=0&gl=0&tr=0&mode=device&configure=0&dark={}",
        web_root(),
        if dark { 1 } else { 0 }
    )
}

fn web_root() -> &'static str {
    "http://drawio.localhost/"
}

fn web_available() -> bool {
    cfg!(windows)
}

fn bundle_present() -> bool {
    #[cfg(windows)]
    {
        web::BUNDLE.len() > 64
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn open_link(url: &str) {
    #[cfg(windows)]
    {
        let _ = open::that_detached(url);
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// The little pictures on the start page's cards.
fn paint_art(p: &egui::Painter, r: egui::Rect, which: usize, accent: Color32) {
    let s = Stroke::new(1.6, accent);
    let c = r.center();
    match which {
        0 => {
            let page = egui::Rect::from_center_size(c, vec2(44.0, 54.0));
            p.rect_stroke(page, CornerRadius::same(4), s, egui::StrokeKind::Middle);
            p.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], s);
            p.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], s);
        }
        1 => {
            let a = egui::Rect::from_center_size(c - vec2(46.0, 14.0), vec2(40.0, 22.0));
            let b = egui::Rect::from_center_size(c + vec2(46.0, -14.0), vec2(40.0, 22.0));
            let d = egui::Rect::from_center_size(c + vec2(0.0, 20.0), vec2(40.0, 22.0));
            for box_ in [a, b, d] {
                p.rect(box_, CornerRadius::same(4), accent.gamma_multiply(0.25), s, egui::StrokeKind::Middle);
            }
            p.line_segment([a.right_center(), b.left_center()], s);
            p.line_segment([a.center_bottom(), d.left_center()], s);
            p.line_segment([b.center_bottom(), d.right_center()], s);
        }
        _ => {
            let folder = egui::Rect::from_center_size(c + vec2(0.0, 4.0), vec2(60.0, 40.0));
            p.rect_stroke(folder, CornerRadius::same(4), s, egui::StrokeKind::Middle);
            p.line_segment([folder.left_top() + vec2(4.0, 0.0), folder.left_top() + vec2(18.0, -8.0)], s);
            p.line_segment([folder.left_top() + vec2(18.0, -8.0), folder.left_top() + vec2(30.0, -8.0)], s);
            p.line_segment([folder.left_top() + vec2(30.0, -8.0), folder.left_top() + vec2(34.0, 0.0)], s);
        }
    }
}

/// The bytes of a `data:...;base64,` URL (or of a plain `data:image/svg+xml,` one).
fn data_url_bytes(url: &str) -> Option<Vec<u8>> {
    let (head, body) = url.strip_prefix("data:")?.split_once(',')?;
    if head.ends_with(";base64") {
        base64_decode(body)
    } else {
        Some(percent_decode(body).into_bytes())
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in s.as_bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl Host for DrawioHost {
    fn kind(&self) -> AppKind {
        AppKind::Drawio
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump();
        // Themisio (or draw.io from the rail) is closing: unsaved diagrams ask first.
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.diagrams.iter().any(|d| d.dirty) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.closing = true;
                self.ask = Some(Ask::All);
            } else {
                self.diagrams.clear();
                self.active = None;
            }
        }
        if self.closing && self.ask.is_none() && self.diagrams.iter().all(|d| !d.dirty) && self.diagrams.iter().all(|d| d.pending.is_none()) {
            self.closing = false;
            self.diagrams.clear();
            self.active = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.diagrams.iter().any(|d| d.view_pending()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.set_visuals(if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() });
        let rect = ui.max_rect();
        let area = (self.active.is_some() && self.ask.is_none()).then_some(rect);
        if self.ask.is_some() {
            self.ask_ui(ui);
        } else if self.diagrams.is_empty() {
            self.start_page(ui, &ctx);
        } else {
            // Under the editor (seen while it starts): the page colour and a spinner.
            let p = crate::theme::Palette::of(self.dark);
            ui.painter().rect_filled(rect, CornerRadius::ZERO, if self.dark { Color32::from_rgb(0x12, 0x12, 0x12) } else { p.bg });
            if !web_available() || !bundle_present() {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(48.0)), |ui| {
                    let msg = if web_available() { t(self.lang, "draw.io isn't part of this build (scripts/fetch-upstream).") } else { t(self.lang, "The draw.io editor runs on Windows 10 and 11 (Microsoft Edge WebView2).") };
                    ui.label(RichText::new(msg).size(14.0).color(p.muted));
                });
            } else if self.diagrams.iter().any(|d| Some(d.id) == self.active && !d.ready) {
                ui.put(egui::Rect::from_center_size(rect.center(), vec2(32.0, 32.0)), egui::Spinner::new().size(28.0));
            }
        }
        self.area = area;
        #[cfg(windows)]
        self.place_views(frame, area);
        #[cfg(not(windows))]
        let _ = frame;
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw: &mut egui::RawInput) {}

    fn open(&mut self, _ctx: &egui::Context, path: &Path, _intent: Intent) {
        if let Some(d) = self.diagrams.iter().find(|d| d.path.as_deref() == Some(path)) {
            self.active = Some(d.id);
            return;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.notices.push(tf(self.lang, "Couldn't open {name}: {error}", &[("name", &name), ("error", &e.to_string())]));
                return;
            }
        };
        let data = match Format::of(path) {
            Format::Png => format!("data:image/png;base64,{}", base64_encode(&bytes)),
            Format::Svg => format!("data:image/svg+xml;base64,{}", base64_encode(&bytes)),
            Format::Xml => String::from_utf8_lossy(&bytes).into_owned(),
        };
        // A blank untouched diagram gives way to the file, as in the other apps.
        if self.diagrams.len() == 1 && self.diagrams[0].path.is_none() && !self.diagrams[0].dirty && self.diagrams[0].xml.is_none() {
            self.diagrams.clear();
        }
        self.add(Some(path.to_path_buf()), Start::Load(data));
    }

    fn new_document(&mut self, _ctx: &egui::Context) {
        self.add(None, Start::Blank);
    }

    fn set_language(&mut self, lang: Lang) {
        self.lang = lang;
    }

    fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    fn set_covered(&mut self, covered: bool) {
        self.covered = covered;
        // Hide at once: the page may not be drawn again (another page is shown now).
        #[cfg(windows)]
        if covered {
            for d in &mut self.diagrams {
                if let Some(view) = &mut d.view {
                    view.show(false, None, 1.0);
                }
            }
        }
    }

    fn tabs(&self) -> Option<Vec<super::Tab>> {
        Some(self.diagrams.iter().map(|d| super::Tab { id: d.id, title: d.name(), dirty: d.dirty, active: Some(d.id) == self.active }).collect())
    }

    fn activate_tab(&mut self, id: u64) {
        if self.diagrams.iter().any(|d| d.id == id) {
            self.active = Some(id);
        }
    }

    fn close_tab(&mut self, id: u64) {
        match self.diagrams.iter().find(|d| d.id == id) {
            Some(d) if d.dirty => {
                self.active = Some(id);
                self.ask = Some(Ask::Tab(id));
            }
            Some(_) => self.remove(id),
            None => {}
        }
    }

    fn command(&mut self, cmd: super::DocCommand) -> Result<(), String> {
        let Some(id) = self.active else { return Err("no diagram".into()) };
        match cmd {
            super::DocCommand::Save | super::DocCommand::SaveAs => {
                let ready = self.get(id).is_some_and(|d| d.ready);
                if ready {
                    // draw.io's own Save: it hands over the diagram, then Themisio writes it.
                    if let Some(d) = self.get(id) {
                        d.save_as = cmd == super::DocCommand::SaveAs;
                    }
                    self.post(id, &json!({ "action": "invokeAction", "actionName": "save" }));
                } else {
                    self.save(id, cmd == super::DocCommand::SaveAs, false);
                }
            }
            super::DocCommand::Print => self.post(id, &json!({ "action": "invokeAction", "actionName": "print" })),
        }
        Ok(())
    }

    fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    fn on_exit(&mut self) {
        self.diagrams.clear();
    }
}

impl Diagram {
    fn view_pending(&self) -> bool {
        !self.ready && self.start.is_some()
    }
}

#[cfg(windows)]
mod web {
    //! The WebView2 each diagram's editor runs in.

    use std::borrow::Cow;
    use std::io::Read;
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};

    use wry::http::{Request, Response, header};
    use wry::{WebContext, WebView, WebViewBuilder};

    /// draw.io's web app, zipped by build.rs.
    pub static BUNDLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/drawio.zip"));

    type Archive = zip::ZipArchive<std::io::Cursor<&'static [u8]>>;

    fn archive() -> Option<&'static Mutex<Archive>> {
        static ARCHIVE: OnceLock<Option<Mutex<Archive>>> = OnceLock::new();
        ARCHIVE.get_or_init(|| zip::ZipArchive::new(std::io::Cursor::new(BUNDLE)).ok().map(Mutex::new)).as_ref()
    }

    fn mime(path: &str) -> &'static str {
        match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
            "html" | "htm" => "text/html; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "json" => "application/json",
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "ico" => "image/x-icon",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "xml" => "application/xml",
            "txt" => "text/plain; charset=utf-8",
            "wasm" => "application/wasm",
            _ => "application/octet-stream",
        }
    }

    /// A file of the web app, or 404.
    fn serve(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
        let path = request.uri().path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };
        let path = super::percent_decode(path);
        let found = archive().and_then(|a| {
            let mut a = a.lock().ok()?;
            let mut file = a.by_name(&path).ok()?;
            let mut bytes = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut bytes).ok()?;
            Some(bytes)
        });
        let empty = || Response::new(Cow::Borrowed(&[][..]));
        match found {
            Some(bytes) => {
                let body: Cow<'static, [u8]> = Cow::Owned(bytes);
                Response::builder()
                    .status(200)
                    .header(header::CONTENT_TYPE, mime(&path))
                    .header(header::CACHE_CONTROL, "no-cache")
                    .body(body)
                    .unwrap_or_else(|_| empty())
            }
            None => {
                let body: Cow<'static, [u8]> = Cow::Borrowed(&[][..]);
                Response::builder().status(404).body(body).unwrap_or_else(|_| empty())
            }
        }
    }

    pub struct Env {
        context: WebContext,
    }

    impl Env {
        pub fn new() -> Self {
            // The editor's own storage (its settings, recent colours, libraries) in Themisio's
            // folder, not beside themisio.exe.
            let dir = std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("Themisio").join("WebView2"));
            if let Some(d) = &dir {
                let _ = std::fs::create_dir_all(d);
            }
            Self { context: WebContext::new(dir) }
        }
    }

    pub struct View {
        view: WebView,
        visible: bool,
        bounds: Option<egui::Rect>,
    }

    fn bounds(area: egui::Rect, ppp: f32) -> wry::Rect {
        let x = (area.left() * ppp).round();
        let y = (area.top() * ppp).round();
        let w = (area.width() * ppp).round().max(1.0);
        let h = (area.height() * ppp).round().max(1.0);
        wry::Rect { position: wry::dpi::PhysicalPosition::new(x as i32, y as i32).into(), size: wry::dpi::PhysicalSize::new(w as u32, h as u32).into() }
    }

    impl View {
        #[allow(clippy::too_many_arguments)]
        pub fn new(frame: &eframe::Frame, env: &mut Env, id: u64, url: &str, tx: Sender<(u64, String)>, ctx: egui::Context, area: egui::Rect, ppp: f32) -> Result<Self, String> {
            let bridge = include_str!("drawio_bridge.js");
            let view = WebViewBuilder::new_with_web_context(&mut env.context)
                .with_custom_protocol("drawio".into(), |_id, request| serve(&request))
                .with_initialization_script(bridge)
                .with_ipc_handler(move |request: Request<String>| {
                    let _ = tx.send((id, request.body().clone()));
                    ctx.request_repaint();
                })
                .with_navigation_handler(|url: String| url.starts_with(super::web_root()) || url.starts_with("about:") || url.starts_with("data:") || url.starts_with("blob:"))
                .with_new_window_req_handler(|url: String, _features| {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        super::open_link(&url);
                    }
                    wry::NewWindowResponse::Deny
                })
                .with_bounds(bounds(area, ppp))
                .with_focused(true)
                .with_devtools(cfg!(debug_assertions))
                .with_url(url)
                .build_as_child(frame)
                .map_err(|e| e.to_string())?;
            Ok(Self { view, visible: true, bounds: Some(area) })
        }

        /// Hand draw.io a message.
        pub fn deliver(&self, json: &str) {
            let arg = serde_json::to_string(json).unwrap_or_default();
            if let Err(e) = self.view.evaluate_script(&format!("window.__themisio && window.__themisio.deliver({arg});")) {
                log::warn!("draw.io: deliver: {e}");
            }
        }

        pub fn show(&mut self, visible: bool, area: Option<egui::Rect>, ppp: f32) {
            if visible != self.visible {
                let _ = self.view.set_visible(visible);
                self.visible = visible;
                if !visible {
                    let _ = self.view.focus_parent();
                }
            }
            if visible
                && let Some(area) = area
                && self.bounds != Some(area)
            {
                let _ = self.view.set_bounds(bounds(area, ppp));
                self.bounds = Some(area);
            }
        }

        pub fn focus_parent(&self) {
            let _ = self.view.focus_parent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "a", "ab", "abc", "abcd", "draw.io ✓"] {
            assert_eq!(base64_decode(&base64_encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(data_url_bytes("data:image/svg+xml;base64,PHN2Zy8+").unwrap(), b"<svg/>");
    }

    #[test]
    fn formats() {
        assert_eq!(Format::of(Path::new("a.drawio.svg")), Format::Svg);
        assert_eq!(Format::of(Path::new("a.drawio.png")), Format::Png);
        assert_eq!(Format::of(Path::new("a.drawio")), Format::Xml);
        assert_eq!(Format::of(Path::new("a.xml")), Format::Xml);
    }
}
