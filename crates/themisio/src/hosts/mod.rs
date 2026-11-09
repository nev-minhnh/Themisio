//! The four apps, each wrapped the way its own desktop `main.rs` wraps it: the same services
//! (file dialogs, clipboard, printing, audio), preferences, recovery and start-up steps, so every
//! feature works as in the standalone app. The platform helpers from each app's `apps/<app>/src`
//! are compiled in unchanged with `#[path]`, never copied.

use std::path::Path;

use crate::app_kind::AppKind;
use crate::i18n::Lang;
use crate::route::Intent;

pub mod deck;
pub mod drawio;
pub mod grid;
pub mod pdf;
pub mod sheet_context;
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

/// The text the writing assistant works on, and where it is (each app reads its own `anchor`).
#[derive(Clone, Debug, PartialEq)]
pub struct AiTarget {
    pub text: String,
    /// The user's selection (else the paragraph, text box or cell at the caret).
    pub selection: bool,
    pub anchor: serde_json::Value,
    /// Where on screen it is, when the app knows (the caret).
    pub screen: Option<egui::Pos2>,
}

/// A document command from Themisio's tab menu, run with the app's own command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocCommand {
    Save,
    SaveAs,
    Print,
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
    /// A new blank document (File › New › Blank).
    fn new_document(&mut self, ctx: &egui::Context);
    /// The app's own start screen, where it has one (WordCraft's templates and recent files).
    fn show_start_screen(&mut self) {}
    /// Something of Themisio's (a dialog, a menu, the app bar, another page) is over the app:
    /// apps drawn outside egui (draw.io's webview) hide meanwhile.
    fn set_covered(&mut self, _covered: bool) {}
    /// Nothing the user opened or wrote is on screen (no documents, or only the blank one the
    /// app starts with): the app shows its start screen when chosen from the rail.
    fn is_blank(&self) -> bool {
        self.tabs().is_some_and(|t| t.is_empty())
    }
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
    /// Close document `id`, asking about unsaved changes as the app's File › Close does.
    fn close_tab(&mut self, _id: u64) {}
    /// Messages the app has put up since the last call (its toasts and status messages), for
    /// Themisio's notifications. The app's own toast is taken, so the message shows once.
    fn take_notices(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// The text for the writing assistant: the selection, or the paragraph at the caret
    /// (Documents, Presentation), or the active cell's text (CalcSheet).
    fn ai_target(&mut self) -> Option<AiTarget> {
        None
    }
    /// Replace the first `original` at or after the target's start with `replacement`, as an
    /// edit the user can undo.
    fn ai_replace(&mut self, _target: &AiTarget, _original: &str, _replacement: &str) -> Result<(), String> {
        Err("not available here".into())
    }
    /// Put `text` in place of the whole target (a rewrite or a translation).
    fn ai_replace_all(&mut self, target: &AiTarget, text: &str) -> Result<(), String> {
        self.ai_replace(target, &target.text, text)
    }
    /// A number that changes whenever the text the writing assistant reads may have changed.
    fn ai_revision(&mut self) -> u64 {
        0
    }
    /// Every piece of text the writing assistant checks (paragraphs, text boxes, cells), each
    /// with a key the app understands.
    fn ai_units(&mut self) -> Vec<(serde_json::Value, String)> {
        Vec::new()
    }
    /// Where `original` in unit `key` is on screen, line by line (for underlines). Empty when
    /// the app can't say (the panel still lists the suggestion).
    fn ai_marks(&mut self, _key: &serde_json::Value, _original: &str) -> Vec<egui::Rect> {
        Vec::new()
    }
    /// The area the document is drawn in (underlines are clipped to it).
    fn ai_viewport(&mut self) -> Option<egui::Rect> {
        None
    }
    /// Replace `original` in unit `key` with `replacement` (an edit that can be undone).
    fn ai_apply(&mut self, _key: &serde_json::Value, _original: &str, _replacement: &str) -> Result<(), String> {
        Err("not available here".into())
    }
    /// Show `original` in unit `key` (select it).
    fn ai_focus(&mut self, _key: &serde_json::Value, _original: &str) {}
    /// Insert `text` at the cursor (after the selection).
    fn ai_insert(&mut self, _text: &str) -> Result<(), String> {
        Err("Put the cursor where the text should go first.".into())
    }
    /// CalcSheet: the workbook as the assistant sees it (sheets, selection, the data).
    fn sheet_context(&mut self) -> Option<String> {
        None
    }
    /// CalcSheet: the parameters of these commands, one line each.
    fn command_docs(&mut self, _ids: &[&str]) -> String {
        String::new()
    }
    /// CalcSheet: error values (cell, error, formula) in these ranges, after actions ran.
    fn sheet_errors(&mut self, _ranges: &[String]) -> Vec<(String, String, String)> {
        Vec::new()
    }
    /// Run one of the app's commands with parameters (an action the user allowed).
    fn run_json(&mut self, _id: &str, _params: serde_json::Value) -> Result<serde_json::Value, String> {
        Err("not available here".into())
    }
    /// Save, Save As or Print the active document, as the app's File menu does.
    fn command(&mut self, _cmd: DocCommand) -> Result<(), String> {
        Err("not available here".into())
    }
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
    pub drawio: Option<drawio::DrawioHost>,
}

impl Hosts {
    pub fn get(&mut self, kind: AppKind) -> Option<&mut dyn Host> {
        match kind {
            AppKind::Word => self.word.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Deck => self.deck.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Grid => self.grid.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Pdf => self.pdf.as_mut().map(|h| h as &mut dyn Host),
            AppKind::Drawio => self.drawio.as_mut().map(|h| h as &mut dyn Host),
        }
    }

    pub fn is_loaded(&self, kind: AppKind) -> bool {
        match kind {
            AppKind::Word => self.word.is_some(),
            AppKind::Deck => self.deck.is_some(),
            AppKind::Grid => self.grid.is_some(),
            AppKind::Pdf => self.pdf.is_some(),
            AppKind::Drawio => self.drawio.is_some(),
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
            AppKind::Drawio => self.drawio = Some(drawio::DrawioHost::new(ctx, lang)),
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
            AppKind::Drawio => self.drawio = None,
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

/// The person using this computer, for apps that sign comments and changes: the user name Office
/// keeps (`HKCU\Software\Microsoft\Office\Common\UserInfo`), else the Windows account name.
pub fn user_name() -> Option<String> {
    #[cfg(windows)]
    {
        let office = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Office\\Common\\UserInfo")
            .and_then(|k| k.get_value::<String, _>("UserName"))
            .ok()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        if office.is_some() {
            return office;
        }
    }
    let account = std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).ok()?;
    let account = account.trim();
    if account.is_empty() {
        return None;
    }
    // "minh" reads "Minh" (and its initial "M").
    let mut chars = account.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}
