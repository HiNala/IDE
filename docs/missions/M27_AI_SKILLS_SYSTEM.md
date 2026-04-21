# M27 — AI Skills System

**Mission ID:** M27
**Prerequisites:** M19 complete (provider abstraction + OpenAI working) and M26 complete (terminal exists).
**Output:** A skills system compatible with Anthropic's `SKILL.md` standard (also the emerging cross-tool convention used by Cursor, Gemini CLI, Codex CLI, and Antigravity IDE). Built-in skills teach the model how to use *this* IDE — its tools, its terminal, its git conventions, the user's current OS and shell. User- and team-authored skills (per-project `.ide/skills/*`) add domain-specific know-how. Skills use progressive disclosure: only names and descriptions preload into context; bodies are fetched via a `load_skill` tool only when the model decides a skill is relevant. A small registry + loader + discovery surface; no UI beyond a list in settings. This makes the agent dramatically more effective on IDE-specific workflows.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 3 agent substrate.
- M19 doc — provider abstraction that M27 plugs into.
- M26 doc — terminal-usage patterns that need to become skills.
- `https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview` — Anthropic's canonical skills docs.
- `https://github.com/anthropics/skills` — example skills repository; our built-in skills mimic this structure.
- `https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills` — design rationale. "Progressive disclosure" and "specialize via composable resources" are the key ideas.

---

## The Situation In Plain English

Frontier models know a lot about code in general. They don't know anything about our IDE specifically — which tools we expose, how our terminal is wired up, what the user's shell and OS are, what project-specific conventions matter. Before skills, the only way to give them that context was to cram it into the system prompt and pay for those tokens on every single turn. Before skills, every agent turn re-spends tokens describing how `run_terminal` works.

Anthropic's Skills pattern solves this with progressive disclosure. At session start, the agent's system prompt lists all installed skills — just `name` + `description` for each, typically 10-30 tokens per skill, maybe a few hundred tokens total. The model sees the catalog, not the content. When the model encounters a task that matches a skill (because the description says when to use it), it calls a tool to load that specific skill's body — the detailed instructions, examples, and references — into context. Only then does the skill's content cost tokens, and only for the turns that actually need it.

A skill is a directory. A directory with a `SKILL.md` file is a valid skill. `SKILL.md` has a YAML frontmatter (at least a `name` and a `description` — "pushy" descriptions work best because models under-trigger skills) plus a markdown body with step-by-step instructions, examples, and references. The body can reference other files in the same directory via relative paths; the model loads those on demand too.

We ship a set of built-in skills every fresh install carries:

- **`using-terminal`** — how the `run_terminal` tool works, how to interpret exit codes, how to handle long-running commands, when to prefer it over `run_shell`.
- **`using-git`** — how to read git state via read-only tools, how to *propose* (never execute) git write operations through the terminal, common patterns for investigating history.
- **`writing-rust`** / **`writing-python`** / **`writing-typescript`** — language-specific conventions we bake in. Start minimal; these become a living set.
- **`ide-conventions`** — how our `WorkspaceTx` transaction model works, why edits come back as diffs for approval, how sidecar metadata gets written, when to expect the user to intervene.
- **`system-info`** — a *dynamic* skill that reports the actual OS, architecture, shell, git version, available commands. Every turn reads this afresh so the model doesn't work from stale info.

User-authored skills live in `.ide/skills/<name>/SKILL.md`. Per-project skills ship with the repo and are picked up automatically when the workspace opens. Anthropic's open skills repo format is a near-drop-in match; users can copy skills from https://github.com/anthropics/skills or build their own.

The model-facing surface is two pieces:

1. Names + descriptions injected into the system prompt at turn start. Format: `<available_skills>\n  <skill name="using-terminal" description="..." />\n  ...\n</available_skills>`.
2. A `load_skill(name: string) -> string` tool registered in M20's tool registry. When called, returns the SKILL.md body. The model can then call `load_skill_reference(name: string, file: string)` to pull in referenced files on demand.

That's all. Simple, composable, and a dramatic multiplier on agent effectiveness for our specific environment.

---

## Scope

**In scope:**
- New `editor-skills` crate.
- `SkillRegistry` for enumerating installed skills.
- `SkillLoader` for reading SKILL.md + resolving referenced files.
- YAML frontmatter parsing (reuse the serde_yaml setup from M21).
- Discovery: built-in skills bundled with the binary + per-user (`~/.config/ide/skills/`) + per-workspace (`.ide/skills/`).
- System prompt injection helper: `SkillRegistry::summary_for_system_prompt() -> String`.
- Tools registered with M20's ToolRegistry: `load_skill`, `list_skills`, `load_skill_reference`.
- Built-in skills shipped:
  1. `using-terminal`
  2. `using-git`
  3. `ide-conventions`
  4. `writing-rust` (minimal)
  5. `writing-python` (minimal)
  6. `writing-typescript` (minimal)
  7. `system-info` (dynamic — content generated at load time from runtime detection)
- Settings integration (via M28): list installed skills, enable/disable per-skill, point at additional skill directories.
- Cross-provider compatibility: the injection format works identically for OpenAI, Anthropic, Gemini, Ollama.

**Out of scope:**
- A UI for authoring or editing skills (users edit markdown files directly — correct default).
- Skill versioning or update mechanisms (V4+).
- Skill sandboxing / security review beyond the "trust what you installed" model Anthropic uses (we inherit the same trust assumption).
- Executable scripts bundled with skills (Anthropic's skills can include scripts the model runs via bash; we don't support that in V3 because our shell tool is more constrained — revisit in V4 when we build out more tool expressiveness).
- Fuzzy triggering beyond the description-match the model does naturally.
- A "skill marketplace" browser.
- Per-skill resource constraints (Brian's "how much memory / CPU" idea). Resource reporting goes in `system-info`; actual constraints aren't something we can enforce usefully without a full sandbox, which is out of scope.

---

## North Star

A user asks: "Run the test suite and fix any failures." The model's system prompt lists `using-terminal`, `using-git`, `ide-conventions`, `writing-rust`, `system-info` among available skills. The model calls `load_skill("system-info")` — now knows we're on Windows 11 with PowerShell and rustc 1.80. Calls `load_skill("using-terminal")` — now knows it should call `run_terminal("cargo test")` and interpret the output. Calls `run_terminal("cargo test")`. A test fails. The model reads the failure, locates the file via `grep`, proposes a diff, which the user approves through M23's normal flow. At no point does the model waste tokens on skills it doesn't need (`writing-python`, `writing-typescript` — never loaded).

---

## TODO List

### 1. Create `editor-skills` crate

- [ ] 1.1. `cargo new --lib crates/editor-skills`. Deps: `serde`, `serde_yaml`, `thiserror`, `tracing`, `editor-core`, `editor-workspace`.
- [ ] 1.2. Commit: `feat(skills): scaffold editor-skills crate`.

### 2. Skill schema

- [ ] 2.1. `src/schema.rs`:
  ```rust
  pub struct Skill {
      pub name: String,
      pub description: String,
      pub source: SkillSource,
      pub path: PathBuf,           // directory containing SKILL.md
      pub body: String,            // SKILL.md markdown body (without frontmatter)
      pub allowed_tools: Option<Vec<String>>,
      pub disable_model_invocation: bool,
      pub compatibility: Option<String>,
  }
  pub enum SkillSource {
      Builtin,
      UserGlobal,
      Workspace,
  }
  ```
- [ ] 2.2. `parse_skill_md(content: &str) -> Result<ParsedSkill, SkillParseError>` — splits at `---\n` boundaries, parses YAML via `serde_yaml`, returns `(frontmatter_fields, body_string)`.
- [ ] 2.3. Commit: `feat(skills): schema and parser`.

### 3. `SkillRegistry`

- [ ] 3.1. `src/registry.rs`:
  ```rust
  pub struct SkillRegistry {
      skills: HashMap<String, Skill>,
      enabled: HashSet<String>,
  }
  impl SkillRegistry {
      pub fn load(workspace_root: Option<&Path>) -> Self;
      pub fn list(&self) -> Vec<&Skill>;
      pub fn get(&self, name: &str) -> Option<&Skill>;
      pub fn enabled_skills(&self) -> impl Iterator<Item = &Skill>;
      pub fn set_enabled(&mut self, name: &str, enabled: bool);
      pub fn summary_for_system_prompt(&self) -> String;
      pub fn reload(&mut self, workspace_root: Option<&Path>);
  }
  ```
- [ ] 3.2. Load order (later overrides earlier on name conflict):
  1. Built-in skills bundled in the binary via `include_str!`.
  2. `~/.config/ide/skills/*/SKILL.md` (Windows: `%APPDATA%\ide\skills`; macOS: `~/Library/Application Support/ide/skills`).
  3. `<workspace>/.ide/skills/*/SKILL.md`.
- [ ] 3.3. `summary_for_system_prompt` format:
  ```
  <available_skills>
    <skill name="using-terminal">Shows the agent how to use the integrated terminal via run_terminal. Use whenever the task requires running shell commands, installing dependencies, running tests, or inspecting process state.</skill>
    <skill name="using-git">Read-only git workflow. Use when the task involves inspecting git history, diffs, branches, or deciding whether a change is safe.</skill>
    ...
  </available_skills>
  ```
  Pushy descriptions — per Anthropic's skill-creator guidance, models under-trigger; descriptions should over-index on "use this whenever ...".
- [ ] 3.4. Enabled state persists in `PersistedState`.
- [ ] 3.5. Commit: `feat(skills): registry with precedence-ordered discovery`.

### 4. Built-in skill: `using-terminal`

- [ ] 4.1. `crates/editor-skills/assets/builtin/using-terminal/SKILL.md`:
  ```markdown
  ---
  name: using-terminal
  description: How to run shell commands through the integrated terminal. Use this skill whenever the task requires executing any shell command — running tests, building code, installing dependencies, inspecting files with standard Unix tools, starting dev servers, or running any CLI tool. Prefer this over any other shell access.
  ---

  # Using the integrated terminal

  This IDE has an integrated terminal that runs the user's native shell. The `run_terminal` tool runs commands in that terminal. The user sees exactly what you execute. This is good — the user can intervene — and means you should be transparent and predictable.

  ## When to use run_terminal vs run_shell

  - Prefer `run_terminal(command)` for everything the user would reasonably want to watch happen.
  - Use `run_shell` only for commands whose output is pure data the model consumes (e.g., reading a JSON file you can't read another way) and where showing the user noise would be worse than useful.

  ## Interpreting output

  `run_terminal` returns captured output and an exit code. Non-zero exit is failure. Read the last ~50 lines carefully — real errors are usually at the end, not the start.

  ## Long-running commands

  If you start a server or watcher, the `run_terminal` call will time out. Instead:
  - Tell the user you'd like them to start the server manually and report back.
  - Or start it in the background with `&` (Unix) / `Start-Process` (PowerShell) and note that you can no longer read its output.

  ## Patterns

  ### Run the test suite
  ```
  run_terminal("cargo test")   # for Rust
  run_terminal("pytest")        # for Python
  run_terminal("npm test")      # for Node
  ```
  Read exit code and final lines to determine failure location.

  ### Install a dependency
  Always ask the user first for `cargo add`, `npm install`, etc. New dependencies are decisions the user should make consciously.

  ### Never
  - Never `rm -rf` without explicit user confirmation.
  - Never modify `~/.bashrc` or similar.
  - Never run commands you don't understand.
  ```
- [ ] 4.2. Commit: `feat(skills): built-in using-terminal skill`.

### 5. Built-in skill: `using-git`

- [ ] 5.1. `crates/editor-skills/assets/builtin/using-git/SKILL.md` — similar structure. Covers: reading git log / diff / blame via read-only tools M18 exposes; proposing `git add`/`git commit` through the terminal for the user to execute (NOT via `run_terminal` — present the command, let the user run it themselves so they control history); understanding the difference between the working tree and the index; when to suggest `git stash` vs other strategies.
- [ ] 5.2. Include concrete patterns for "check if a file has uncommitted changes," "look at the last commit that touched this file," "compare two branches."
- [ ] 5.3. Commit: `feat(skills): built-in using-git skill`.

### 6. Built-in skill: `ide-conventions`

- [ ] 6.1. Covers this IDE's specifics: `WorkspaceTx` transaction model; edit operations produce diffs for approval rather than instant commits; sidecar metadata is written on commit so the model should include meaningful reasoning in tool calls; how to read a file before editing (always); path safety (never attempt paths outside workspace root).
- [ ] 6.2. Explicitly describes the `edit_lines`/`insert_at`/`append_to`/`replace_in_file` preferred patterns over full-file rewrites.
- [ ] 6.3. Commit: `feat(skills): built-in ide-conventions skill`.

### 7. Built-in language skills (minimal)

- [ ] 7.1. `writing-rust`, `writing-python`, `writing-typescript` — each is a short SKILL.md with description like "Conventions and pitfalls when writing <lang> in this IDE. Use whenever editing or writing <lang> code." Content is intentionally thin at launch (think 20-40 lines each) with pointers to community style guides.
- [ ] 7.2. Commit: `feat(skills): built-in language skills (minimal)`.

### 8. Dynamic skill: `system-info`

- [ ] 8.1. Unlike the markdown skills, `system-info`'s *body* is generated at `load_skill` time from runtime detection:
  - OS: `std::env::consts::OS`.
  - Arch: `std::env::consts::ARCH`.
  - Shell: from the terminal config M26 determined.
  - Rust toolchain if project is Rust: `rustc --version` via a quick PTY call (cached).
  - Node version if `package.json` present.
  - Python version if `pyproject.toml` / `requirements.txt` present.
  - git version if in a git repo.
  - Workspace root path.
  - Available memory, CPU count, free disk on workspace's filesystem (useful for the model to know when offering to run heavy tests — addresses Brian's resource-awareness point).
- [ ] 8.2. Cached for 5 minutes; cache invalidates on workspace open/close.
- [ ] 8.3. Registry special-cases this: the description is static ("Current OS, shell, language toolchains, and project metadata. Use whenever any task depends on the environment."), but the body is generated on load.
- [ ] 8.4. Commit: `feat(skills): dynamic system-info skill`.

### 9. Tools registered with M20

- [ ] 9.1. `load_skill(name: string) -> string`:
  - Look up the skill; return its body.
  - If skill doesn't exist: return error listing enabled skills.
  - For `system-info`: run the detection, return the generated body.
- [ ] 9.2. `list_skills() -> string`:
  - Returns the same `<available_skills>` XML/markdown as the system-prompt summary.
  - Useful when the model has already loaded one skill and wants to scan the full list freshly without re-seeing the system prompt.
- [ ] 9.3. `load_skill_reference(name: string, file: string) -> string`:
  - For skills with auxiliary files (e.g., a `SKILL.md` referencing `examples.md` in the same directory), loads that sibling file.
  - Path-safety: the file must be inside the skill's directory; reject traversal.
- [ ] 9.4. Register these in M20's `ToolRegistry`.
- [ ] 9.5. Commit: `feat(ai-tools): skill loading tools`.

### 10. System prompt integration with M23

- [ ] 10.1. When M23's agent loop builds a `ChatRequest`, prepend to the `system` prompt:
  ```
  [existing system prompt]

  <available_skills>
    <skill name="..."  description="..." />
    ...
  </available_skills>

  If a skill looks relevant to a user request, call load_skill(name) to fetch its instructions before acting. Always call system-info at the start of any task that depends on the environment.
  ```
- [ ] 10.2. This adds ~400-800 tokens to every turn. Acceptable — skill content adds many times that much value when used.
- [ ] 10.3. Commit: `feat(chat): inject skills into system prompt`.

### 11. User-authored skills: per-workspace

- [ ] 11.1. On workspace open, scan `<workspace>/.ide/skills/*/SKILL.md`. Register each.
- [ ] 11.2. File watcher (from M13): on any change to `.ide/skills/**`, reload the registry. Agent's next turn picks up changes.
- [ ] 11.3. Commit: `feat(skills): workspace-local skill discovery + hot reload`.

### 12. User-authored skills: per-user

- [ ] 12.1. Scan `~/.config/ide/skills/` / platform-equivalent on startup.
- [ ] 12.2. No watcher — rescan requires a restart or a settings action. (User-global skills change rarely.)
- [ ] 12.3. Commit: `feat(skills): per-user skill discovery`.

### 13. Settings hooks (stubs for M28)

- [ ] 13.1. `SkillRegistry` exposes `list()` + `set_enabled(name, bool)` for the settings panel.
- [ ] 13.2. Additional skill dirs can be registered via config; persisted in `PersistedState.extra_skill_dirs: Vec<PathBuf>`.
- [ ] 13.3. Commit: `feat(skills): settings-panel API surface`.

### 14. Documentation

- [ ] 14.1. `/docs/SKILLS.md` — author-facing guide. "How to write a skill," "What makes a good description" (pushy!), "Examples."
- [ ] 14.2. `/docs/BUILTIN_SKILLS.md` — reference for all 7 built-in skills.
- [ ] 14.3. Commit: `docs: skills authoring and built-in reference`.

### 15. Tests + benchmarks

- [ ] 15.1. Unit tests: parsing valid/invalid SKILL.md.
- [ ] 15.2. Integration: discover built-in + workspace skills, verify precedence works.
- [ ] 15.3. Dynamic skill: assert `system-info` returns current OS, shell, etc.
- [ ] 15.4. Bench: `summary_for_system_prompt` at 50 skills: < 500 μs.
- [ ] 15.5. Commit: `test(skills): unit + integration + bench`.

### 16. Quality gates + tag

- [ ] 16.1. Standard gates.
- [ ] 16.2. Manual test: submit a prompt through M23's chat panel, observe the model calling `load_skill`.
- [ ] 16.3. Tag: `git tag -a m27-complete -m "M27 complete: AI skills system"`. Push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. All 7 built-in skills are bundled in the release binary.
3. `SkillRegistry::summary_for_system_prompt()` produces well-formed output.
4. `load_skill` tool returns skill bodies; `system-info` returns live runtime info.
5. Workspace-local skills load correctly and hot-reload on file change.
6. Cross-provider: the system-prompt injection format works with OpenAI, Anthropic, Ollama (manually verify with at least two).
7. `m27-complete` tag pushed.

## Testing Requirements

- Unit tests for parsing + discovery + precedence.
- Integration test with a live chat panel + Ollama.
- Manual verification with OpenAI in the actual chat panel.

## Git Commit Strategy

12-14 commits. Push after items 3, 7, 9, 11, 14, 16.

## Handoff to M20

M20 now picks up the three skill-loading tools. M20 also should reference `ide-conventions` in its own docs as the canonical description of the tool + transaction model.

## Handoff to M23

M23's system prompt is now enriched with the skill list every turn. This is the key multiplier.

---

## Standing Orders Reminder

- Descriptions must be pushy. Models under-trigger. "Use this whenever X" > "About X."
- Progressive disclosure is the whole point. Never inline a skill's body into the system prompt.
- Built-in skills ship in the binary via `include_str!`. Never require network fetch at runtime.
- The skill format is Anthropic-compatible so users can copy existing skills from `github.com/anthropics/skills` and they just work.
- User skills are trusted inputs. Do not execute arbitrary code they might embed. The current design only loads markdown content — keep it that way.

Go.
