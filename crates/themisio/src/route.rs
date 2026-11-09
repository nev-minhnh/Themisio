//! Which app opens which file, and what it does with it.
//!
//! The lists follow what each app's own engine opens (`wordcraft_engine::io::OPEN_EXTS`,
//! `gridcraft_engine::io::FileKind`, DeckCraft's open dialog, PdfCraft's PDF and Create
//! inputs). Where several apps open the same extension (`.txt`, `.md`, `.html`, `.json`), Themisio
//! asks once and can remember the answer.

use std::path::Path;

use crate::app_kind::AppKind;

/// What opening a file should do in its app, beyond showing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Open the document for editing (templates open as they do in the app itself).
    Edit,
    /// A PowerPoint Show (`.ppsx`): start the slide show, as a double-click does in PowerPoint.
    SlideShow,
    /// A picture or text file given to PdfCraft: make a PDF from it (Create › From file).
    ConvertToPdf,
}

/// One way to open a file: the app and what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub app: AppKind,
    pub intent: Intent,
}

const fn edit(app: AppKind) -> Target {
    Target { app, intent: Intent::Edit }
}

/// Word documents, rich text and the text formats WordCraft imports.
pub const WORD_EXTS: &[&str] = &[
    "docx", "docm", "dotx", "dotm", "doc", "dot", "rtf", "odt", "ott", "fodt", "mht", "mhtml", "tex", "latex", "ltx", "xml", "txt", "md",
    "markdown", "html", "htm", "json",
];
/// Presentations, templates and shows; outlines (`.txt`, `.md`) import as slides.
pub const DECK_EXTS: &[&str] = &["deckcraft", "pptx", "potx", "ppsx", "txt", "md"];
/// Workbooks and the data formats GridCraft reads.
pub const GRID_EXTS: &[&str] = &["xlsx", "xlsm", "xltx", "xltm", "xls", "xlt", "xlsb", "ods", "csv", "tsv", "tab", "txt", "slk", "json", "html", "htm"];
/// PDF documents and form data.
pub const PDF_EXTS: &[&str] = &["pdf", "fdf", "xfdf"];
/// Pictures and text PdfCraft turns into a PDF (`pdfcraft_engine::CONVERTIBLE`, kept in step).
pub const PDF_CONVERTIBLE: &[&str] = &["png", "jpg", "jpeg", "tif", "tiff", "bmp", "gif", "webp", "txt"];

/// The extensions Themisio registers with Windows (Open With), grouped by the app that opens them
/// by default. Ambiguous text formats are registered for Open With only (`assoc`).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn registered_extensions() -> Vec<(&'static str, AppKind)> {
    let mut out = Vec::new();
    for e in ["docx", "docm", "dotx", "dotm", "doc", "dot", "rtf", "odt", "ott", "fodt"] {
        out.push((e, AppKind::Word));
    }
    for e in ["pptx", "potx", "ppsx", "deckcraft"] {
        out.push((e, AppKind::Deck));
    }
    for e in ["xlsx", "xlsm", "xltx", "xltm", "xls", "xlsb", "ods", "csv", "tsv"] {
        out.push((e, AppKind::Grid));
    }
    for e in ["pdf", "fdf", "xfdf"] {
        out.push((e, AppKind::Pdf));
    }
    for e in ["txt", "md", "markdown", "html", "htm", "tex", "json", "xml", "slk"] {
        out.push((e, AppKind::Word));
    }
    out
}

/// The lower-case extension of `path`, with `.wcraft.json` recognised.
pub fn extension(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if name.ends_with(".wcraft.json") {
        return "wcraft.json".to_string();
    }
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// Every way Themisio can open `path`, the usual one first. Empty: no app opens it.
pub fn targets(path: &Path) -> Vec<Target> {
    let ext = extension(path);
    let ext = ext.as_str();
    match ext {
        // Unambiguous: one app, one way.
        "docx" | "docm" | "dotx" | "dotm" | "doc" | "dot" | "rtf" | "odt" | "ott" | "fodt" | "mht" | "mhtml" | "tex" | "latex" | "ltx"
        | "xml" | "wcraft.json" => vec![edit(AppKind::Word)],
        "pptx" | "potx" | "deckcraft" => vec![edit(AppKind::Deck)],
        "ppsx" => vec![Target { app: AppKind::Deck, intent: Intent::SlideShow }, edit(AppKind::Deck)],
        "xlsx" | "xlsm" | "xltx" | "xltm" | "xls" | "xlt" | "xlsb" | "ods" | "csv" | "tsv" | "tab" | "slk" => vec![edit(AppKind::Grid)],
        "pdf" | "fdf" | "xfdf" => vec![edit(AppKind::Pdf)],
        "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "gif" | "webp" => {
            vec![Target { app: AppKind::Pdf, intent: Intent::ConvertToPdf }]
        }
        // Shared text formats: the user picks (and may remember the pick).
        "txt" => vec![
            edit(AppKind::Word),
            edit(AppKind::Grid),
            edit(AppKind::Deck),
            Target { app: AppKind::Pdf, intent: Intent::ConvertToPdf },
        ],
        "md" | "markdown" => vec![edit(AppKind::Word), edit(AppKind::Deck)],
        "html" | "htm" => vec![edit(AppKind::Word), edit(AppKind::Grid)],
        "json" => vec![edit(AppKind::Word), edit(AppKind::Grid)],
        _ => Vec::new(),
    }
}

/// Whether the app shown now takes a file dropped on it itself (insert a picture, add pages,
/// import data…). Those drops are left to the app; others are routed by Themisio.
pub fn app_takes_drop(app: AppKind, path: &Path) -> bool {
    let ext = extension(path);
    let ext = ext.as_str();
    let picture = matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "svg" | "emf" | "wmf");
    match app {
        AppKind::Word => picture || WORD_EXTS.contains(&ext) || ext == "wcraft.json",
        AppKind::Deck => {
            picture
                || DECK_EXTS.contains(&ext)
                || matches!(ext, "mp4" | "m4v" | "mov" | "webm" | "mkv" | "wmv" | "wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg" | "opus" | "thmx")
        }
        AppKind::Grid => picture || GRID_EXTS.contains(&ext),
        AppKind::Pdf => PDF_EXTS.contains(&ext) || PDF_CONVERTIBLE.contains(&ext),
    }
}

/// The filter list for Themisio's own Open dialog: everything, then per app.
pub fn open_filters() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut all: Vec<&'static str> = Vec::new();
    for list in [WORD_EXTS, DECK_EXTS, GRID_EXTS, PDF_EXTS, PDF_CONVERTIBLE] {
        for e in list {
            if !all.contains(e) {
                all.push(e);
            }
        }
    }
    vec![
        ("*", all),
        ("word", WORD_EXTS.to_vec()),
        ("deck", DECK_EXTS.to_vec()),
        ("grid", GRID_EXTS.to_vec()),
        ("pdf", PDF_EXTS.iter().chain(PDF_CONVERTIBLE).copied().collect()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_files_go_to_their_app() {
        assert_eq!(targets(Path::new("a/Report.DOCX"))[0].app, AppKind::Word);
        assert_eq!(targets(Path::new("talk.pptx"))[0].app, AppKind::Deck);
        assert_eq!(targets(Path::new("book.xlsx"))[0].app, AppKind::Grid);
        assert_eq!(targets(Path::new("scan.pdf"))[0].app, AppKind::Pdf);
    }

    #[test]
    fn a_show_starts_its_slide_show() {
        assert_eq!(targets(Path::new("talk.ppsx"))[0], Target { app: AppKind::Deck, intent: Intent::SlideShow });
    }

    #[test]
    fn shared_text_formats_offer_a_choice() {
        assert!(targets(Path::new("notes.txt")).len() > 1);
        assert!(targets(Path::new("notes.md")).len() > 1);
        assert!(targets(Path::new("x.unknown")).is_empty());
    }

    #[test]
    fn word_keeps_pictures_dropped_on_it() {
        assert!(app_takes_drop(AppKind::Word, Path::new("p.png")));
        assert!(!app_takes_drop(AppKind::Word, Path::new("s.xlsx")));
    }
}
