//! Extensible language metadata used by the editor for editing behavior and highlighting.
//!
//! This registry is intentionally data-driven so adding another language does
//! not require changing editor command logic.

use std::path::Path;

use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Preferred highlight backend for a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightBackend {
    /// Use a specific tree-sitter backend.
    TreeSitter(TreeSitterBackend),
    /// Use syntect syntax highlighting.
    Syntect,
    /// Do not syntax-highlight; render as plain text.
    PlainText,
}

/// Supported tree-sitter-backed languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSitterBackend {
    /// LilyPond grammar with embedded Scheme support.
    LilyPond,
    /// Standalone Scheme grammar.
    Scheme,
    /// TOML grammar.
    Toml,
    /// Makefile grammar.
    Makefile,
}

/// Editing-related metadata for one language family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageConfig {
    /// Canonical syntax identifier used by the editor.
    pub id: &'static str,
    /// Accepted syntax identifiers and aliases that map to this config.
    pub syntax_ids: &'static [&'static str],
    /// Known file extensions that should bootstrap to this config.
    pub extensions: &'static [&'static str],
    /// Exact filenames that should bootstrap to this config.
    pub filenames: &'static [&'static str],
    /// Optional line comment token.
    pub line_comment: Option<&'static str>,
    /// Optional block comment delimiters.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// Bracket pairs used for matching-bracket navigation/highlighting.
    pub bracket_pairs: &'static [(char, char)],
    /// Auto-closing and surrounding pairs used by smart editing.
    pub auto_closing_pairs: &'static [(char, char)],
    /// Preferred highlighting backend.
    pub highlight_backend: HighlightBackend,
    /// Syntect extension candidates, in fallback order.
    pub syntect_extensions: &'static [&'static str],
    /// Syntect syntax-name candidates, in fallback order.
    pub syntect_names: &'static [&'static str],
}

const DEFAULT_BRACKETS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];
const DEFAULT_AUTO_CLOSING_PAIRS: &[(char, char)] =
    &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')];

const TEXT: LanguageConfig = LanguageConfig {
    id: "text",
    syntax_ids: &["text", "txt"],
    extensions: &["txt"],
    filenames: &[],
    line_comment: None,
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::PlainText,
    syntect_extensions: &[],
    syntect_names: &[],
};

const MARKDOWN: LanguageConfig = LanguageConfig {
    id: "markdown",
    syntax_ids: &["markdown", "md"],
    extensions: &["md", "markdown"],
    filenames: &[],
    line_comment: None,
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &["md"],
    syntect_names: &["Markdown"],
};

const LILYPOND: LanguageConfig = LanguageConfig {
    id: "lilypond",
    syntax_ids: &["lilypond", "ly", "ily"],
    extensions: &["ly", "ily"],
    filenames: &[],
    line_comment: Some("%"),
    block_comment: Some(("%{", "%}")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::TreeSitter(TreeSitterBackend::LilyPond),
    syntect_extensions: &[],
    syntect_names: &[],
};

const SCHEME: LanguageConfig = LanguageConfig {
    id: "scheme",
    syntax_ids: &["scheme", "scm", "lilypond_scheme"],
    extensions: &["scm"],
    filenames: &[],
    line_comment: Some(";"),
    block_comment: Some(("#|", "|#")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::TreeSitter(TreeSitterBackend::Scheme),
    syntect_extensions: &["scm", "scheme"],
    syntect_names: &["Scheme"],
};

const TOML: LanguageConfig = LanguageConfig {
    id: "toml",
    syntax_ids: &["toml"],
    extensions: &["toml"],
    filenames: &[],
    line_comment: Some("#"),
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::TreeSitter(TreeSitterBackend::Toml),
    syntect_extensions: &["toml"],
    syntect_names: &["TOML"],
};

const RON: LanguageConfig = LanguageConfig {
    id: "ron",
    syntax_ids: &["ron"],
    extensions: &["ron"],
    filenames: &[],
    line_comment: Some("//"),
    block_comment: Some(("/*", "*/")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &["ron", "rs"],
    syntect_names: &["Rust"],
};

const MAKEFILE: LanguageConfig = LanguageConfig {
    id: "makefile",
    syntax_ids: &["makefile", "make", "mk"],
    extensions: &["mk"],
    filenames: &["Makefile", "makefile", "GNUmakefile"],
    line_comment: Some("#"),
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::TreeSitter(TreeSitterBackend::Makefile),
    syntect_extensions: &["make", "mk"],
    syntect_names: &["Makefile", "Make"],
};

const C_STYLE: LanguageConfig = LanguageConfig {
    id: "c-style",
    syntax_ids: &[
        "rs",
        "rust",
        "js",
        "javascript",
        "jsx",
        "ts",
        "typescript",
        "tsx",
        "c",
        "cc",
        "cpp",
        "cxx",
        "h",
        "hpp",
        "java",
        "go",
        "swift",
        "kt",
        "kts",
        "css",
        "scss",
    ],
    extensions: &[
        "rs", "js", "jsx", "ts", "tsx", "c", "cc", "cpp", "cxx", "h", "hpp", "java", "go", "swift",
        "kt", "kts", "css", "scss",
    ],
    filenames: &[],
    line_comment: Some("//"),
    block_comment: Some(("/*", "*/")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &[],
    syntect_names: &[],
};

const HASH_STYLE: LanguageConfig = LanguageConfig {
    id: "hash-style",
    syntax_ids: &[
        "py", "python", "sh", "bash", "zsh", "rb", "ruby", "yaml", "yml", "ini", "conf", "pl",
    ],
    extensions: &[
        "py", "sh", "bash", "zsh", "rb", "yaml", "yml", "ini", "conf", "pl",
    ],
    filenames: &[],
    line_comment: Some("#"),
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &[],
    syntect_names: &[],
};

const LUA: LanguageConfig = LanguageConfig {
    id: "lua",
    syntax_ids: &["lua"],
    extensions: &["lua"],
    filenames: &[],
    line_comment: Some("--"),
    block_comment: Some(("--[[", "]]")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &["lua"],
    syntect_names: &["Lua"],
};

const SQL: LanguageConfig = LanguageConfig {
    id: "sql",
    syntax_ids: &["sql"],
    extensions: &["sql"],
    filenames: &[],
    line_comment: Some("--"),
    block_comment: Some(("/*", "*/")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &["sql"],
    syntect_names: &["SQL"],
};

const HTML: LanguageConfig = LanguageConfig {
    id: "html",
    syntax_ids: &["html", "htm", "xml", "svg"],
    extensions: &["html", "htm", "xml", "svg"],
    filenames: &[],
    line_comment: None,
    block_comment: Some(("<!--", "-->")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
    highlight_backend: HighlightBackend::Syntect,
    syntect_extensions: &["html", "xml"],
    syntect_names: &["HTML", "XML"],
};

const LANGUAGE_CONFIGS: &[LanguageConfig] = &[
    LILYPOND, SCHEME, TOML, RON, MAKEFILE, MARKDOWN, C_STYLE, HASH_STYLE, LUA, SQL, HTML, TEXT,
];

/// Returns the language config for a syntax identifier, falling back to text.
pub fn config_for_syntax(syntax: &str) -> &'static LanguageConfig {
    LANGUAGE_CONFIGS
        .iter()
        .find(|config| config.syntax_ids.contains(&syntax))
        .unwrap_or(&TEXT)
}

/// Returns the preferred syntax identifier for a file extension, if known.
pub fn syntax_for_extension(extension: &str) -> Option<&'static str> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    LANGUAGE_CONFIGS
        .iter()
        .find(|config| config.extensions.contains(&extension.as_str()))
        .map(|config| {
            if let Some(syntax_id) = config
                .syntax_ids
                .iter()
                .copied()
                .find(|syntax_id| *syntax_id == extension.as_str())
            {
                syntax_id
            } else {
                config.id
            }
        })
}

/// Returns the preferred syntax identifier for a full path, if known.
pub fn syntax_for_path(path: &Path) -> Option<&'static str> {
    let file_name = path.file_name().and_then(|name| name.to_str())?;

    if let Some(config) = LANGUAGE_CONFIGS.iter().find(|config| {
        config
            .filenames
            .iter()
            .any(|name| name.eq_ignore_ascii_case(file_name))
    }) {
        return Some(config.id);
    }

    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(syntax_for_extension)
}

/// Returns the language config for a full path, falling back to text.
pub fn config_for_path(path: &Path) -> &'static LanguageConfig {
    syntax_for_path(path)
        .map(config_for_syntax)
        .unwrap_or(&TEXT)
}

pub(crate) fn find_syntect_syntax<'a>(
    syntax: &str,
    syntax_set: &'a SyntaxSet,
) -> Option<&'a SyntaxReference> {
    let config = config_for_syntax(syntax);

    match config.highlight_backend {
        HighlightBackend::PlainText => return Some(syntax_set.find_syntax_plain_text()),
        HighlightBackend::TreeSitter(_) | HighlightBackend::Syntect => {}
    }

    for extension in std::iter::once(syntax).chain(config.syntect_extensions.iter().copied()) {
        if let Some(syntax_ref) = syntax_set.find_syntax_by_extension(extension) {
            return Some(syntax_ref);
        }
    }

    for name in config.syntect_names {
        if let Some(syntax_ref) = syntax_set.find_syntax_by_name(name) {
            return Some(syntax_ref);
        }
    }

    if config.id == "text" {
        Some(syntax_set.find_syntax_plain_text())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lilypond_config_has_expected_comments() {
        let config = config_for_syntax("lilypond");
        assert_eq!(config.line_comment, Some("%"));
        assert_eq!(config.block_comment, Some(("%{", "%}")));
    }

    #[test]
    fn path_bootstrap_resolves_known_languages() {
        assert_eq!(syntax_for_path(Path::new("score.ly")), Some("ly"));
        assert_eq!(syntax_for_path(Path::new("Cargo.toml")), Some("toml"));
        assert_eq!(syntax_for_path(Path::new("project.ron")), Some("ron"));
        assert_eq!(syntax_for_path(Path::new("README.md")), Some("md"));
        assert_eq!(syntax_for_path(Path::new("Makefile")), Some("makefile"));
        assert_eq!(syntax_for_path(Path::new("GNUmakefile")), Some("makefile"));
        assert_eq!(syntax_for_path(Path::new("build.mk")), Some("mk"));
        assert_eq!(syntax_for_path(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn markdown_has_no_comment_toggle() {
        let config = config_for_syntax("markdown");
        assert_eq!(config.line_comment, None);
        assert_eq!(config.block_comment, None);
    }

    #[test]
    fn ron_uses_rust_like_syntect_fallback() {
        let config = config_for_syntax("ron");
        assert_eq!(config.syntect_extensions, ["ron", "rs"]);
        assert_eq!(config.syntect_names, ["Rust"]);
    }
}
