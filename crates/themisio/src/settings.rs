//! Themisio's own settings (`%APPDATA%\Themisio\settings.json`). Each app keeps its preferences,
//! recovery files and logs in its own folder, as it does when run on its own.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app_kind::AppKind;
use crate::i18n::LangPref;

/// How mouse-wheel notches glide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Smoothness {
    /// Each notch moves at once.
    Off,
    /// egui's default glide.
    Standard,
    /// A longer, eased glide like Microsoft Edge's.
    #[default]
    Fluid,
}

impl Smoothness {
    /// `(reach, seconds)` for `egui::themisio::set_wheel_smoothing`.
    pub fn wheel(self) -> (f32, f32) {
        match self {
            Self::Off => (0.999, 0.01),
            Self::Standard => (0.90, 0.10),
            Self::Fluid => (0.97, 0.22),
        }
    }
}

/// Whether the shell animates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Motion {
    /// Follow Settings ▸ Accessibility ▸ Visual effects ▸ Animation effects.
    #[default]
    System,
    On,
    Off,
}

/// Swap-chain presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Presentation {
    /// Wait for the display's refresh (no tearing, least power).
    #[default]
    Balanced,
    /// Mailbox / immediate where the driver offers it: lowest input latency.
    LowLatency,
}

/// Light or dark, for Themisio and every app in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePref {
    /// Follow Windows' app mode (Settings ▸ Personalisation ▸ Colours).
    #[default]
    System,
    Light,
    Dark,
}

/// The app bar on the left: always there, or tucked away until the pointer reaches the edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RailMode {
    #[default]
    AutoHide,
    Pinned,
}

/// What draws the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Renderer {
    /// Direct3D 12 (Vulkan/Metal elsewhere), with a small GPU memory pool.
    #[default]
    Gpu,
    /// OpenGL: the least memory, for older GPUs too.
    OpenGl,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent {
    pub path: PathBuf,
    pub app: AppKind,
    /// Seconds since the Unix epoch.
    pub opened: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: LangPref,
    pub smoothness: Smoothness,
    pub motion: Motion,
    pub presentation: Presentation,
    pub theme: ThemePref,
    pub rail: RailMode,
    pub renderer: Renderer,
    pub recent: Vec<Recent>,
    /// Remembered choices for extensions several apps open (`txt` → `word`).
    pub open_with: BTreeMap<String, AppKind>,
    /// The app shown when Themisio last closed.
    pub last_app: Option<AppKind>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LangPref::System,
            smoothness: Smoothness::Fluid,
            motion: Motion::System,
            presentation: Presentation::Balanced,
            theme: ThemePref::System,
            rail: RailMode::AutoHide,
            renderer: Renderer::Gpu,
            recent: Vec::new(),
            open_with: BTreeMap::new(),
            last_app: None,
        }
    }
}

/// Most recent files kept.
const MAX_RECENT: usize = 40;

impl Settings {
    pub fn load() -> Self {
        let Some(path) = file() else { return Self::default() };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                log::warn!("settings: {} is unreadable ({e}); using defaults", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = file() else { return };
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            log::warn!("settings: cannot create {}: {e}", dir.display());
            return;
        }
        match serde_json::to_vec_pretty(self) {
            Ok(bytes) => {
                // Write beside, then rename: a crash mid-write never leaves half a file.
                let tmp = path.with_extension("json.tmp");
                if let Err(e) = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, &path)) {
                    log::warn!("settings: cannot save {}: {e}", path.display());
                    let _ = std::fs::remove_file(&tmp);
                }
            }
            Err(e) => log::warn!("settings: {e}"),
        }
    }

    pub fn add_recent(&mut self, path: &Path, app: AppKind) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|r| r.path != path);
        self.recent.insert(0, Recent { path, app, opened: chrono::Utc::now().timestamp() });
        self.recent.truncate(MAX_RECENT);
    }
}

/// `%APPDATA%\Themisio` on Windows, `$XDG_CONFIG_HOME/themisio` elsewhere.
pub fn dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Themisio"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("themisio"))
    }
}

fn file() -> Option<PathBuf> {
    if std::env::var_os("THEMISIO_NO_PREFS").is_some() {
        return None;
    }
    dir().map(|d| d.join("settings.json"))
}
