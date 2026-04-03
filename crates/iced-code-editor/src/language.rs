//! Extensible language metadata used by the editor for editing behavior.
//!
//! This registry is intentionally data-driven so adding another language does
//! not require changing editor command logic.

/// Editing-related metadata for one language family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageConfig {
    /// Canonical syntax identifier used by the editor.
    pub id: &'static str,
    /// Accepted syntax identifiers and aliases that map to this config.
    pub syntax_ids: &'static [&'static str],
    /// Known file extensions that should bootstrap to this config.
    pub extensions: &'static [&'static str],
    /// Optional line comment token.
    pub line_comment: Option<&'static str>,
    /// Optional block comment delimiters.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// Bracket pairs used for matching-bracket navigation/highlighting.
    pub bracket_pairs: &'static [(char, char)],
    /// Auto-closing and surrounding pairs used by smart editing.
    pub auto_closing_pairs: &'static [(char, char)],
}

const DEFAULT_BRACKETS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];
const DEFAULT_AUTO_CLOSING_PAIRS: &[(char, char)] =
    &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')];

const TEXT: LanguageConfig = LanguageConfig {
    id: "text",
    syntax_ids: &["text", "txt", "md", "markdown"],
    extensions: &["txt", "md"],
    line_comment: None,
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const LILYPOND: LanguageConfig = LanguageConfig {
    id: "lilypond",
    syntax_ids: &["lilypond", "ly", "ily"],
    extensions: &["ly", "ily"],
    line_comment: Some("%"),
    block_comment: Some(("%{", "%}")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const SCHEME: LanguageConfig = LanguageConfig {
    id: "scheme",
    syntax_ids: &["scheme", "scm", "lilypond_scheme"],
    extensions: &["scm"],
    line_comment: Some(";"),
    block_comment: Some(("#|", "|#")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
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
    line_comment: Some("//"),
    block_comment: Some(("/*", "*/")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const HASH_STYLE: LanguageConfig = LanguageConfig {
    id: "hash-style",
    syntax_ids: &[
        "py", "python", "sh", "bash", "zsh", "rb", "ruby", "toml", "yaml", "yml", "ini", "conf",
        "make", "mk", "pl",
    ],
    extensions: &[
        "py", "sh", "bash", "zsh", "rb", "toml", "yaml", "yml", "ini", "conf", "make", "mk", "pl",
    ],
    line_comment: Some("#"),
    block_comment: None,
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const LUA: LanguageConfig = LanguageConfig {
    id: "lua",
    syntax_ids: &["lua"],
    extensions: &["lua"],
    line_comment: Some("--"),
    block_comment: Some(("--[[", "]]")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const SQL: LanguageConfig = LanguageConfig {
    id: "sql",
    syntax_ids: &["sql"],
    extensions: &["sql"],
    line_comment: Some("--"),
    block_comment: Some(("/*", "*/")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const HTML: LanguageConfig = LanguageConfig {
    id: "html",
    syntax_ids: &["html", "htm", "xml", "svg"],
    extensions: &["html", "htm", "xml", "svg"],
    line_comment: None,
    block_comment: Some(("<!--", "-->")),
    bracket_pairs: DEFAULT_BRACKETS,
    auto_closing_pairs: DEFAULT_AUTO_CLOSING_PAIRS,
};

const LANGUAGE_CONFIGS: &[LanguageConfig] =
    &[LILYPOND, SCHEME, C_STYLE, HASH_STYLE, LUA, SQL, HTML, TEXT];

/// Returns the language config for a syntax identifier, falling back to text.
pub fn config_for_syntax(syntax: &str) -> &'static LanguageConfig {
    LANGUAGE_CONFIGS
        .iter()
        .find(|config| config.syntax_ids.contains(&syntax))
        .unwrap_or(&TEXT)
}

/// Returns the preferred syntax identifier for a file extension, if known.
pub fn syntax_for_extension(extension: &str) -> Option<&'static str> {
    LANGUAGE_CONFIGS
        .iter()
        .find(|config| config.extensions.contains(&extension))
        .map(|config| match config.id {
            "c-style" | "hash-style" => config
                .syntax_ids
                .iter()
                .copied()
                .find(|syntax| *syntax == extension)
                .unwrap_or(config.id),
            _ => config.id,
        })
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
    fn extension_bootstrap_resolves_known_languages() {
        assert_eq!(syntax_for_extension("ly"), Some("lilypond"));
        assert_eq!(syntax_for_extension("rs"), Some("rs"));
        assert_eq!(syntax_for_extension("unknown"), None);
    }
}
