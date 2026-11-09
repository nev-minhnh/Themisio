//! GridCraft, set up as `upstream/gridcraft/apps/gridcraft/src/main.rs` sets it up: one window
//! per workbook (`DocWindows`), the first in Themisio's window, others in windows of their own.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gridcraft_engine::Session;
use gridcraft_ui_egui::{Services, SheetApp};

use super::{Host, path_text};
use super::grid_src::print;
use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;


pub struct GridHost {
    app: SheetApp,
    language: Option<Lang>,
}

fn config_dir() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("GridCraft"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
            .map(|c| c.join("gridcraft"))
    }
}

fn prefs_enabled() -> bool {
    std::env::var_os("GRIDCRAFT_NO_PREFS").is_none()
}

fn load_prefs(app: &mut SheetApp) {
    if !prefs_enabled() {
        return;
    }
    app.ui.language = gridcraft_ui_egui::i18n::Language::system();
    app.ui.system_theme = true;
    let Some(dir) = config_dir() else { return };
    if let Ok(b) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<gridcraft_ui_egui::UiState>(&b)
    {
        let saved = serde_json::from_slice(&b).ok().and_then(|v| gridcraft_ui_egui::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui.at_startup();
        app.ui.language = language;
    }
    if let Ok(b) = std::fs::read(dir.join("prefs.json"))
        && let Ok(p) = serde_json::from_slice(&b)
    {
        app.session.prefs = p;
    }
}

fn save_prefs(app: &SheetApp) {
    if !prefs_enabled() {
        return;
    }
    let Some(dir) = config_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(b) = serde_json::to_vec_pretty(&app.ui) {
        let _ = std::fs::write(dir.join("ui.json"), b);
    }
    if let Ok(b) = serde_json::to_vec_pretty(&app.session.prefs) {
        let _ = std::fs::write(dir.join("prefs.json"), b);
    }
}

fn read_clipboard<T>(
    handle: &RefCell<Option<arboard::Clipboard>>,
    read: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    let mut clipboard = handle.try_borrow_mut().ok()?;
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    read(clipboard.as_mut()?).ok()
}

fn services() -> Services {
    let clipboard: Rc<RefCell<Option<arboard::Clipboard>>> = Rc::default();
    let (html_clipboard, text_clipboard, image_clipboard) = (clipboard.clone(), clipboard.clone(), clipboard.clone());
    Services {
        copy_html: Some(Box::new(move |html, text| {
            let mut clipboard = clipboard.borrow_mut();
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
            }
            clipboard.as_mut().ok_or("clipboard unavailable")?.set_html(html, Some(text)).map_err(|e| e.to_string())
        })),
        paste_html: Some(Box::new(move || read_clipboard(&html_clipboard, |clipboard| clipboard.get().html()))),
        paste_text: Some(Box::new(move || read_clipboard(&text_clipboard, |clipboard| clipboard.get_text()))),
        paste_image: Some(Box::new(move || {
            let image = read_clipboard(&image_clipboard, |clipboard| clipboard.get_image())?;
            image::RgbaImage::from_raw(u32::try_from(image.width).ok()?, u32::try_from(image.height).ok()?, image.bytes.into_owned())
        })),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Spreadsheets", &["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "slk", "json"])
                .add_filter("Excel Workbook", &["xlsx", "xlsm"])
                .add_filter("Excel 97-2003 Workbook (import)", &["xls"])
                .add_filter("Excel Binary Workbook (data import)", &["xlsb"])
                .add_filter("OpenDocument Spreadsheet (data import)", &["ods"])
                .add_filter("CSV", &["csv"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Pictures (PNG, JPEG)", &["png", "jpg", "jpeg"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let path = std::path::Path::new(suggested);
            let stem = path.file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("Book1").to_string();
            const TYPES: [(&str, &str); 4] =
                [("Excel Workbook (.xlsx)", "xlsx"), ("CSV UTF-8 (.csv)", "csv"), ("Web Page (.html)", "html"), ("PDF (.pdf)", "pdf")];
            let ext = path.extension().and_then(std::ffi::OsStr::to_str).map(str::to_ascii_lowercase).filter(|e| TYPES.iter().any(|(_, x)| x == e));
            let ext = ext.unwrap_or_else(|| "xlsx".to_string());
            let mut dialog = rfd::FileDialog::new().set_file_name(format!("{stem}.{ext}"));
            for (label, x) in TYPES.iter().filter(|(_, x)| *x == ext).chain(TYPES.iter().filter(|(_, x)| *x != ext)) {
                dialog = dialog.add_filter(*label, &[*x]);
            }
            dialog.save_file().and_then(|p| p.to_str().map(str::to_string))
        })),
        open_url: Some(Box::new(|url: &str| open_with_system(url))),
        printers: Some(print::discover()),
        print_native: Some(print::submit()),
        ..Default::default()
    }
}

/// Open an http(s) or mailto link in the default browser or mail app; nothing else.
fn open_with_system(target: &str) {
    let gridcraft_engine::links::LinkKind::External(target) = gridcraft_engine::links::classify(target) else {
        log::warn!("refused to open {target:?}: only http, https and mailto links are opened");
        return;
    };
    let target = target.as_str();
    let cmd = if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(cmd).arg(target).spawn() {
        log::warn!("{cmd} {target}: {e}");
    }
}

fn grid_language(lang: Lang) -> gridcraft_ui_egui::i18n::Language {
    match lang {
        Lang::En => gridcraft_ui_egui::i18n::Language::En,
        Lang::Vi => gridcraft_ui_egui::i18n::Language::Vi,
    }
}

impl GridHost {
    pub fn new(ctx: &egui::Context, lang: Lang) -> Self {
        let mut app = SheetApp::new(Session::new(), services());
        app.watch_desktop_theme();
        load_prefs(&mut app);
        app.ui.language = grid_language(lang);
        if app.session.documents().is_empty() {
            app.session.new_workbook();
        }
        app.after_engine();
        SheetApp::setup_context_for_language(ctx, app.ui.dark, app.ui.language);
        // One window that shows the active workbook (as the web build does); Themisio's tab bar
        // switches between the open workbooks, instead of a hidden native window for each.
        app.native_windows = false;
        Self { app, language: None }
    }
}

impl Host for GridHost {
    fn kind(&self) -> AppKind {
        AppKind::Grid
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(lang) = self.language.take() {
            self.app.ui.language = grid_language(lang);
        }
        self.app.logic(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && self.app.printing.busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(ctx, raw);
        // GridCraft's main opens files dropped on any of its windows.
        for f in std::mem::take(&mut raw.dropped_files) {
            if let Some(p) = f.path().to_str().map(str::to_string).filter(|p| !p.is_empty()) {
                self.app.open_path(&p);
            }
        }
    }

    fn open(&mut self, _ctx: &egui::Context, path: &Path, _intent: Intent) {
        self.app.open_path(&path_text(path));
    }

    fn new_document(&mut self, _ctx: &egui::Context) {
        if let Err(e) = self.app.session.execute("file.new", serde_json::json!({})) {
            log::warn!("GridCraft file.new: {e}");
        }
        self.app.after_engine();
    }

    fn set_language(&mut self, lang: Lang) {
        self.language = Some(lang);
    }

    fn set_dark(&mut self, dark: bool) {
        self.app.ui.system_theme = false;
        self.app.ui.dark = dark;
    }

    fn tabs(&self) -> Option<Vec<super::Tab>> {
        let list = self.app.session.window_list();
        let tabs = list
            .as_array()?
            .iter()
            .filter(|w| !w.get("hidden").and_then(serde_json::Value::as_bool).unwrap_or(false))
            .filter_map(|w| {
                Some(super::Tab {
                    id: w.get("window")?.as_u64()?,
                    title: w.get("title").and_then(serde_json::Value::as_str).unwrap_or("Book").to_string(),
                    dirty: w.get("dirty").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    active: w.get("active").and_then(serde_json::Value::as_bool).unwrap_or(false),
                })
            })
            .collect();
        Some(tabs)
    }

    fn command(&mut self, cmd: super::DocCommand) -> Result<(), String> {
        let id = match cmd {
            super::DocCommand::Save => "file.save",
            super::DocCommand::SaveAs => "file.saveAs",
            super::DocCommand::Print => "file.print",
        };
        self.app.run(id, serde_json::json!({})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn take_notices(&mut self) -> Vec<String> {
        self.app.toast.take().map(|t| t.0).into_iter().collect()
    }

    fn ai_target(&mut self) -> Option<super::AiTarget> {
        let v = self.app.run("cell.get", serde_json::json!({})).ok()?;
        if v["type"] != "text" || v["formula"].is_string() {
            return None;
        }
        let text = v["input"].as_str().or_else(|| v["text"].as_str())?.to_string();
        if text.trim().is_empty() {
            return None;
        }
        Some(super::AiTarget { text, selection: false, anchor: serde_json::json!({"cell": v["cell"]}), screen: None })
    }

    fn ai_replace(&mut self, target: &super::AiTarget, original: &str, replacement: &str) -> Result<(), String> {
        let cell = target.anchor["cell"].as_str().ok_or("no cell")?.to_string();
        let v = self.app.run("cell.get", serde_json::json!({"cell": cell})).map_err(|e| e.to_string())?;
        let input = v["input"].as_str().unwrap_or("").to_string();
        if !input.contains(original) {
            return Err("The cell has changed since it was checked".into());
        }
        let new = input.replacen(original, replacement, 1);
        self.app.run("cell.set", serde_json::json!({"cell": cell, "input": new})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn ai_replace_all(&mut self, target: &super::AiTarget, text: &str) -> Result<(), String> {
        let cell = target.anchor["cell"].as_str().ok_or("no cell")?;
        self.app.run("cell.set", serde_json::json!({"cell": cell, "input": text})).map(|_| ()).map_err(|e| e.to_string())
    }

    fn sheet_context(&mut self) -> Option<String> {
        const ROWS: usize = 200;
        const COLS: usize = 30;
        let (title, sheets, active, selection) = {
            let d = self.app.session.active()?;
            let names: Vec<String> = d.wb.sheets.iter().map(|s| s.name.clone()).collect();
            let active = d.wb.active().map(|s| s.name.clone()).unwrap_or_default();
            let sel: Vec<String> = d.selection.ranges.iter().map(|r| r.a1()).collect();
            let title = d.path.as_deref().map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()).unwrap_or_else(|| "Untitled workbook".into());
            (title, names, active, sel.join(", "))
        };
        let mut out = format!("Workbook: {title}\nSheets: {}\nActive sheet: {active}\nSelection: {selection}\n", sheets.join(", "));
        let table = |v: &serde_json::Value| -> String {
            let mut s = String::new();
            for row in v["values"].as_array().into_iter().flatten().take(ROWS) {
                let cells: Vec<String> = row
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(COLS)
                    .map(|c| match c {
                        serde_json::Value::Null => String::new(),
                        serde_json::Value::String(t) => t.replace(['\t', '\n'], " "),
                        other => other.to_string(),
                    })
                    .collect();
                s.push_str(&cells.join("\t"));
                s.push('\n');
            }
            s
        };
        let multi = selection.contains(':');
        if multi
            && let Ok(v) = self.app.run("sheet.read", serde_json::json!({"range": selection.split(", ").last().unwrap_or("")}))
        {
            out.push_str(&format!("\nThe selection ({}) as tab-separated values:\n{}", v["range"].as_str().unwrap_or(""), table(&v)));
        }
        match self.app.run("sheet.read", serde_json::json!({})) {
            Ok(v) if v["range"].is_string() => {
                let rows = v["values"].as_array().map_or(0, Vec::len);
                out.push_str(&format!(
                    "\nThe active sheet's used range {} ({rows} rows{}), tab-separated, first row = row {} of the sheet:\n{}",
                    v["range"].as_str().unwrap_or(""),
                    if rows > ROWS { format!(", first {ROWS} shown") } else { String::new() },
                    v["range"].as_str().and_then(|r| r.trim_start_matches(|c: char| c.is_ascii_alphabetic()).split(':').next()).unwrap_or("1"),
                    table(&v)
                ));
            }
            _ => out.push_str("\nThe active sheet is empty.\n"),
        }
        Some(out)
    }

    fn command_docs(&mut self, ids: &[&str]) -> String {
        self.app
            .session
            .commands()
            .into_iter()
            .filter(|c| ids.contains(&c.id))
            .map(|c| format!("- {} ({}): {}", c.id, c.label, c.params))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn run_json(&mut self, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        self.app.run(id, params).map_err(|e| e.to_string())
    }

    fn activate_tab(&mut self, id: u64) {
        if let Err(e) = self.app.run("window.activate", serde_json::json!({"window": id})) {
            log::warn!("CalcSheet: switch to {id}: {e}");
        }
    }

    fn close_tab(&mut self, id: u64) {
        self.app.run_or_alert("window.close", serde_json::json!({"window": id}));
    }

    fn on_exit(&mut self) {
        print::finish_pending_jobs();
        save_prefs(&self.app);
    }
}
