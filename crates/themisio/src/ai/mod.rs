//! Themisio's AI: the providers you sign in to once (API keys in Windows Credential Manager, or
//! Claude Code with your Claude plan), and the requests the writing assistant, translation and the
//! CalcSheet assistant make. Requests go straight from this PC to the provider; nothing passes
//! through any other server.
//!
//! Two wire protocols cover the API providers: Anthropic's Messages API (Claude) and the OpenAI
//! Chat Completions API (ChatGPT, Gemini through Google AI Studio, Grok, Perplexity, llama.cpp,
//! DeepSeek, MiniMax and any other OpenAI-compatible service). Claude Code runs the official
//! `claude` program with the user's own sign-in. Every request runs on its own thread.

pub mod assist;
pub mod keys;
pub mod settings_ui;
pub mod sheet_chat;
pub mod skills;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A provider Themisio knows how to reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Anthropic,
    OpenAi,
    ClaudeCode,
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub wire: Wire,
    pub base_url: &'static str,
    pub key_url: &'static str,
    /// A key isn't needed (a local server).
    pub optional_key: bool,
}

/// The built-in providers. Custom ones (DeepSeek, MiniMax, OpenRouter…) are added in Settings.
pub const PRESETS: &[Preset] = &[
    Preset { id: "anthropic", name: "Claude (Anthropic)", wire: Wire::Anthropic, base_url: "https://api.anthropic.com", key_url: "https://console.anthropic.com/settings/keys", optional_key: false },
    Preset { id: "openai", name: "ChatGPT (OpenAI)", wire: Wire::OpenAi, base_url: "https://api.openai.com/v1", key_url: "https://platform.openai.com/api-keys", optional_key: false },
    Preset {
        id: "gemini",
        name: "Gemini (Google AI Studio)",
        wire: Wire::OpenAi,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        key_url: "https://aistudio.google.com/apikey",
        optional_key: false,
    },
    Preset { id: "xai", name: "Grok (xAI)", wire: Wire::OpenAi, base_url: "https://api.x.ai/v1", key_url: "https://console.x.ai", optional_key: false },
    Preset { id: "perplexity", name: "Perplexity", wire: Wire::OpenAi, base_url: "https://api.perplexity.ai", key_url: "https://www.perplexity.ai/account/api/keys", optional_key: false },
    Preset { id: "llamacpp", name: "llama.cpp (your server)", wire: Wire::OpenAi, base_url: "http://localhost:8080/v1", key_url: "", optional_key: true },
];

/// Ready-made addresses for "Add a custom provider".
pub const CUSTOM_TEMPLATES: &[(&str, &str)] = &[
    ("DeepSeek", "https://api.deepseek.com/v1"),
    ("MiniMax", "https://api.minimax.io/v1"),
    ("OpenRouter", "https://openrouter.ai/api/v1"),
    ("Mistral", "https://api.mistral.ai/v1"),
    ("Groq", "https://api.groq.com/openai/v1"),
    ("Together", "https://api.together.xyz/v1"),
];

/// Claude models Claude Code can run (id, label), newest first; "default" lets Claude Code pick.
pub const CC_MODELS: &[(&str, &str)] = &[
    ("default", "Default"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-fable-5", "Fable 5"),
    ("claude-opus-5", "Opus 5"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-opus-4-8", "Opus 4.8"),
    ("claude-opus-4-7", "Opus 4.7"),
    ("claude-opus-4-6", "Opus 4.6"),
    ("claude-sonnet-4-6", "Sonnet 4.6"),
    ("claude-opus-4-5", "Opus 4.5"),
    ("claude-sonnet-4-5", "Sonnet 4.5"),
    ("claude-haiku-4-5", "Haiku 4.5"),
];

pub const EFFORTS: &[(&str, &str)] = &[("", "Model default"), ("low", "Low"), ("medium", "Medium"), ("high", "High"), ("xhigh", "Extra"), ("max", "Max")];

/// One provider's settings (its key lives in Credential Manager under `ai-key-<id>`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCfg {
    pub enabled: bool,
    /// Empty: the preset's address.
    pub base_url: String,
    /// The models offered in the pickers.
    pub models: Vec<String>,
    /// Custom providers: the name shown.
    pub name: String,
}

/// What the writing assistant may look at, per app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppsOn {
    pub documents: bool,
    pub presentation: bool,
    pub calcsheet: bool,
}

impl Default for AppsOn {
    fn default() -> Self {
        Self { documents: true, presentation: true, calcsheet: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    /// Built-in providers by id, and custom ones (`custom-1`, `custom-2`…).
    pub providers: BTreeMap<String, ProviderCfg>,
    /// Claude Code (the user's Claude plan, no key).
    pub claude_code: bool,
    /// "provider|model" for the writing assistant (check, rewrite).
    pub write_model: String,
    /// "provider|model" for the CalcSheet assistant; empty = the writing model.
    pub sheet_model: String,
    /// "google", or "provider|model" to translate with AI.
    pub translate_engine: String,
    /// Translate into this language (BCP 47); "auto" = Vietnamese ↔ English.
    pub translate_to: String,
    pub effort: String,
    pub thinking: bool,
    pub apps: AppsOn,
    /// Check the paragraph being written after a pause in typing.
    pub auto_check: bool,
    pub auto_delay_ms: u32,
    /// What to look for, beyond spelling and grammar.
    pub check_style: bool,
    /// Extra instructions for every writing request ("British spelling", "formal tone"…).
    pub instructions: String,
    /// Skills (SKILL.md folders) used for writing requests, by name.
    pub active_skills: Vec<String>,
    /// More folders holding skills (Themisio's own and Claude Code's are always read).
    pub skill_dirs: Vec<String>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            providers: BTreeMap::new(),
            claude_code: false,
            write_model: String::new(),
            sheet_model: String::new(),
            translate_engine: "google".into(),
            translate_to: "auto".into(),
            effort: String::new(),
            thinking: false,
            apps: AppsOn::default(),
            auto_check: false,
            auto_delay_ms: 2000,
            check_style: true,
            instructions: String::new(),
            active_skills: Vec::new(),
            skill_dirs: Vec::new(),
        }
    }
}

/// A provider as a request needs it.
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub wire: Wire,
    pub base_url: String,
    pub optional_key: bool,
}

impl AiSettings {
    pub fn cfg(&self, id: &str) -> ProviderCfg {
        self.providers.get(id).cloned().unwrap_or_default()
    }

    /// Every provider: the presets, then custom ones, then Claude Code.
    pub fn endpoints(&self) -> Vec<Endpoint> {
        let mut v: Vec<Endpoint> = PRESETS
            .iter()
            .map(|p| {
                let c = self.cfg(p.id);
                Endpoint {
                    id: p.id.into(),
                    name: p.name.into(),
                    wire: p.wire,
                    base_url: if c.base_url.trim().is_empty() { p.base_url.into() } else { c.base_url.trim().into() },
                    optional_key: p.optional_key,
                }
            })
            .collect();
        for (id, c) in &self.providers {
            if id.starts_with("custom") {
                v.push(Endpoint {
                    id: id.clone(),
                    name: if c.name.trim().is_empty() { "Custom".into() } else { c.name.trim().into() },
                    wire: Wire::OpenAi,
                    base_url: c.base_url.trim().into(),
                    optional_key: true,
                });
            }
        }
        v.push(Endpoint { id: "claudecode".into(), name: "Claude Code".into(), wire: Wire::ClaudeCode, base_url: String::new(), optional_key: true });
        v
    }

    pub fn endpoint(&self, id: &str) -> Option<Endpoint> {
        self.endpoints().into_iter().find(|e| e.id == id)
    }

    fn enabled(&self, id: &str) -> bool {
        if id == "claudecode" { self.claude_code } else { self.cfg(id).enabled }
    }

    /// Every "provider|model" the pickers offer, with a label, grouped by provider.
    pub fn choices(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut out = Vec::new();
        for e in self.endpoints() {
            if !self.enabled(&e.id) {
                continue;
            }
            let models: Vec<(String, String)> = if e.wire == Wire::ClaudeCode {
                let mut m: Vec<(String, String)> = CC_MODELS.iter().map(|(id, l)| (format!("claudecode|{id}"), format!("Claude Code · {l}"))).collect();
                for extra in &self.cfg("claudecode").models {
                    m.push((format!("claudecode|{extra}"), format!("Claude Code · {extra}")));
                }
                m
            } else {
                self.cfg(&e.id).models.iter().map(|m| (format!("{}|{m}", e.id), m.clone())).collect()
            };
            if !models.is_empty() {
                out.push((e.name.clone(), models));
            }
        }
        out
    }

    /// The model for `role`, falling back to the first one available.
    pub fn model_for(&self, preferred: &str) -> Option<String> {
        let all: Vec<String> = self.choices().into_iter().flat_map(|(_, m)| m.into_iter().map(|(v, _)| v)).collect();
        if !preferred.is_empty() && all.iter().any(|v| v == preferred) {
            return Some(preferred.to_string());
        }
        if !self.write_model.is_empty() && all.iter().any(|v| *v == self.write_model) {
            return Some(self.write_model.clone());
        }
        all.into_iter().next()
    }

    /// Whether any provider is ready to use.
    pub fn any_ready(&self) -> bool {
        !self.choices().is_empty()
    }

    /// The label of "provider|model".
    pub fn label(&self, value: &str) -> String {
        for (_, models) in self.choices() {
            if let Some((_, l)) = models.into_iter().find(|(v, _)| v == value) {
                return l;
            }
        }
        value.split_once('|').map_or(value.to_string(), |(_, m)| m.to_string())
    }
}

/// A request to a model.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// "provider|model".
    pub model: String,
    pub system: String,
    /// The conversation: (role "user" | "assistant", text). The last one is the user's.
    pub messages: Vec<(String, String)>,
    pub effort: String,
    pub thinking: bool,
    pub max_tokens: u32,
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn read(resp: ureq::http::Response<ureq::Body>) -> Result<(u16, String), String> {
    let status = resp.status().as_u16();
    let mut body = resp.into_body();
    let text = body.with_config().limit(32 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())?;
    Ok((status, text))
}

/// The provider's own error message from a failed response.
fn api_error(status: u16, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = v["error"]["message"].as_str().or_else(|| v["error"].as_str()).or_else(|| v["message"].as_str()).or_else(|| v[0]["error"]["message"].as_str());
    let hint = match status {
        401 | 403 => " — check the API key in Settings ▸ AI",
        404 => " — check the model name and the provider's address",
        429 => " — the provider's rate limit or quota was reached",
        _ => "",
    };
    match msg {
        Some(m) => format!("{status}: {m}{hint}"),
        None => format!("{status}: {}{hint}", body.chars().take(300).collect::<String>()),
    }
}

/// Run `req` and return the model's answer (its text). Blocking: call from a worker thread.
pub fn complete(settings: &AiSettings, req: &Request, cancel: &AtomicBool) -> Result<String, String> {
    let (provider, model) = req.model.split_once('|').ok_or_else(|| "Choose a model in Settings ▸ AI.".to_string())?;
    let ep = settings.endpoint(provider).ok_or_else(|| format!("Unknown provider {provider}"))?;
    match ep.wire {
        Wire::Anthropic => anthropic(&ep, model, req),
        Wire::OpenAi => openai(&ep, model, req),
        Wire::ClaudeCode => claude_code(model, req, cancel),
    }
}

fn claude_new_gen(m: &str) -> bool {
    m.contains("opus-4-7") || m.contains("opus-4-8") || (m.contains("-5") && !m.contains("-4-5") && !m.contains("3-5"))
}
fn claude_adaptive(m: &str) -> bool {
    claude_new_gen(m) || m.contains("opus-4-6") || m.contains("sonnet-4-6")
}

fn anthropic(ep: &Endpoint, model: &str, req: &Request) -> Result<String, String> {
    let key = keys::get(&ep.id).ok_or("No API key for Claude — add it in Settings ▸ AI.")?;
    let base = ep.base_url.trim_end_matches('/');
    let think = req.thinking;
    let max = if req.max_tokens > 0 { req.max_tokens } else if think { 16000 } else { 4096 };
    let messages: Vec<Value> = req.messages.iter().map(|(r, t)| json!({"role": if r == "assistant" { "assistant" } else { "user" }, "content": t})).collect();
    let mut body = json!({"model": model, "max_tokens": max, "messages": messages});
    if !req.system.is_empty() {
        body["system"] = json!(req.system);
    }
    if think {
        body["thinking"] = if claude_adaptive(model) { json!({"type": "adaptive"}) } else { json!({"type": "enabled", "budget_tokens": (max / 2).max(1024)}) };
    }
    if !req.effort.is_empty() && (claude_adaptive(model) || model.contains("opus-4-5")) {
        let e = if (req.effort == "xhigh" || req.effort == "max") && !claude_new_gen(model) { "high" } else { req.effort.as_str() };
        body["output_config"] = json!({"effort": e});
    }
    let resp = agent(240)
        .post(&format!("{base}/v1/messages"))
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(|e| e.to_string())?;
    let (status, text) = read(resp)?;
    if status >= 400 {
        return Err(api_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let out: String = v["content"].as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
    Ok(out)
}

fn openai(ep: &Endpoint, model: &str, req: &Request) -> Result<String, String> {
    let key = keys::get(&ep.id);
    if key.is_none() && !ep.optional_key {
        return Err(format!("No API key for {} — add it in Settings ▸ AI.", ep.name));
    }
    let base = ep.base_url.trim_end_matches('/');
    if base.is_empty() {
        return Err(format!("{} has no address — set it in Settings ▸ AI.", ep.name));
    }
    let mut messages = Vec::new();
    if !req.system.is_empty() {
        messages.push(json!({"role": "system", "content": req.system}));
    }
    for (r, t) in &req.messages {
        messages.push(json!({"role": if r == "assistant" { "assistant" } else { "user" }, "content": t}));
    }
    let mut body = json!({"model": model, "messages": messages});
    if req.max_tokens > 0 {
        body["max_tokens"] = json!(req.max_tokens);
    }
    // Reasoning models take an effort; "off" thinking asks for the least.
    let effort = match (req.effort.as_str(), req.thinking) {
        ("", true) => Some("medium"),
        ("", false) => None,
        ("low", _) => Some("low"),
        ("medium", _) => Some("medium"),
        (_, _) => Some("high"),
    };
    if let Some(e) = effort {
        body["reasoning_effort"] = json!(e);
    }
    let send = |body: &Value| -> Result<(u16, String), String> {
        let mut r = agent(240).post(&format!("{base}/chat/completions")).header("content-type", "application/json");
        if let Some(k) = &key {
            r = r.header("authorization", &format!("Bearer {k}"));
        }
        read(r.send(body.to_string()).map_err(|e| e.to_string())?)
    };
    let (mut status, mut text) = send(&body)?;
    // A model that takes no effort setting: try again without it.
    if status == 400 && body.get("reasoning_effort").is_some() && text.to_lowercase().contains("reasoning") {
        if let Some(o) = body.as_object_mut() {
            o.remove("reasoning_effort");
        }
        (status, text) = send(&body)?;
    }
    if status >= 400 {
        return Err(api_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let content = &v["choices"][0]["message"]["content"];
    let out = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect(),
        _ => String::new(),
    };
    // Some reasoning models put their thinking in the answer between <think> tags.
    Ok(strip_think(&out))
}

fn strip_think(s: &str) -> String {
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if b < a {
            break;
        }
        out.replace_range(a..b + "</think>".len(), "");
    }
    out.trim().to_string()
}

// ---------------- Claude Code ----------------

fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from)
}

/// Where the `claude` program is (PATH, or where its installers put it).
pub fn find_claude() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(h) = home() {
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".claude").join("local"));
    }
    if let Some(a) = std::env::var_os("APPDATA") {
        dirs.push(std::path::PathBuf::from(a).join("npm"));
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(std::path::PathBuf::from(l).join("Microsoft").join("WinGet").join("Links"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    for d in dirs {
        for e in exts {
            let p = d.join(format!("claude{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn hidden(exe: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    // An API key in the environment would silently take precedence over the user's Claude plan.
    c.env_remove("ANTHROPIC_API_KEY");
    c
}

/// Run a command, giving up after `timeout` (or when `cancel` is set).
fn run_for(mut c: std::process::Command, input: Option<String>, timeout: Duration, cancel: Option<&AtomicBool>) -> Result<std::process::Output, String> {
    use std::io::{Read, Write};
    c.stdin(if input.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    if let (Some(text), Some(mut si)) = (input, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = si.write_all(text.as_bytes());
        });
    }
    let mut so = child.stdout.take();
    let mut se = child.stderr.take();
    let t1 = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(s) = so.as_mut() {
            let _ = s.read_to_end(&mut v);
        }
        v
    });
    let t2 = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(s) = se.as_mut() {
            let _ = s.read_to_end(&mut v);
        }
        v
    });
    let started = std::time::Instant::now();
    let status = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break s;
        }
        if started.elapsed() > timeout || cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if started.elapsed() > timeout { "Claude Code took too long".into() } else { "Stopped".into() });
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    Ok(std::process::Output { status, stdout: t1.join().unwrap_or_default(), stderr: t2.join().unwrap_or_default() })
}

fn claude_code(model: &str, req: &Request, cancel: &AtomicBool) -> Result<String, String> {
    let exe = find_claude().ok_or("Claude Code isn't installed — Settings ▸ AI ▸ Claude Code")?;
    let dir = std::env::temp_dir().join("themisio-claude");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let sys = dir.join(format!("system-{}-{}.md", std::process::id(), rand_id()));
    std::fs::write(&sys, format!("{}\n\n# How to answer\nYou are answering inside Themisio, a document editor. Use no tools; answer directly, following the instructions above exactly.", req.system))
        .map_err(|e| e.to_string())?;
    let prompt = if req.messages.len() <= 1 {
        req.messages.last().map(|m| m.1.clone()).unwrap_or_default()
    } else {
        let mut p = String::from("Conversation so far:\n\n");
        for (r, t) in &req.messages[..req.messages.len() - 1] {
            p.push_str(&format!("{}: {t}\n\n", if r == "assistant" { "Assistant" } else { "User" }));
        }
        p.push_str(&format!("Now reply to the user's last message:\n\n{}", req.messages.last().map(|m| m.1.as_str()).unwrap_or("")));
        p
    };
    let mut c = hidden(&exe);
    c.current_dir(&dir).args(["-p", "--output-format", "json", "--max-turns", "1"]);
    c.args(["--disallowedTools", "Bash,PowerShell,Read,Edit,MultiEdit,Write,NotebookEdit,Glob,Grep,WebFetch,WebSearch,Task,Agent,TodoWrite,Skill"]);
    c.arg("--append-system-prompt-file").arg(&sys);
    if !model.is_empty() && model != "default" {
        c.args(["--model", model]);
    }
    if ["low", "medium", "high", "xhigh", "max"].contains(&req.effort.as_str()) {
        c.args(["--effort", &req.effort]);
    }
    if !req.thinking {
        c.env("MAX_THINKING_TOKENS", "0");
    }
    let out = run_for(c, Some(prompt), Duration::from_secs(300), Some(cancel));
    let _ = std::fs::remove_file(&sys);
    let out = out?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Null);
    if v["is_error"].as_bool() == Some(true) {
        return Err(format!("Claude Code: {}", v["result"].as_str().unwrap_or("it reported an error")));
    }
    if let Some(r) = v["result"].as_str() {
        return Ok(r.trim().to_string());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let msg = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it stopped without an answer").to_string();
    if msg.to_lowercase().contains("login") || msg.to_lowercase().contains("auth") {
        return Err("Claude Code isn't signed in — Settings ▸ AI ▸ Claude Code ▸ Sign in".into());
    }
    Err(format!("Claude Code: {msg}"))
}

fn rand_id() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.finish()
}

/// Claude Code on this PC: installed (and its version), and who is signed in.
#[derive(Clone, Debug, Default)]
pub struct CcStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub signed_in: bool,
    pub account: Option<String>,
}

pub fn claude_code_status() -> CcStatus {
    let Some(exe) = find_claude() else { return CcStatus::default() };
    let mut c = hidden(&exe);
    c.arg("--version");
    let version = run_for(c, None, Duration::from_secs(30), None).ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty());
    let mut c = hidden(&exe);
    c.args(["auth", "status"]);
    let (signed_in, account) = match run_for(c, None, Duration::from_secs(30), None) {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            let v: Value = serde_json::from_str(text.trim()).unwrap_or(Value::Null);
            let method = v["authMethod"].as_str().unwrap_or("");
            let signed = v["loggedIn"].as_bool().unwrap_or(o.status.success() && !method.is_empty() && method != "none");
            let account = v["email"].as_str().or_else(|| v["account"]["email"].as_str()).or_else(|| v["emailAddress"].as_str()).map(str::to_string);
            (signed, account)
        }
        Err(_) => (false, None),
    };
    CcStatus { installed: true, version, signed_in, account }
}

/// Claude Code's own sign-in, in a console window (it opens the browser).
pub fn claude_code_sign_in() -> Result<(), String> {
    let exe = find_claude().ok_or("Claude Code isn't installed")?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let mut c = std::process::Command::new("cmd.exe");
        c.raw_arg(format!("/k \"title Sign in to Claude Code && \"{}\" auth login && echo. && echo Signed in - you can close this window.\"", exe.display()));
        c.env_remove("ANTHROPIC_API_KEY");
        c.creation_flags(CREATE_NEW_CONSOLE);
        c.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
        Err("Run `claude auth login` in a terminal".into())
    }
}

pub fn claude_code_sign_out() -> Result<(), String> {
    let exe = find_claude().ok_or("Claude Code isn't installed")?;
    let mut c = hidden(&exe);
    c.args(["auth", "logout"]);
    run_for(c, None, Duration::from_secs(30), None).map(|_| ())
}

/// The models a provider offers (its `/models` list).
pub fn list_models(ep: &Endpoint) -> Result<Vec<String>, String> {
    let key = keys::get(&ep.id);
    let base = ep.base_url.trim_end_matches('/');
    let resp = match ep.wire {
        Wire::Anthropic => {
            let key = key.ok_or("Add the API key first")?;
            agent(30).get(&format!("{base}/v1/models?limit=100")).header("x-api-key", &key).header("anthropic-version", "2023-06-01").call()
        }
        Wire::OpenAi => {
            let mut r = agent(30).get(&format!("{base}/models"));
            if let Some(k) = &key {
                r = r.header("authorization", &format!("Bearer {k}"));
            }
            r.call()
        }
        Wire::ClaudeCode => return Ok(CC_MODELS.iter().map(|(m, _)| m.to_string()).collect()),
    }
    .map_err(|e| e.to_string())?;
    let (status, text) = read(resp)?;
    if status >= 400 {
        return Err(api_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .or_else(|| v["models"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()))
        .map(|s| s.trim_start_matches("models/").to_string())
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Google Translate (the free web endpoint, no key): `text` into `to`. Returns the translation
/// and the detected source language.
pub fn google_translate(text: &str, to: &str) -> Result<(String, String), String> {
    let resp = agent(30)
        .get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", to)
        .query("dt", "t")
        .query("q", text)
        .call()
        .map_err(|e| e.to_string())?;
    let (status, body) = read(resp)?;
    if status >= 400 {
        return Err(format!("Google Translate: {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let out: String = v[0].as_array().into_iter().flatten().filter_map(|seg| seg[0].as_str()).collect();
    let src = v[2].as_str().unwrap_or("").to_string();
    if out.is_empty() {
        return Err("Google Translate returned nothing".into());
    }
    Ok((out, src))
}

/// Work on a thread whose result the interface picks up later.
pub struct Job<T> {
    slot: Arc<Mutex<Option<Result<T, String>>>>,
    cancel: Arc<AtomicBool>,
    pub started: std::time::Instant,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(ctx: &egui::Context, f: impl FnOnce(&AtomicBool) -> Result<T, String> + Send + 'static) -> Self {
        let slot = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let (s, c, ctx) = (slot.clone(), cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&c))).unwrap_or_else(|_| Err("The request failed unexpectedly".into()));
            if let Ok(mut g) = s.lock() {
                *g = Some(r);
            }
            ctx.request_repaint();
        });
        Self { slot, cancel, started: std::time::Instant::now() }
    }

    /// The result, once (None while it runs).
    pub fn take(&self) -> Option<Result<T, String>> {
        self.slot.lock().ok().and_then(|mut g| g.take())
    }

}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The first JSON object in a model's answer (models sometimes wrap it in prose or a code fence).
pub fn json_in(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Some(v);
    }
    let body = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).map(|s| s.trim_end_matches("```")).unwrap_or(t);
    if let Ok(v) = serde_json::from_str::<Value>(body.trim()) {
        return Some(v);
    }
    let a = t.find('{')?;
    let b = t.rfind('}')?;
    serde_json::from_str(t.get(a..=b)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_found_in_prose_and_fences() {
        assert_eq!(json_in("```json\n{\"a\":1}\n```").and_then(|v| v["a"].as_i64()), Some(1));
        assert_eq!(json_in("Here: {\"a\": 2} done").and_then(|v| v["a"].as_i64()), Some(2));
        assert!(json_in("no json").is_none());
    }

    #[test]
    fn thinking_tags_are_removed() {
        assert_eq!(strip_think("<think>hmm</think>Answer"), "Answer");
    }

    #[test]
    fn the_first_ready_model_is_used() {
        let mut s = AiSettings::default();
        assert!(s.model_for("").is_none());
        s.providers.insert("gemini".into(), ProviderCfg { enabled: true, models: vec!["gemini-x".into()], ..Default::default() });
        assert_eq!(s.model_for("").as_deref(), Some("gemini|gemini-x"));
        s.claude_code = true;
        assert_eq!(s.model_for("claudecode|claude-opus-5-5").as_deref(), Some("claudecode|claude-opus-5-5"));
    }
}
