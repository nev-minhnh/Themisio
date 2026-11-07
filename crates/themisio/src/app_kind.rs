//! The four hosted apps and the pages of the shell.

use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppKind {
    /// WordCraft: documents.
    Word,
    /// DeckCraft: presentations.
    Deck,
    /// GridCraft: spreadsheets.
    Grid,
    /// PdfCraft: PDF reading and editing.
    Pdf,
}

impl AppKind {
    pub const ALL: [Self; 4] = [Self::Word, Self::Deck, Self::Grid, Self::Pdf];

    /// Stable id for settings and the command line.
    pub fn id(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Deck => "deck",
            Self::Grid => "grid",
            Self::Pdf => "pdf",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id.trim().to_ascii_lowercase())
    }

    /// The app's name in Themisio.
    pub fn name(self) -> &'static str {
        match self {
            Self::Word => "Documents",
            Self::Deck => "Presentation",
            Self::Grid => "CalcSheet",
            Self::Pdf => "PDF",
        }
    }

    /// Accent colour of the app's rail icon and Home tile (the strips of Themisio's logo).
    pub fn accent(self) -> Color32 {
        match self {
            Self::Word => Color32::from_rgb(0x1A, 0x8C, 0xF8),
            Self::Deck => Color32::from_rgb(0xF5, 0x8E, 0x1C),
            Self::Grid => Color32::from_rgb(0x2B, 0xB8, 0x6A),
            Self::Pdf => Color32::from_rgb(0xF0, 0x3C, 0x45),
        }
    }
}

/// What fills the window beside the rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    App(AppKind),
}

impl Page {
    pub fn app(self) -> Option<AppKind> {
        match self {
            Self::Home => None,
            Self::App(k) => Some(k),
        }
    }
}
