//! Writing skills: folders holding a `SKILL.md` (the format Claude Code uses), whose instructions
//! are added to writing requests when the skill is active. Read from Themisio's own skills folder
//! (`%APPDATA%\Themisio\skills`), Claude Code's (`~/.claude/skills`) and any folder added in
//! Settings › AI.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
}

/// Themisio's own skills folder (created on demand).
pub fn own_dir() -> Option<PathBuf> {
    crate::settings::dir().map(|d| d.join("skills"))
}

fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| PathBuf::from(h).join(".claude").join("skills"))
}

/// `name:` and `description:` from a SKILL.md's front matter.
fn front_matter(text: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut desc = None;
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    for l in lines {
        let l = l.trim();
        if l == "---" {
            break;
        }
        if let Some(v) = l.strip_prefix("name:") {
            name = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(v) = l.strip_prefix("description:") {
            desc = Some(v.trim().trim_matches('"').to_string());
        }
    }
    (name, desc)
}

fn scan(dir: &Path, out: &mut Vec<Skill>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten().take(500) {
        let p = e.path();
        let md = p.join("SKILL.md");
        if !md.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&md) else { continue };
        let (name, desc) = front_matter(&text);
        let name = name.unwrap_or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        if out.iter().any(|s| s.name == name) {
            continue;
        }
        out.push(Skill { name, description: desc.unwrap_or_default(), path: md });
    }
}

/// Every skill found, by name.
pub fn list(extra: &[String]) -> Vec<Skill> {
    let mut out = Vec::new();
    for d in own_dir().into_iter().chain(claude_dir()).chain(extra.iter().map(PathBuf::from)) {
        scan(&d, &mut out);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// The instructions of the named skills (their SKILL.md without front matter), for a system
/// prompt. Each is capped so a long skill can't crowd out the text being checked.
pub fn instructions(names: &[String], extra: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for s in list(extra).into_iter().filter(|s| names.contains(&s.name)) {
        let Ok(text) = std::fs::read_to_string(&s.path) else { continue };
        let body = if text.trim_start().starts_with("---") {
            text.trim_start().trim_start_matches("---").split_once("\n---").map_or(text.as_str(), |(_, b)| b).trim().to_string()
        } else {
            text.trim().to_string()
        };
        let body: String = body.chars().take(12_000).collect();
        out.push_str(&format!("\n\n# Skill: {}\n{}", s.name, body));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_is_read() {
        let (n, d) = front_matter("---\nname: academic-voice\ndescription: \"Formal\"\n---\nBody");
        assert_eq!(n.as_deref(), Some("academic-voice"));
        assert_eq!(d.as_deref(), Some("Formal"));
    }
}
