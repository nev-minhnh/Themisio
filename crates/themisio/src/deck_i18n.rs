//! The apps' interface strings as Themisio shows them: their own names become Themisio's
//! (Documents, Presentation, CalcSheet, PDF), and DeckCraft's strings become Vietnamese.
//!
//! DeckCraft in Vietnamese.
//!
//! DeckCraft's interface strings are English literals (it has no catalog yet), so Themisio
//! translates them as they are drawn: `egui::themisio` offers each interface string to
//! [`translate`] while DeckCraft draws and Vietnamese is chosen. The dictionary is
//! `deck_vi.tsv` (English, tab, Vietnamese; `{name}` placeholders; `\n` for line breaks).
//! Strings not in it stay English, so a newer DeckCraft only ever shows some English.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const TSV: &str = include_str!("deck_vi.tsv");

/// A dictionary entry with placeholders: the literal parts around them, and the Vietnamese.
struct Pattern {
    /// Literal text before, between and after the placeholders (one more than `names`).
    parts: Vec<String>,
    names: Vec<String>,
    vi: String,
}

struct Dict {
    exact: HashMap<String, String>,
    patterns: Vec<Pattern>,
    /// Results for strings seen before (most interface strings are drawn every frame).
    cache: Mutex<HashMap<String, Option<String>>>,
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t")
}

/// Split `{name}` placeholders out of a template. `None` when it has none.
fn split_template(s: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut parts = Vec::new();
    let mut names = Vec::new();
    let mut rest = s;
    let mut current = String::new();
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|c| open + c) else { break };
        let name = &rest[open + 1..close];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            current.push_str(&rest[..=close]);
            rest = &rest[close + 1..];
            continue;
        }
        current.push_str(&rest[..open]);
        parts.push(std::mem::take(&mut current));
        names.push(name.to_string());
        rest = &rest[close + 1..];
    }
    current.push_str(rest);
    parts.push(current);
    (!names.is_empty()).then_some((parts, names))
}

fn dict() -> &'static Dict {
    static DICT: OnceLock<Dict> = OnceLock::new();
    DICT.get_or_init(|| {
        let mut exact = HashMap::new();
        let mut patterns = Vec::new();
        for line in TSV.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let Some((en, vi)) = line.split_once('\t') else { continue };
            let (en, vi) = (unescape(en), unescape(vi.trim_end_matches('\r')));
            if vi.is_empty() {
                continue;
            }
            match split_template(&en) {
                // Two placeholders side by side can't be told apart; such entries are skipped.
                Some((parts, names)) if parts.iter().skip(1).take(names.len().saturating_sub(1)).all(|p| !p.is_empty()) => {
                    patterns.push(Pattern { parts, names, vi });
                }
                Some(_) => {}
                None => {
                    exact.insert(en, vi);
                }
            }
        }
        // Longer literal text first: the most specific pattern wins.
        patterns.sort_by_key(|p| std::cmp::Reverse(p.parts.iter().map(String::len).sum::<usize>()));
        Dict { exact, patterns, cache: Mutex::new(HashMap::new()) }
    })
}

fn match_pattern(p: &Pattern, s: &str) -> Option<String> {
    let first = p.parts.first()?;
    let last = p.parts.last()?;
    let mut rest = s.strip_prefix(first.as_str())?;
    if rest.len() < last.len() {
        return None;
    }
    rest = rest.strip_suffix(last.as_str())?;
    let mut values = Vec::with_capacity(p.names.len());
    let inner = &p.parts[1..p.parts.len() - 1];
    for sep in inner {
        let at = rest.find(sep.as_str())?;
        values.push(&rest[..at]);
        rest = &rest[at + sep.len()..];
    }
    values.push(rest);
    if values.len() != p.names.len() || values.iter().any(|v| v.is_empty()) {
        return None;
    }
    let mut out = p.vi.clone();
    for (name, value) in p.names.iter().zip(values) {
        // A value that is itself an interface word ("Bold") is translated too.
        let value = lookup(value).unwrap_or_else(|| value.to_string());
        out = out.replace(&format!("{{{name}}}"), &value);
    }
    Some(out)
}

/// The Vietnamese for one string, without the cache.
fn lookup(s: &str) -> Option<String> {
    let d = dict();
    if let Some(v) = d.exact.get(s) {
        return Some(v.clone());
    }
    // Surrounding spaces, as some labels are padded.
    let trimmed = s.trim();
    if trimmed.len() != s.len() && !trimmed.is_empty() {
        let lead = &s[..s.len() - s.trim_start().len()];
        let trail = &s[s.trim_end().len()..];
        return lookup(trimmed).map(|v| format!("{lead}{v}{trail}"));
    }
    // "Insert Picture…" / "Insert Picture..." / "Size:" share their stem's translation.
    for suffix in ["…", "...", ":", " ▾", " ›", " ▸"] {
        if let Some(stem) = s.strip_suffix(suffix)
            && !stem.is_empty()
            && let Some(v) = d.exact.get(stem)
        {
            return Some(format!("{v}{suffix}"));
        }
    }
    // A shortcut in parentheses: "Copy (Ctrl+C)".
    if let Some(open) = s.rfind(" (")
        && s.ends_with(')')
    {
        let inside = &s[open + 2..s.len() - 1];
        if inside.contains('+') || inside.starts_with('F') || inside.starts_with("Ctrl") || inside.starts_with('⌘') {
            if let Some(v) = lookup(&s[..open]) {
                return Some(format!("{v}{}", &s[open..]));
            }
        }
    }
    // Paths of menus: "Insert › Table".
    for sep in [" › ", " ▸ ", " > "] {
        if s.contains(sep) {
            let parts: Vec<String> = s.split(sep).map(|p| lookup(p).unwrap_or_else(|| p.to_string())).collect();
            let joined = parts.join(sep);
            return (joined != s).then_some(joined);
        }
    }
    d.patterns.iter().find_map(|p| match_pattern(p, s))
}

static DECK_VIETNAMESE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the strings drawn now are the Presentation app's, shown in Vietnamese.
pub fn set_deck_vietnamese(on: bool) {
    DECK_VIETNAMESE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The apps' own names, as Themisio calls them.
const NAMES: [(&str, &str); 5] =
    [("WordCraft", "Documents"), ("DeckCraft", "Presentation"), ("GridCraft", "CalcSheet"), ("PdfCraft", "PDF"), ("PrintCraft", "PDF")];

/// `text` with the apps' names replaced; `None` when it names none of them.
pub fn rebrand(text: &str) -> Option<String> {
    if !text.contains("Craft") {
        return None;
    }
    let mut out = text.to_string();
    for (from, to) in NAMES {
        if out.contains(from) {
            out = out.replace(from, to);
        }
    }
    (out != text).then_some(out)
}

/// The translator `egui::themisio` calls while an app draws.
pub fn translate(s: &str) -> Option<String> {
    let vi = if DECK_VIETNAMESE.load(std::sync::atomic::Ordering::Relaxed) { deck_vietnamese(s) } else { None };
    match vi {
        Some(v) => Some(rebrand(&v).unwrap_or(v)),
        None => rebrand(s),
    }
}

/// DeckCraft's string in Vietnamese, when the dictionary has it.
fn deck_vietnamese(s: &str) -> Option<String> {
    // Long text (a paragraph of help, a pasted value) is never an interface label.
    if s.len() > 400 || !s.chars().any(char::is_alphabetic) {
        return None;
    }
    let d = dict();
    if let Ok(cache) = d.cache.lock()
        && let Some(hit) = cache.get(s)
    {
        return hit.clone();
    }
    let result = lookup(s);
    if let Ok(mut cache) = d.cache.lock() {
        if cache.len() > 50_000 {
            cache.clear();
        }
        cache.insert(s.to_string(), result.clone());
    }
    result
}

/// How many entries the dictionary has (for the log).
pub fn size() -> usize {
    let d = dict();
    d.exact.len() + d.patterns.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_split_into_parts_and_names() {
        let (parts, names) = split_template("Couldn't save {name}: {e}").unwrap_or_default();
        assert_eq!(parts, ["Couldn't save ", ": ", ""]);
        assert_eq!(names, ["name", "e"]);
        assert!(split_template("No placeholders").is_none());
    }

    #[test]
    fn the_apps_names_become_themisios() {
        assert_eq!(rebrand("About DeckCraft").as_deref(), Some("About Presentation"));
        assert_eq!(rebrand("Welcome to PdfCraft").as_deref(), Some("Welcome to PDF"));
        assert_eq!(rebrand("Plain text"), None);
    }

    #[test]
    fn exact_suffix_and_pattern_lookups() {
        set_deck_vietnamese(true);
        assert!(dict().exact.contains_key("Open"));
        assert!(translate("Open").is_some());
        assert!(translate("Open…").is_some_and(|v| v.ends_with('…')));
        assert_eq!(translate("zzz not a label"), None);
        assert_eq!(translate("12.5"), None);
    }
}
