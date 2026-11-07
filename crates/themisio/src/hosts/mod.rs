//! The four apps, each wrapped the way its own desktop `main.rs` wraps it: the same services
//! (file dialogs, clipboard, printing, audio), preferences, recovery and start-up steps, so every
//! feature works as in the standalone app. The platform helpers from each app's `apps/<app>/src`
//! are compiled in unchanged with `#[path]`, never copied.

use std::path::Path;

use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;

pub mod deck;
pub mod grid;
pub mod pdf;
pub mod word;

// Each app's desktop helpers (its `apps/<app>/src` modules), compiled in unchanged. An inline
// module with a `#[path]` keeps their own sub-modules (`print/isolated.rs`…) where they are.
#[path = "../../../../upstream/wordcraft/apps/wordcraft/src"]
pub(crate) mod word_src {
    pub mod animations;
    pub mod clipboard;
    pub mod file_dialogs;
    pub mod print;
    pub mod region;
}
#[path = "../../../../upstream/deckcraft/apps/deckcraft/src"]
pub(crate) mod deck_src {
    pub mod audio;
    pub mod pasteboard;
}
#[path = "../../../../upstream/gridcraft/apps/gridcraft/src"]
pub(crate) mod grid_src {
    pub mod print;
}
#[path = "../../../../upstream/pdfcraft/apps/pdfcraft/src"]
pub(crate) mod pdf_src {
    #[cfg(target_os = "windows")]
    pub mod motion;
    pub mod text_scale;
}

/// One open document, as Themisio's tab bar shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub dirty: bool,
    pub active: bool,
}

/// What the shell needs from a hosted app.
pub trait Host {
    #[allow(dead_code)]
    fn kind(&self) -> AppKind;
    /// Once per pass while the app is shown, before [`Self::ui`].
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame);
    /// Draw the app into `ui` (the window minus Themisio's rail).
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame);
    /// Raw input for this pass, before egui sees it.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput);
    /// Open a file the way the app's own command line / Explorer double-click would.
    fn open(&mut self, ctx: &egui::Context, path: &Path, intent: Intent);
    /// A new blank document (File ▸ New ▸ Blank).
    fn new_document(&mut self, ctx: &egui::Context);
    /// The app's own start screen, where it has one (WordCraft's templates and recent files).
    fn show_start_screen(&mut self) {}
    /// Show the interface in `lang`. Applied from the next pass the app is shown.
    fn set_language(&mut self, lang: Lang);
    /// Light or dark, as Themisio's theme says (every app follows it).
    fn set_dark(&mut self, dark: bool);
    /// The open documents for Themisio's tab bar; `None` when the app shows its own tabs (PDF).
    fn tabs(&self) -> Option<Vec<Tab>> {
        None
    }
    /// Show document `id` (a tab was clicked).
    fn activate_tab(&mut self, _id: u64) {}
    /// Close document `id`, asking about unsaved changes as the app's File ▸ Close does.
    fn close_tab(&mut self, _id: u64) {}
    /// eframe's periodic save (only PdfCraft keeps its settings there).
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
    /// The app is going away (window closing, or the app was closed): save preferences, finish
    /// print jobs and recovery, as the standalone app does on exit.
    fn on_exit(&mut self);
}

/// The hosted apps, created when first shown.
#[derive(Default)]
pub struct Hosts {
    pub word: Option<word::WordHost>,
    pub deck: Option<deck::DeckHost>,
    pub grid: Option<grid::GridHost>,
    pub pdf: Option<pdf::PdfHost>,
}

impl Hosts {
    pub fn get(&mut self, kind: AppKind) -> Option<&mut dyn Host> {
        match kind {
            AppKind::Word => self.word.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Deck => self.deck.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Grid => self.grid.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Pdf => self.pdf.as_mut().map(|h| h as &mut dyn Host),
        }
    }

    pub fn is_loaded(&self, kind: AppKind) -> bool {
        match kind {
            AppKind::Word => self.word.is_some(),
            AppKind::Deck => self.deck.is_some(),
            AppKind::Grid => self.grid.is_some(),
            AppKind::Pdf => self.pdf.is_some(),
        }
    }

    /// Create the app if it isn't running yet.
    pub fn ensure(&mut self, kind: AppKind, ctx: &egui::Context, frame: &eframe::Frame, lang: Lang, dark: bool) {
        if self.is_loaded(kind) {
            return;
        }
        let started = std::time::Instant::now();
        match kind {
            AppKind::Word => self.word = Some(word::WordHost::new(ctx, lang)),
            AppKind::Deck => self.deck = Some(deck::DeckHost::new(ctx, lang)),
            AppKind::Grid => self.grid = Some(grid::GridHost::new(ctx, lang)),
            AppKind::Pdf => self.pdf = Some(pdf::PdfHost::new(ctx, frame.storage(), lang)),
        }
        if let Some(h) = self.get(kind) {
            h.set_dark(dark);
        }
        log::info!("{} started in {} ms", kind.name(), started.elapsed().as_millis());
    }

    /// Stop the app (after its own on-exit work).
    pub fn unload(&mut self, kind: AppKind) {
        if let Some(h) = self.get(kind) {
            h.on_exit();
        }
        match kind {
            AppKind::Word => self.word = None,
            AppKind::Deck => self.deck = None,
            AppKind::Grid => self.grid = None,
            AppKind::Pdf => self.pdf = None,
        }
    }

    pub fn loaded(&self) -> Vec<AppKind> {
        AppKind::ALL.into_iter().filter(|k| self.is_loaded(*k)).collect()
    }
}

/// Lossy UTF-8 path text for the apps' string-based open calls.
pub fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
