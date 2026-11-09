//! API keys, kept in Windows Credential Manager (never in Themisio's settings file). Each key is
//! the entry `ai-key-<provider>`. Keys saved by Callio (or Proemia, or Quire, its earlier names)
//! are used too and copied over, so a provider you signed in to there works here without typing
//! the key again.

/// Themisio's entries in Credential Manager.
const SERVICE: &str = "Themisio";
/// Where Minh's editor keeps the same keys, newest name first.
const SHARED: &[&str] = &["Callio", "Proemia", "Quire"];

fn entry_name(provider: &str) -> String {
    format!("ai-key-{provider}")
}

#[cfg(windows)]
fn read(service: &str, name: &str) -> Option<String> {
    keyring::Entry::new(service, name).ok()?.get_password().ok().filter(|s| !s.trim().is_empty())
}

#[cfg(windows)]
fn write(service: &str, name: &str, value: &str) -> Result<(), String> {
    let e = keyring::Entry::new(service, name).map_err(|e| e.to_string())?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}

// Elsewhere (development builds): THEMISIO_KEY_<PROVIDER> in the environment.
#[cfg(not(windows))]
fn read(service: &str, name: &str) -> Option<String> {
    if service != SERVICE {
        return None;
    }
    let var = format!("THEMISIO_KEY_{}", name.trim_start_matches("ai-key-").to_uppercase().replace('-', "_"));
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

#[cfg(not(windows))]
fn write(_service: &str, _name: &str, _value: &str) -> Result<(), String> {
    Err("Keys are kept in Windows Credential Manager".into())
}

/// The key for `provider`, if one is saved (Themisio's own, else one Callio saved).
pub fn get(provider: &str) -> Option<String> {
    let name = entry_name(provider);
    if let Some(k) = read(SERVICE, &name) {
        return Some(k);
    }
    for s in SHARED {
        if let Some(k) = read(s, &name) {
            let _ = write(SERVICE, &name, &k);
            return Some(k);
        }
    }
    None
}

/// Save (or, with an empty value, remove) the key for `provider`.
pub fn set(provider: &str, value: &str) -> Result<(), String> {
    write(SERVICE, &entry_name(provider), value.trim())
}

/// Whether a key is saved for `provider`.
pub fn has(provider: &str) -> bool {
    get(provider).is_some()
}
