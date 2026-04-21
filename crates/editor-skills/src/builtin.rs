//! Built-in skills embedded with `include_str!`.

pub const BUILTIN_USING_TERMINAL: &str = include_str!("../assets/builtin/using-terminal/SKILL.md");
pub const BUILTIN_USING_GIT: &str = include_str!("../assets/builtin/using-git/SKILL.md");
pub const BUILTIN_IDE_CONVENTIONS: &str =
    include_str!("../assets/builtin/ide-conventions/SKILL.md");
pub const BUILTIN_WRITING_RUST: &str = include_str!("../assets/builtin/writing-rust/SKILL.md");
pub const BUILTIN_WRITING_PYTHON: &str = include_str!("../assets/builtin/writing-python/SKILL.md");
pub const BUILTIN_WRITING_TYPESCRIPT: &str =
    include_str!("../assets/builtin/writing-typescript/SKILL.md");

pub static BUILTIN_SKILL_TEXTS: &[(&str, &str)] = &[
    ("using-terminal", BUILTIN_USING_TERMINAL),
    ("using-git", BUILTIN_USING_GIT),
    ("ide-conventions", BUILTIN_IDE_CONVENTIONS),
    ("writing-rust", BUILTIN_WRITING_RUST),
    ("writing-python", BUILTIN_WRITING_PYTHON),
    ("writing-typescript", BUILTIN_WRITING_TYPESCRIPT),
];
