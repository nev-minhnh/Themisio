//! Windows file associations, per user (`HKCU\Software\Classes`, no administrator rights): one
//! ProgID per kind of file, Open With entries for every extension the apps open, and the
//! Capabilities that list Themisio in Settings › Apps › Default apps. Windows doesn't let an app
//! make itself the default; the user confirms there.

use crate::app_kind::AppKind;

/// ProgID and description for each app's files.
#[cfg_attr(not(windows), allow(dead_code))]
fn prog_id(app: AppKind, ext: &str) -> (&'static str, &'static str) {
    match (app, ext) {
        (AppKind::Deck, "ppsx") => ("Themisio.Show", "Themisio Slide Show"),
        (_, "txt" | "md" | "markdown" | "html" | "htm" | "json" | "xml" | "tex") => ("Themisio.Text", "Text file (Themisio)"),
        (AppKind::Word, _) => ("Themisio.Document", "Themisio Document"),
        (AppKind::Deck, _) => ("Themisio.Presentation", "Themisio Presentation"),
        (AppKind::Grid, _) => ("Themisio.Workbook", "Themisio Workbook"),
        (AppKind::Pdf, _) => ("Themisio.PDF", "Themisio PDF Document"),
        (AppKind::Drawio, _) => ("Themisio.Diagram", "Themisio Diagram (draw.io)"),
    }
}

#[cfg(windows)]
pub fn register() -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy().into_owned();
    let open = format!("\"{exe}\" \"%1\"");
    let icon = format!("\"{exe}\",0");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let err = |e: std::io::Error| e.to_string();
    let classes = hkcu.create_subkey(r"Software\Classes").map_err(err)?.0;
    let mut progs: Vec<(&str, &str)> = Vec::new();
    let (caps, _) = hkcu.create_subkey(r"Software\Themisio\Capabilities").map_err(err)?;
    caps.set_value("ApplicationName", &"Themisio").map_err(err)?;
    caps.set_value("ApplicationDescription", &"Themisio – Make your documents glow. Documents, presentations, workbooks and PDFs.").map_err(err)?;
    caps.set_value("ApplicationIcon", &icon).map_err(err)?;
    let (file_assoc, _) = caps.create_subkey("FileAssociations").map_err(err)?;
    let (app_key, _) = classes.create_subkey(r"Applications\themisio.exe").map_err(err)?;
    app_key.set_value("FriendlyAppName", &"Themisio").map_err(err)?;
    let (app_cmd, _) = app_key.create_subkey(r"shell\open\command").map_err(err)?;
    app_cmd.set_value("", &open).map_err(err)?;
    let (supported, _) = app_key.create_subkey("SupportedTypes").map_err(err)?;
    for (ext, app) in crate::route::registered_extensions() {
        let (id, desc) = prog_id(app, ext);
        if !progs.iter().any(|(p, _)| *p == id) {
            progs.push((id, desc));
        }
        let dot = format!(".{ext}");
        let (ext_key, _) = classes.create_subkey(format!(r"{dot}\OpenWithProgids")).map_err(err)?;
        ext_key.set_value(id, &"").map_err(err)?;
        file_assoc.set_value(&dot, &id).map_err(err)?;
        supported.set_value(&dot, &"").map_err(err)?;
    }
    for (id, desc) in progs {
        let (prog, _) = classes.create_subkey(id).map_err(err)?;
        prog.set_value("", &desc).map_err(err)?;
        let (default_icon, _) = prog.create_subkey("DefaultIcon").map_err(err)?;
        default_icon.set_value("", &icon).map_err(err)?;
        let (cmd, _) = prog.create_subkey(r"shell\open\command").map_err(err)?;
        cmd.set_value("", &open).map_err(err)?;
    }
    let (registered, _) = hkcu.create_subkey(r"Software\RegisteredApplications").map_err(err)?;
    registered.set_value("Themisio", &r"Software\Themisio\Capabilities").map_err(err)?;
    log::info!("file types registered for {exe}");
    Ok(())
}

#[cfg(windows)]
pub fn unregister() -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.open_subkey_with_flags(r"Software\Classes", winreg::enums::KEY_ALL_ACCESS).map_err(|e| e.to_string())?;
    for (ext, app) in crate::route::registered_extensions() {
        let (id, _) = prog_id(app, ext);
        if let Ok(k) = classes.open_subkey_with_flags(format!(r".{ext}\OpenWithProgids"), winreg::enums::KEY_ALL_ACCESS) {
            let _ = k.delete_value(id);
        }
    }
    for id in ["Themisio.Document", "Themisio.Presentation", "Themisio.Show", "Themisio.Workbook", "Themisio.PDF", "Themisio.Text", r"Applications\themisio.exe"] {
        let _ = classes.delete_subkey_all(id);
    }
    let _ = hkcu.delete_subkey_all(r"Software\Themisio\Capabilities");
    if let Ok(k) = hkcu.open_subkey_with_flags(r"Software\RegisteredApplications", winreg::enums::KEY_ALL_ACCESS) {
        let _ = k.delete_value("Themisio");
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn register() -> Result<(), String> {
    Err("file types are registered on Windows only".into())
}

#[cfg(not(windows))]
pub fn unregister() -> Result<(), String> {
    Ok(())
}

/// Settings › Apps › Default apps, on Themisio's page.
pub fn open_default_apps() {
    #[cfg(windows)]
    if let Err(e) = open::that_detached("ms-settings:defaultapps?registeredAppUser=Themisio") {
        log::warn!("couldn't open Settings: {e}");
    }
}

/// WordCraft's File › Options "make the default" button, pointed at Themisio.
pub fn default_app_hook() -> Option<Box<dyn Fn() -> Result<wordcraft_ui_egui::DefaultApp, String>>> {
    if cfg!(windows) {
        Some(Box::new(|| {
            register()?;
            open_default_apps();
            Ok(wordcraft_ui_egui::DefaultApp::SettingsOpened)
        }))
    } else {
        None
    }
}
