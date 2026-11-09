//! PdfCraft, set up as `upstream/pdfcraft/apps/pdfcraft/src/main.rs` sets it up. PdfCraft keeps
//! its preferences in eframe's storage under its own key, here in Themisio's `app.ron`.

use std::path::Path;

use pdfcraft_ui_egui::PdfCraftApp;

use super::{Host, path_text};
#[cfg(target_os = "windows")]
use super::pdf_src::motion;
use super::pdf_src::text_scale;
use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;

/// The key PdfCraft's settings are saved under (as in the standalone app), and its former name.
const STORAGE_KEY: &str = "pdfcraft";
const LEGACY_STORAGE_KEY: &str = "printcraft";

pub struct PdfHost {
    app: PdfCraftApp,
}

impl PdfHost {
    pub fn new(ctx: &egui::Context, storage: Option<&dyn eframe::Storage>, lang: Lang) -> Self {
        let _ = ctx;
        let mut app = PdfCraftApp::new();
        app.units = pdfcraft_ui_egui::units::Unit::system();
        #[cfg(target_os = "linux")]
        app.enable_desktop_theme();
        if let Some(json) = storage.and_then(|s| s.get_string(STORAGE_KEY).or_else(|| s.get_string(LEGACY_STORAGE_KEY))) {
            app.restore(&json);
        }
        app.integrated_titlebar = false;
        app.window_controls = None;
        app.system_text_scale = text_scale::system();
        #[cfg(target_os = "windows")]
        {
            app.motion.source = Some(motion::source());
        }
        app.printer_source = Some(pdfcraft_ui_egui::system_printers());
        app.os_key_store_ids = cfg!(any(target_os = "macos", target_os = "windows"));
        // Autosave unsaved changes; offer to recover documents a crashed session left behind.
        if let Some(dir) = pdfcraft_ui_egui::RecoveryStore::default_dir() {
            app.enable_recovery(pdfcraft_ui_egui::RecoveryStore::new(dir));
        }
        app.ocr_verify.dictionary_dir = pdfcraft_ui_egui::default_dictionary_dir();
        app.language = lang.code().to_string();
        // With the preference on, last session's files come back.
        app.reopen_last_files(&[]);
        Self { app }
    }
}

impl Host for PdfHost {
    fn take_notices(&mut self) -> Vec<String> {
        self.app.toast.take().map(|t| t.0).into_iter().collect()
    }

    fn kind(&self) -> AppKind {
        AppKind::Pdf
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::logic(&mut self.app, ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.app, ui, frame);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        eframe::App::raw_input_hook(&mut self.app, ctx, raw);
    }

    fn open(&mut self, _ctx: &egui::Context, path: &Path, _intent: Intent) {
        // `open_path` opens PDFs and form data, and turns pictures and text into a new PDF
        // (Create ▸ From file) as PdfCraft's Open does.
        let _ = self.app.open_path(&path_text(path));
    }

    fn new_document(&mut self, _ctx: &egui::Context) {
        self.app.run_command("create.blank");
    }

    fn set_language(&mut self, lang: Lang) {
        // PdfCraft applies `language` at the start of every pass.
        self.app.language = lang.code().to_string();
    }

    fn set_dark(&mut self, dark: bool) {
        use pdfcraft_ui_egui::theme::ThemePreference;
        self.app.set_theme_preference(if dark { ThemePreference::Dark } else { ThemePreference::Light });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::App::save(&mut self.app, storage);
    }

    fn on_exit(&mut self) {
        eframe::App::on_exit(&mut self.app, None);
    }
}
