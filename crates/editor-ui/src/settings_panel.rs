//! Full-window settings overlay content (M28) — text-first; GPU draws the returned lines.

use std::time::Instant;

use editor_settings::{LineEndingPreference, Settings, KNOWN_PROVIDER_KEYS};

/// Left-nav section (order matches [`Section::ALL`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    AiProviders,
    Editor,
    Terminal,
    Skills,
    Keybindings,
    About,
}

impl Section {
    pub const ALL: &[Section] = &[
        Section::AiProviders,
        Section::Editor,
        Section::Terminal,
        Section::Skills,
        Section::Keybindings,
        Section::About,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Section::AiProviders => "AI Providers",
            Section::Editor => "Editor",
            Section::Terminal => "Terminal",
            Section::Skills => "Skills",
            Section::Keybindings => "Keybindings",
            Section::About => "About",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Section::AiProviders => Section::Editor,
            Section::Editor => Section::Terminal,
            Section::Terminal => Section::Skills,
            Section::Skills => Section::Keybindings,
            Section::Keybindings => Section::About,
            Section::About => Section::AiProviders,
        }
    }

    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Section::AiProviders => Section::About,
            Section::Editor => Section::AiProviders,
            Section::Terminal => Section::Editor,
            Section::Skills => Section::Terminal,
            Section::Keybindings => Section::Skills,
            Section::About => Section::Keybindings,
        }
    }
}

/// UI-only state (persisted editor state is in [`Settings`] / `SettingsStore`).
#[derive(Debug, Clone)]
pub struct SettingsPanelState {
    pub section: Section,
    /// Row index in the synthetic control list for the active section.
    pub row: usize,
    pub filter_keybindings: String,
    pub filter_skills: String,
    /// `Some(provider_id)` while the user is typing a new API key.
    pub key_edit_provider: Option<String>,
    pub key_edit_buffer: String,
    /// Last user-facing status (test result, save confirm, etc.).
    pub message: Option<String>,
    /// `reset_confirm.0` is the section, `.1` is deadline for second press.
    pub reset_confirm: Option<(Section, Instant)>,
}

impl Default for SettingsPanelState {
    fn default() -> Self {
        Self {
            section: Section::AiProviders,
            row: 0,
            filter_keybindings: String::new(),
            filter_skills: String::new(),
            key_edit_provider: None,
            key_edit_buffer: String::new(),
            message: None,
            reset_confirm: None,
        }
    }
}

/// Row snapshot for the Skills section (filled by the app from `SkillRegistry`).
#[derive(Debug, Clone)]
pub struct SkillRow {
    pub id: String,
    pub source: String,
    pub enabled: bool,
}

/// Build lines for the GPU text overlay (~14px monospace).
#[must_use]
pub fn format_settings_overlay(
    settings: &Settings,
    state: &SettingsPanelState,
    keyring_present: impl Fn(&str) -> bool,
    skills: &[SkillRow],
    about: &AboutStrings,
) -> Vec<String> {
    let mut out = vec![
        "IDE settings — Esc closes".to_string(),
        "(Tab / Shift+Tab move focus, Space toggles, Enter activates)".to_string(),
        String::new(),
    ];

    let mut nav = String::from("Sections: ");
    for s in Section::ALL {
        if *s == state.section {
            nav.push_str(&format!("[{}] ", s.label()));
        } else {
            nav.push_str(&format!("{}  ", s.label()));
        }
    }
    out.push(nav);
    out.push(String::new());

    match state.section {
        Section::AiProviders => push_ai(settings, state, &keyring_present, &mut out),
        Section::Editor => push_editor(settings, state, &mut out),
        Section::Terminal => push_terminal(settings, &mut out),
        Section::Skills => push_skills(settings, state, skills, &mut out),
        Section::Keybindings => push_keybindings(state, &mut out),
        Section::About => push_about(about, &mut out),
    }

    if out.len() > 80 {
        out.truncate(80);
        out.push("… (truncated)".into());
    }
    out
}

fn push_ai(
    settings: &Settings,
    state: &SettingsPanelState,
    keyring_present: &impl Fn(&str) -> bool,
    out: &mut Vec<String>,
) {
    out.push("Global".into());
    out.push(format!(
        "  Active provider:  {}",
        settings.ai.active_provider.as_deref().unwrap_or("(use row focus + Space to cycle)")
    ));
    out.push(format!(
        "  Active model:     {}",
        settings.ai.active_model.as_deref().unwrap_or("(provider default)")
    ));
    out.push(String::new());

    if let Some(ref prov) = state.key_edit_provider {
        let masked: String = "•".repeat(state.key_edit_buffer.len());
        out.push(format!("  Editing API key for `{prov}` — Enter save, Esc cancel"));
        out.push(format!("  {masked}"));
        out.push(String::new());
    }

    if let Some(m) = &state.message {
        out.push(format!("  {m}"));
        out.push(String::new());
    }

    out.push("Providers (Enter = set key, Space = toggle enabled)".into());
    for (i, id) in KNOWN_PROVIDER_KEYS.iter().enumerate() {
        let pc = settings.ai.providers.get(*id);
        let enabled = pc.map(|p| p.enabled).unwrap_or(false);
        let dm = pc.map(|p| p.default_model.as_str()).unwrap_or("");
        let base = pc.and_then(|p| p.base_url.as_deref()).unwrap_or("");
        let on = if enabled { "[x]" } else { "[ ]" };
        let needs_key = *id != "ollama";
        let has_key = if needs_key { keyring_present(id) } else { true };
        let key_stat: String = if *id == "ollama" {
            "(no API key)".to_string()
        } else if has_key {
            "key set".to_string()
        } else {
            "no key".to_string()
        };

        let mark = if state.row == i { ">" } else { " " };
        let mut line = format!("{mark} {on} {id:<10}  {key_stat:<9}  model `{dm}`",);
        if *id == "custom" {
            line.push_str(&format!(
                "  base `{}`",
                if base.is_empty() { "(required)" } else { base }
            ));
        } else if *id == "ollama" && !base.is_empty() {
            line.push_str(&format!("  base `{base}`"));
        }
        out.push(line);
    }

    out.push(String::new());
    out.push("Advanced".into());
    let base = KNOWN_PROVIDER_KEYS.len();
    for (j, label, on) in [
        (0, "Summarizer", settings.ai.enabled_summarizer),
        (1, "Vector index", settings.ai.enabled_vector_index),
    ] {
        let row = base + j;
        let mark = if state.row == row { ">" } else { " " };
        let v = if on { "[x]" } else { "[ ]" };
        out.push(format!("{mark}  {label:<18}  {v}"));
    }
    let row_mt = base + 2;
    let mark_mt = if state.row == row_mt { ">" } else { " " };
    out.push(format!("{}  Max tokens default  {}", mark_mt, settings.ai.max_tokens_default));
    let row_t = base + 3;
    let mark_t = if state.row == row_t { ">" } else { " " };
    out.push(format!(
        "{}  Temperature default {}",
        mark_t,
        settings.ai.temperature_default.map(|f| f.to_string()).unwrap_or_else(|| "(unset)".into())
    ));
}

fn push_editor(settings: &Settings, state: &SettingsPanelState, out: &mut Vec<String>) {
    let e = &settings.editor;
    let m = |i: usize| -> &'static str {
        if state.row == i {
            ">"
        } else {
            " "
        }
    };
    out.push(format!("{} Font size (px):       {:.1}", m(0), e.font_size));
    out.push(format!(
        "{} Line ending:          {}",
        m(1),
        match e.line_ending {
            LineEndingPreference::Auto => "auto",
            LineEndingPreference::Lf => "LF",
            LineEndingPreference::Crlf => "CRLF",
        }
    ));
    out.push(format!(
        "{} Trim trailing space: [{}]",
        m(2),
        if e.trim_trailing_whitespace_on_save { "x" } else { " " }
    ));
    out.push(format!(
        "{} Newline at end:       [{}]",
        m(3),
        if e.ensure_newline_at_eof { "x" } else { " " }
    ));
    out.push(format!("{} Word wrap:            [{}]", m(4), if e.word_wrap { "x" } else { " " }));
    out.push(format!("{} Undo coalesce (ms):   {}", m(5), e.undo_coalesce_ms));
}

fn push_terminal(settings: &Settings, out: &mut Vec<String>) {
    let t = &settings.terminal;
    out.push(format!(
        "  Shell override: {}",
        t.shell_override
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(empty — auto-detect)".into())
    ));
    out.push(format!(
        "  Shell args: {}",
        if t.shell_args.is_empty() { "(none)".into() } else { t.shell_args.join(" ") }
    ));
    out.push(format!("  Font size (px): {:.1}", t.font_size));
    out.push(format!("  Scrollback lines: {}", t.scrollback_lines));
    out.push(format!("  Default pane height: {:.0}% of window", t.default_height_pct));
}

fn push_skills(
    settings: &Settings,
    state: &SettingsPanelState,
    skills: &[SkillRow],
    out: &mut Vec<String>,
) {
    let f = state.filter_skills.trim().to_lowercase();
    out.push(format!("  Filter: `{}`  (/ focuses)", state.filter_skills));
    out.push(String::new());
    out.push("  Registered skills".into());
    for s in skills {
        if !f.is_empty() && !s.id.to_lowercase().contains(&f) {
            continue;
        }
        let on = if s.enabled { "[x]" } else { "[ ]" };
        out.push(format!("  {on}  {:<28}  ({})", s.id, s.source));
    }
    out.push(String::new());
    out.push("  Extra skill directories".into());
    if settings.extra_skill_dirs.is_empty() {
        out.push("    (none)".into());
    } else {
        for p in &settings.extra_skill_dirs {
            out.push(format!("    {}", p.display()));
        }
    }
}

fn push_keybindings(state: &SettingsPanelState, out: &mut Vec<String>) {
    let f = state.filter_keybindings.trim().to_lowercase();
    out.push(format!("  Filter: `{}`  (/ focuses)", state.filter_keybindings));
    out.push(String::new());
    for (cmd, keys) in crate::keybindings::DEFAULT_KEYMAP {
        if !f.is_empty() && !cmd.to_lowercase().contains(&f) && !keys.to_lowercase().contains(&f) {
            continue;
        }
        out.push(format!("  {:<38} {}", cmd, keys));
    }
}

#[derive(Debug, Clone, Default)]
pub struct AboutStrings {
    pub version: String,
    pub git_hash: String,
    pub build_date: String,
    pub repo_url: String,
    pub docs_url: String,
    pub changelog_url: String,
}

fn push_about(about: &AboutStrings, out: &mut Vec<String>) {
    out.push(format!("  {}", about.version));
    if !about.git_hash.is_empty() {
        out.push(format!("  Commit: {}", about.git_hash));
    }
    out.push(format!("  Built: {}", about.build_date));
    out.push(format!("  Repository: {}", about.repo_url));
    out.push(format!("  Docs: {}", about.docs_url));
    out.push(format!("  Changelog: {}", about.changelog_url));
}

/// Advance row + wrap for testing / future mouse map.
#[must_use]
pub fn max_rows_for_section(section: Section) -> usize {
    match section {
        Section::AiProviders => KNOWN_PROVIDER_KEYS.len().saturating_add(3),
        Section::Editor => 5,
        Section::Terminal => 5,
        Section::Skills => 64,
        Section::Keybindings => crate::keybindings::DEFAULT_KEYMAP.len() + 4,
        Section::About => 6,
    }
}
