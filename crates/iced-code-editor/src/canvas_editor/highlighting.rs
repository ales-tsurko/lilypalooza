use iced::Color;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

use crate::theme::Style;

const TREE_SITTER_HIGHLIGHT_NAMES: &[&str] = &[
    "comment",
    "string",
    "string.delimiter.left",
    "string.delimiter.right",
    "escape",
    "keyword",
    "number",
    "value.number",
    "function",
    "identifier.core.function",
    "variable",
    "identifier",
    "identifier.variable",
    "operator",
    "processing",
    "global",
    "identifier.core.global",
    "constant",
    "boolean",
    "value.boolean",
    "entity",
    "value.entity",
    "punctuation",
    "punctuation.bracket",
    "bracket",
    "invalid",
];

#[derive(Debug, Clone)]
pub(crate) struct HighlightedDocument {
    pub(crate) lines: Vec<Vec<StyledSpan>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StyledSpan {
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
    pub(crate) color: Color,
}

pub(crate) fn highlight_document(
    syntax: &str,
    source: &str,
    style: &Style,
) -> Option<HighlightedDocument> {
    let backend = HighlightBackend::for_syntax(syntax)?;
    backend.highlight(source, style).ok()
}

enum HighlightBackend {
    LilyPond,
    Scheme,
}

impl HighlightBackend {
    fn for_syntax(syntax: &str) -> Option<Self> {
        match syntax {
            "ly" | "ily" | "lilypond" => Some(Self::LilyPond),
            "scm" | "scheme" | "lilypond_scheme" => Some(Self::Scheme),
            _ => None,
        }
    }

    fn highlight(&self, source: &str, style: &Style) -> Result<HighlightedDocument, String> {
        match self {
            Self::LilyPond => highlight_lilypond(source, style),
            Self::Scheme => highlight_lilypond_scheme(source, style),
        }
    }
}

fn highlight_lilypond(source: &str, style: &Style) -> Result<HighlightedDocument, String> {
    let lilypond_language = tree_sitter_lilypond::LANGUAGE_LILYPOND.into();
    let scheme_language = tree_sitter_lilypond::LANGUAGE_LILYPOND_SCHEME.into();

    let mut lilypond_config = HighlightConfiguration::new(
        lilypond_language,
        "lilypond",
        tree_sitter_lilypond::HIGHLIGHTS_QUERY,
        tree_sitter_lilypond::INJECTIONS_QUERY,
        "",
    )
    .map_err(|error| format!("Tree-sitter LilyPond config error: {error}"))?;
    lilypond_config.configure(TREE_SITTER_HIGHLIGHT_NAMES);

    let mut scheme_config = HighlightConfiguration::new(
        scheme_language,
        "lilypond_scheme",
        tree_sitter_lilypond::HIGHLIGHTS_SCHEME_QUERY,
        "",
        "",
    )
    .map_err(|error| format!("Tree-sitter LilyPond Scheme config error: {error}"))?;
    scheme_config.configure(TREE_SITTER_HIGHLIGHT_NAMES);

    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(&lilypond_config, source.as_bytes(), None, |language_name| {
            if language_name == "lilypond_scheme" {
                Some(&scheme_config)
            } else {
                None
            }
        })
        .map_err(|error| format!("Tree-sitter highlight error: {error}"))?;

    let line_infos = line_infos(source);
    let mut styled_chars = line_infos
        .iter()
        .map(|info| vec![None; info.text.chars().count()])
        .collect::<Vec<_>>();
    let mut highlight_stack = Vec::new();

    for event in events {
        match event.map_err(|error| format!("Tree-sitter highlight event error: {error}"))? {
            HighlightEvent::HighlightStart(highlight) => highlight_stack.push(highlight),
            HighlightEvent::HighlightEnd => {
                let _ = highlight_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let Some(highlight) = highlight_stack.last().copied() else {
                    continue;
                };
                let color = highlight_color(highlight, style);
                apply_highlight_range(&line_infos, &mut styled_chars, start, end, color);
            }
        }
    }

    Ok(HighlightedDocument {
        lines: styled_chars
            .into_iter()
            .map(compress_styled_chars)
            .collect(),
    })
}

fn highlight_lilypond_scheme(source: &str, style: &Style) -> Result<HighlightedDocument, String> {
    let scheme_language = tree_sitter_lilypond::LANGUAGE_LILYPOND_SCHEME.into();

    let mut scheme_config = HighlightConfiguration::new(
        scheme_language,
        "lilypond_scheme",
        tree_sitter_lilypond::HIGHLIGHTS_SCHEME_QUERY,
        "",
        "",
    )
    .map_err(|error| format!("Tree-sitter LilyPond Scheme config error: {error}"))?;
    scheme_config.configure(TREE_SITTER_HIGHLIGHT_NAMES);

    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(&scheme_config, source.as_bytes(), None, |_| None)
        .map_err(|error| format!("Tree-sitter highlight error: {error}"))?;

    let line_infos = line_infos(source);
    let mut styled_chars = line_infos
        .iter()
        .map(|info| vec![None; info.text.chars().count()])
        .collect::<Vec<_>>();
    let mut highlight_stack = Vec::new();

    for event in events {
        match event.map_err(|error| format!("Tree-sitter highlight event error: {error}"))? {
            HighlightEvent::HighlightStart(highlight) => highlight_stack.push(highlight),
            HighlightEvent::HighlightEnd => {
                let _ = highlight_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let Some(highlight) = highlight_stack.last().copied() else {
                    continue;
                };
                let color = highlight_color(highlight, style);
                apply_highlight_range(&line_infos, &mut styled_chars, start, end, color);
            }
        }
    }

    Ok(HighlightedDocument {
        lines: styled_chars
            .into_iter()
            .map(compress_styled_chars)
            .collect(),
    })
}

#[derive(Debug)]
struct LineInfo<'a> {
    start_byte: usize,
    text: &'a str,
}

fn line_infos(source: &str) -> Vec<LineInfo<'_>> {
    let mut infos = Vec::new();
    let mut start_byte = 0usize;

    for line in source.split('\n') {
        infos.push(LineInfo {
            start_byte,
            text: line,
        });
        start_byte = start_byte.saturating_add(line.len()).saturating_add(1);
    }

    if infos.is_empty() {
        infos.push(LineInfo {
            start_byte: 0,
            text: "",
        });
    }

    infos
}

fn apply_highlight_range(
    lines: &[LineInfo<'_>],
    styled_chars: &mut [Vec<Option<Color>>],
    start: usize,
    end: usize,
    color: Color,
) {
    if start >= end || lines.is_empty() {
        return;
    }

    let mut line_index = line_index_for_byte(lines, start);

    while let Some(line) = lines.get(line_index) {
        let line_start = line.start_byte;
        let line_end = line_start + line.text.len();

        if end <= line_start {
            break;
        }

        let segment_start = start.max(line_start);
        let segment_end = end.min(line_end);

        if segment_start < segment_end {
            let relative_start = segment_start - line_start;
            let relative_end = segment_end - line_start;
            let start_col = char_count_at_byte(line.text, relative_start);
            let end_col = char_count_at_byte(line.text, relative_end);

            if let Some(line_styles) = styled_chars.get_mut(line_index) {
                for cell in line_styles.iter_mut().take(end_col).skip(start_col) {
                    *cell = Some(color);
                }
            }
        }

        if end <= line_end {
            break;
        }

        line_index += 1;
    }
}

fn line_index_for_byte(lines: &[LineInfo<'_>], byte: usize) -> usize {
    match lines.binary_search_by_key(&byte, |line| line.start_byte) {
        Ok(index) => index,
        Err(0) => 0,
        Err(index) => index - 1,
    }
}

fn char_count_at_byte(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    text[..clamped].chars().count()
}

fn compress_styled_chars(chars: Vec<Option<Color>>) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let mut current_color = None;
    let mut current_start = 0usize;
    let total_len = chars.len();

    for (index, color) in chars.into_iter().enumerate() {
        if color != current_color {
            if let Some(previous_color) = current_color {
                spans.push(StyledSpan {
                    start_col: current_start,
                    end_col: index,
                    color: previous_color,
                });
            }

            current_color = color;
            current_start = index;
        }
    }

    if let Some(color) = current_color {
        spans.push(StyledSpan {
            start_col: current_start,
            end_col: total_len,
            color,
        });
    }

    spans
}

fn highlight_color(highlight: Highlight, style: &Style) -> Color {
    let name = TREE_SITTER_HIGHLIGHT_NAMES
        .get(highlight.0)
        .copied()
        .unwrap_or("text");

    match name {
        "comment" => style.comment_color,
        "string" => style.string_color,
        "string.delimiter.left" | "string.delimiter.right" => style.string_delimiter_color,
        "escape" => style.escape_color,
        "keyword" => style.keyword_color,
        "number" | "value.number" => style.number_color,
        "function" | "identifier.core.function" => style.function_color,
        "identifier.variable" => style.command_color,
        "variable" | "identifier" => style.variable_color,
        "operator" => style.operator_color,
        "processing" => style.processing_color,
        "global" | "identifier.core.global" => style.constant_color,
        "constant" | "boolean" | "value.boolean" | "entity" | "value.entity" => {
            style.constant_color
        }
        "punctuation.bracket" | "bracket" => style.bracket_color,
        "punctuation" => style.punctuation_color,
        "invalid" => style.invalid_color,
        _ => style.text_color,
    }
}
