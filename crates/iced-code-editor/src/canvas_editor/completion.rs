use std::collections::HashSet;
use std::fs;
use std::path::Path;

use iced::widget::operation::scroll_to;
use iced::widget::{Space, button, column, container, row, scrollable, stack, text};
use iced::{Alignment, Background, Color, Element, Length, Point, Theme};

use super::{CodeEditor, Message};

const MAX_VISIBLE_ITEMS: usize = 8;
const ITEM_HEIGHT: f32 = 22.0;
const MENU_WIDTH: f32 = 300.0;
const MENU_PADDING: f32 = 4.0;

const LILYPOND_COMMANDS: &[&str] = &[
    "\\absolute",
    "\\acciaccatura",
    "\\addlyrics",
    "\\afterGrace",
    "\\alternative",
    "\\appoggiatura",
    "\\autoBeamOff",
    "\\autoBeamOn",
    "\\bar",
    "\\book",
    "\\bookpart",
    "\\break",
    "\\chordmode",
    "\\clef",
    "\\context",
    "\\cueDuring",
    "\\default",
    "\\drummode",
    "\\dynamicDown",
    "\\dynamicUp",
    "\\figuremode",
    "\\glissando",
    "\\grace",
    "\\header",
    "\\include",
    "\\key",
    "\\language",
    "\\layout",
    "\\lyricmode",
    "\\lyrics",
    "\\major",
    "\\mark",
    "\\markup",
    "\\markuplist",
    "\\midi",
    "\\minor",
    "\\new",
    "\\noBreak",
    "\\numericTimeSignature",
    "\\oneVoice",
    "\\ottava",
    "\\override",
    "\\pageBreak",
    "\\paper",
    "\\parallelMusic",
    "\\partCombine",
    "\\phrasingSlurDown",
    "\\phrasingSlurUp",
    "\\relative",
    "\\remove",
    "\\repeat",
    "\\rest",
    "\\revert",
    "\\score",
    "\\section",
    "\\set",
    "\\simultaneous",
    "\\slurDown",
    "\\slurUp",
    "\\stemDown",
    "\\stemNeutral",
    "\\stemUp",
    "\\tempo",
    "\\time",
    "\\transpose",
    "\\tuplet",
    "\\tweak",
    "\\unset",
    "\\version",
    "\\voiceFour",
    "\\voiceOne",
    "\\voiceThree",
    "\\voiceTwo",
];

const LILYPOND_MUSIC_FUNCTIONS: &[&str] = &[
    "\\absolute",
    "\\acciaccatura",
    "\\afterGrace",
    "\\appoggiatura",
    "\\cueDuring",
    "\\grace",
    "\\key",
    "\\language",
    "\\numericTimeSignature",
    "\\ottava",
    "\\parallelMusic",
    "\\partCombine",
    "\\relative",
    "\\repeat",
    "\\tempo",
    "\\time",
    "\\transpose",
    "\\tuplet",
];

const LILYPOND_MUSIC_OBJECTS: &[&str] = &[
    "\\accent",
    "\\arpeggio",
    "\\breathe",
    "\\fermata",
    "\\ff",
    "\\fff",
    "\\ffff",
    "\\f",
    "\\fp",
    "\\glissando",
    "\\marcato",
    "\\mf",
    "\\mp",
    "\\p",
    "\\pp",
    "\\ppp",
    "\\pppp",
    "\\prall",
    "\\repeatTie",
    "\\sf",
    "\\sff",
    "\\sfz",
    "\\slurDotted",
    "\\staccato",
    "\\staccatissimo",
    "\\tenuto",
    "\\trill",
    "\\turn",
];

const LILYPOND_MARKUP_COMMANDS: &[&str] = &[
    "\\bold",
    "\\box",
    "\\caps",
    "\\center-align",
    "\\column",
    "\\concat",
    "\\dynamic",
    "\\fill-line",
    "\\fontsize",
    "\\hspace",
    "\\huge",
    "\\italic",
    "\\justify",
    "\\justify-line",
    "\\large",
    "\\line",
    "\\markup",
    "\\normal-size-sub",
    "\\normal-size-super",
    "\\number",
    "\\overlay",
    "\\pad-markup",
    "\\raise",
    "\\roman",
    "\\rotate",
    "\\sans",
    "\\small",
    "\\smaller",
    "\\tiny",
    "\\typewriter",
    "\\underline",
    "\\vspace",
    "\\wordwrap",
];

const LILYPOND_CONTEXTS: &[&str] = &[
    "ChoirStaff",
    "ChordNames",
    "CueVoice",
    "Devnull",
    "DrumStaff",
    "FiguredBass",
    "FretBoards",
    "Global",
    "GrandStaff",
    "GregorianTranscriptionStaff",
    "Lyrics",
    "MensuralStaff",
    "NoteNames",
    "NullVoice",
    "PianoStaff",
    "RhythmicStaff",
    "Score",
    "Staff",
    "StaffGroup",
    "TabStaff",
    "VaticanaStaff",
    "Voice",
];

const LILYPOND_TRANSLATORS: &[&str] = &[
    "Accidental_engraver",
    "Axis_group_engraver",
    "Bar_engraver",
    "Beam_engraver",
    "Clef_engraver",
    "Collision_engraver",
    "Cue_clef_engraver",
    "Custos_engraver",
    "Dynamic_engraver",
    "Figured_bass_engraver",
    "Fingering_engraver",
    "Key_engraver",
    "Ledger_line_spanner",
    "Metronome_mark_engraver",
    "New_fingering_engraver",
    "Note_heads_engraver",
    "Ottava_spanner_engraver",
    "Piano_pedal_engraver",
    "Rest_engraver",
    "Script_engraver",
    "Slur_engraver",
    "Staff_symbol_engraver",
    "Stem_engraver",
    "System_start_delimiter_engraver",
    "Text_engraver",
    "Text_spanner_engraver",
    "Tie_engraver",
    "Time_signature_engraver",
];

const LILYPOND_GROBS: &[&str] = &[
    "Accidental",
    "Arpeggio",
    "BarLine",
    "Beam",
    "Clef",
    "DynamicText",
    "Fingering",
    "Glissando",
    "Hairpin",
    "KeySignature",
    "LedgerLineSpanner",
    "LyricText",
    "NoteHead",
    "OttavaBracket",
    "PianoPedalBracket",
    "RepeatTie",
    "Rest",
    "Script",
    "Slur",
    "StaffSymbol",
    "Stem",
    "StemTremolo",
    "SystemStartBar",
    "SystemStartBrace",
    "SystemStartBracket",
    "TextScript",
    "TextSpanner",
    "Tie",
    "TimeSignature",
    "TrillSpanner",
    "TupletBracket",
    "TupletNumber",
];

const LILYPOND_GROB_PROPERTIES: &[&str] = &[
    "X-extent",
    "Y-extent",
    "X-offset",
    "Y-offset",
    "avoid-slur",
    "color",
    "direction",
    "extra-offset",
    "font-family",
    "font-name",
    "font-series",
    "font-shape",
    "font-size",
    "layer",
    "length",
    "minimum-length",
    "outside-staff-priority",
    "padding",
    "positions",
    "rotation",
    "self-alignment-X",
    "self-alignment-Y",
    "shorten-pair",
    "stencil",
    "style",
    "staff-padding",
    "text",
    "thickness",
    "transparent",
];

const LILYPOND_CONTEXT_PROPERTIES: &[&str] = &[
    "autoBeaming",
    "baseMoment",
    "beamExceptions",
    "beatStructure",
    "clefPosition",
    "createKeyOnClefChange",
    "instrumentName",
    "majorSevenSymbol",
    "markFormatter",
    "midiInstrument",
    "ottavation",
    "printKeyCancellation",
    "proportionalNotationDuration",
    "shortInstrumentName",
    "subdivideBeams",
    "suggestAccidentals",
    "tempoWholesPerMinute",
    "timeSignatureFraction",
];

const LILYPOND_CLEFS: &[&str] = &[
    "treble",
    "treble_8",
    "treble^8",
    "bass",
    "bass_8",
    "alto",
    "tenor",
    "soprano",
    "mezzosoprano",
    "baritone",
    "percussion",
    "tab",
];

const LILYPOND_SCALES: &[&str] = &[
    "\\major",
    "\\minor",
    "\\dorian",
    "\\mixolydian",
    "\\lydian",
    "\\phrygian",
    "\\aeolian",
    "\\locrian",
    "\\ionian",
];

const LILYPOND_REPEAT_TYPES: &[&str] = &["percent", "segno", "tremolo", "unfold", "volta"];
const LILYPOND_UNITS: &[&str] = &["\\cm", "\\in", "\\mm", "\\pt"];
const LILYPOND_PAPER_VARIABLES: &[&str] = &[
    "annotate-spacing",
    "bottom-margin",
    "evenFooterMarkup",
    "evenHeaderMarkup",
    "first-page-number",
    "indent",
    "left-margin",
    "line-width",
    "oddFooterMarkup",
    "oddHeaderMarkup",
    "paper-height",
    "paper-width",
    "print-page-number",
    "ragged-bottom",
    "ragged-last",
    "ragged-last-bottom",
    "ragged-right",
    "right-margin",
    "score-markup",
    "short-indent",
    "system-separator-markup",
    "system-system-spacing",
    "top-margin",
    "two-sided",
];
const LILYPOND_HEADER_VARIABLES: &[&str] = &[
    "arranger",
    "composer",
    "copyright",
    "dedication",
    "instrument",
    "meter",
    "opus",
    "piece",
    "poet",
    "subsubtitle",
    "subtitle",
    "tagline",
    "title",
];
const LILYPOND_PITCH_LANGUAGES: &[&str] = &[
    "catalan",
    "deutsch",
    "english",
    "espanol",
    "français",
    "italiano",
    "nederlands",
    "norsk",
    "portugues",
    "suomi",
    "svenska",
    "vlaams",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Command,
    Function,
    Global,
    Markup,
    Context,
    Translator,
    Grob,
    Property,
    Variable,
    Constant,
    FilePath,
    Word,
}

impl CompletionKind {
    fn icon(self) -> &'static str {
        match self {
            Self::Command => "⌘",
            Self::Function => "ƒ",
            Self::Global => "◈",
            Self::Markup => "¶",
            Self::Context => "⊞",
            Self::Translator => "⚙",
            Self::Grob => "⬢",
            Self::Property => "≡",
            Self::Variable => "𝑥",
            Self::Constant => "◉",
            Self::FilePath => "↗",
            Self::Word => "𝚃",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub insert_text: String,
    pub kind: CompletionKind,
    rank_group: u8,
}

impl CompletionItem {
    fn new(
        label: impl Into<String>,
        insert_text: impl Into<String>,
        kind: CompletionKind,
        rank_group: u8,
    ) -> Self {
        Self {
            label: label.into(),
            insert_text: insert_text.into(),
            kind,
            rank_group,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub visible: bool,
    pub anchor: Option<Point>,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub prefix: String,
    pub replace_start: Option<(usize, usize)>,
    pub replace_end: Option<(usize, usize)>,
    pub suppressed_start: Option<(usize, usize)>,
}

impl CompletionState {
    pub fn set(
        &mut self,
        items: Vec<CompletionItem>,
        anchor: Point,
        prefix: String,
        replace_start: (usize, usize),
        replace_end: (usize, usize),
    ) {
        self.visible = !items.is_empty();
        self.anchor = Some(anchor);
        self.items = items;
        self.selected = 0;
        self.prefix = prefix;
        self.replace_start = Some(replace_start);
        self.replace_end = Some(replace_end);
    }

    pub fn close(&mut self, suppress: bool) {
        if suppress {
            self.suppressed_start = self.replace_start;
        }
        self.visible = false;
        self.anchor = None;
        self.items.clear();
        self.selected = 0;
        self.prefix.clear();
        self.replace_start = None;
        self.replace_end = None;
    }

    pub fn navigate(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(len) as usize;
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }

    pub fn scroll_offset_for_selected(&self) -> f32 {
        self.selected as f32 * ITEM_HEIGHT
    }
}

pub(super) fn view<'a>(state: &'a CompletionState, editor: &'a CodeEditor) -> Element<'a, Message> {
    if !state.visible || state.items.is_empty() {
        return container(Space::new().width(Length::Shrink).height(Length::Shrink)).into();
    }

    let Some(anchor) = state.anchor else {
        return container(Space::new().width(Length::Shrink).height(Length::Shrink)).into();
    };

    let viewport_width = editor.viewport_width();
    let viewport_height = editor.viewport_height();
    let offset_x = (anchor.x - editor.horizontal_scroll_offset + 2.0)
        .clamp(4.0, (viewport_width - MENU_WIDTH - 4.0).max(4.0));
    let adjusted_y = (anchor.y - editor.viewport_scroll()).max(0.0);
    let visible_count = state.items.len().min(MAX_VISIBLE_ITEMS);
    let menu_height = (visible_count as f32 * ITEM_HEIGHT) + (MENU_PADDING * 2.0);
    let offset_y = if adjusted_y + editor.line_height + menu_height + 6.0 <= viewport_height {
        adjusted_y + editor.line_height + 4.0
    } else {
        (adjusted_y - menu_height - 4.0).max(0.0)
    };

    let items: Vec<Element<'_, Message>> = state
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let selected = index == state.selected;
            button(
                row![
                    text(&item.label).size(12).width(Length::Fill),
                    text(item.kind.icon()).size(12).width(Length::Fixed(14.0)),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
            )
            .width(Length::Fill)
            .padding([4, 8])
            .on_press(Message::CompletionSelected(index))
            .style(move |theme: &Theme, status| {
                let palette = theme.extended_palette();
                let surface = crate::theme::popup_surface(theme);
                let hover = matches!(status, button::Status::Hovered);
                let text_color = if hover || selected {
                    palette.background.weakest.text
                } else {
                    surface.text_color.unwrap_or(palette.background.base.text)
                };
                button::Style {
                    background: if selected || hover {
                        Some(Background::Color(palette.background.strong.color))
                    } else {
                        None
                    },
                    text_color,
                    ..button::Style::default()
                }
            })
            .into()
        })
        .collect();

    let list = scrollable(column(items).spacing(0))
        .id(editor.completion_scrollable_id.clone())
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(3).scroller_width(3),
        ))
        .width(Length::Fixed(MENU_WIDTH))
        .height(Length::Fixed(menu_height))
        .style(|theme, status| {
            let mut style = scrollable::default(theme, status);
            style.container = container::Style::default();
            style.vertical_rail.background = Some(Color::TRANSPARENT.into());
            style.vertical_rail.scroller.background = Color::from_rgba(0.0, 0.0, 0.0, 0.18).into();
            style
        });

    let popup = container(list).padding(MENU_PADDING).style(|theme| {
        let mut style = crate::theme::popup_surface(theme);
        style.border.width = 0.0;
        style.border.color = Color::TRANSPARENT;
        style.border.radius = 0.0.into();
        style.shadow = iced::Shadow::default();
        style
    });

    let positioned = container(
        column![
            Space::new().height(Length::Fixed(offset_y)),
            row![Space::new().width(Length::Fixed(offset_x)), popup],
        ]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    let dismiss = button(
        container(Space::new().width(Length::Fill).height(Length::Fill)).height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_press(Message::CloseCompletion)
    .style(|_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..button::Style::default()
    });

    stack![dismiss, positioned].into()
}

impl CodeEditor {
    pub(super) fn trigger_completion(&mut self, manual: bool) {
        let Some(result) = self.compute_completion_items(manual) else {
            self.completion_state.close(manual);
            self.overlay_cache.clear();
            return;
        };

        if self.completion_state.suppressed_start == Some(result.replace_start) && !manual {
            return;
        }

        self.completion_state.set(
            result.items,
            result.anchor,
            result.prefix,
            result.replace_start,
            result.replace_end,
        );
        self.overlay_cache.clear();
    }

    pub(super) fn close_completion(&mut self, suppress: bool) {
        if self.completion_state.visible || suppress {
            self.completion_state.close(suppress);
            self.overlay_cache.clear();
        }
    }

    pub(super) fn clear_completion_suppression_if_needed(&mut self) {
        let Some(suppressed_start) = self.completion_state.suppressed_start else {
            return;
        };
        let current_start = self.current_completion_token_start();
        if current_start != Some(suppressed_start) {
            self.completion_state.suppressed_start = None;
        }
    }

    pub(super) fn handle_completion_navigation(&mut self, delta: i32) {
        self.completion_state.navigate(delta);
        self.overlay_cache.clear();
    }

    pub(super) fn completion_scroll_task(&self) -> iced::Task<Message> {
        if !self.completion_state.visible {
            return iced::Task::none();
        }

        scroll_to(
            self.completion_scrollable_id.clone(),
            iced::widget::scrollable::AbsoluteOffset {
                x: 0.0,
                y: self.completion_state.scroll_offset_for_selected(),
            },
        )
    }

    pub(super) fn apply_selected_completion(&mut self, index: Option<usize>) -> bool {
        let Some(replace_start) = self.completion_state.replace_start else {
            return false;
        };
        let Some(replace_end) = self.completion_state.replace_end else {
            return false;
        };
        let item = index
            .and_then(|idx| self.completion_state.items.get(idx))
            .or_else(|| self.completion_state.selected_item())
            .cloned();
        let Some(item) = item else {
            return false;
        };

        self.end_grouping_if_active();
        let cursor_after = Self::position_after_text(replace_start, &item.insert_text);
        self.apply_replace_range(replace_start, replace_end, item.insert_text, cursor_after);
        self.finish_edit_operation();
        self.close_completion(true);
        true
    }

    pub(super) fn should_auto_trigger_completion(&self, ch: char) -> bool {
        ch.is_alphanumeric()
            || matches!(ch, '_' | '-' | ':')
            || (self.syntax == "lilypond" && ch == '\\')
            || (self.syntax == "lilypond" && self.is_inside_lilypond_include_string())
    }

    fn current_completion_token_start(&self) -> Option<(usize, usize)> {
        self.detect_completion_context(false)
            .map(|context| context.replace_start)
    }

    fn compute_completion_items(&self, manual: bool) -> Option<CompletionResult> {
        let context = self.detect_completion_context(manual)?;

        let mut items = Vec::new();
        if self.syntax == "lilypond" {
            items.extend(self.lilypond_completion_items(&context));
        }
        items.extend(self.document_word_completion_items(&context));

        let prefix_lower = context.prefix.to_lowercase();
        items.retain(|item| {
            prefix_lower.is_empty() || item.insert_text.to_lowercase().starts_with(&prefix_lower)
        });

        items.sort_by(|left, right| {
            let left_match = completion_match_rank(&left.insert_text, &context.prefix);
            let right_match = completion_match_rank(&right.insert_text, &context.prefix);
            left.rank_group
                .cmp(&right.rank_group)
                .then(left_match.cmp(&right_match))
                .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
        });

        let mut seen = HashSet::new();
        items.retain(|item| seen.insert(item.insert_text.clone()));

        (!items.is_empty()).then_some(CompletionResult {
            anchor: context.anchor,
            prefix: context.prefix,
            replace_start: context.replace_start,
            replace_end: context.replace_end,
            items,
        })
    }

    fn lilypond_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        if context.in_comment {
            return Vec::new();
        }

        if let Some(include_prefix) = &context.include_path_prefix {
            return self.include_path_completion_items(include_prefix, context);
        }

        if context.in_string {
            return Vec::new();
        }

        let token = context.prefix.as_str();
        let mut items = Vec::new();

        match context.kind {
            CompletionContextKind::LilyCommand => {
                items.extend(static_items(LILYPOND_COMMANDS, CompletionKind::Command, 0));
                items.extend(static_items(
                    LILYPOND_MUSIC_FUNCTIONS,
                    CompletionKind::Function,
                    0,
                ));
                items.extend(static_items(
                    LILYPOND_MARKUP_COMMANDS,
                    CompletionKind::Markup,
                    0,
                ));
            }
            CompletionContextKind::LilyContexts => {
                items.extend(static_items(LILYPOND_CONTEXTS, CompletionKind::Context, 0));
            }
            CompletionContextKind::LilyTranslators => {
                items.extend(static_items(
                    LILYPOND_TRANSLATORS,
                    CompletionKind::Translator,
                    0,
                ));
            }
            CompletionContextKind::LilyClefs => {
                items.extend(static_items(LILYPOND_CLEFS, CompletionKind::Constant, 0));
            }
            CompletionContextKind::LilyScales => {
                items.extend(static_items(LILYPOND_SCALES, CompletionKind::Constant, 0));
            }
            CompletionContextKind::LilyRepeatTypes => {
                items.extend(static_items(
                    LILYPOND_REPEAT_TYPES,
                    CompletionKind::Constant,
                    0,
                ));
            }
            CompletionContextKind::LilyLanguages => {
                items.extend(static_items(
                    LILYPOND_PITCH_LANGUAGES,
                    CompletionKind::Constant,
                    0,
                ));
            }
            CompletionContextKind::LilyPaperVariables => {
                items.extend(static_items(
                    LILYPOND_PAPER_VARIABLES,
                    CompletionKind::Variable,
                    0,
                ));
                items.extend(static_items(LILYPOND_UNITS, CompletionKind::Constant, 1));
            }
            CompletionContextKind::LilyHeaderVariables => {
                items.extend(static_items(
                    LILYPOND_HEADER_VARIABLES,
                    CompletionKind::Variable,
                    0,
                ));
            }
            CompletionContextKind::LilyGrobProperties => {
                if let Some((grob_prefix, property_prefix)) = token.split_once('.') {
                    items.extend(
                        LILYPOND_GROB_PROPERTIES
                            .iter()
                            .filter(|property| property.starts_with(property_prefix))
                            .map(|property| {
                                CompletionItem::new(
                                    format!("{grob_prefix}.{property}"),
                                    format!("{grob_prefix}.{property}"),
                                    CompletionKind::Property,
                                    0,
                                )
                            }),
                    );
                } else {
                    items.extend(static_items(
                        LILYPOND_GROB_PROPERTIES,
                        CompletionKind::Property,
                        0,
                    ));
                }
            }
            CompletionContextKind::LilyGrobs => {
                items.extend(static_items(LILYPOND_GROBS, CompletionKind::Grob, 0));
                items.extend(static_items(
                    LILYPOND_CONTEXT_PROPERTIES,
                    CompletionKind::Property,
                    1,
                ));
            }
            CompletionContextKind::GenericWord => {}
        }

        if matches!(context.kind, CompletionContextKind::LilyCommand) && token.starts_with('\\') {
            items.extend(static_items(
                LILYPOND_MUSIC_OBJECTS,
                CompletionKind::Global,
                1,
            ));
        }

        items
    }

    fn document_word_completion_items(&self, context: &CompletionContext) -> Vec<CompletionItem> {
        if context.in_comment || context.in_string {
            return Vec::new();
        }

        let mut seen = HashSet::new();
        let mut current = String::new();
        let mut words = Vec::new();

        for ch in self.buffer.to_string().chars() {
            if is_document_word_char(ch) {
                current.push(ch);
            } else if !current.is_empty() {
                if current != context.prefix && seen.insert(current.clone()) {
                    words.push(CompletionItem::new(
                        current.clone(),
                        current.clone(),
                        CompletionKind::Word,
                        3,
                    ));
                }
                current.clear();
            }
        }

        if !current.is_empty() && current != context.prefix && seen.insert(current.clone()) {
            words.push(CompletionItem::new(
                current.clone(),
                current.clone(),
                CompletionKind::Word,
                3,
            ));
        }

        words
    }

    fn detect_completion_context(&self, manual: bool) -> Option<CompletionContext> {
        let line_index = self.cursor.0;
        let line = self.buffer.line(line_index);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.1.min(chars.len());
        let before: String = chars[..col].iter().collect();
        let in_string = self.is_inside_string(&before);
        let in_comment = self.is_inside_comment(&before);

        let include_prefix = self.lilypond_include_prefix(&before);
        if let Some(prefix) = include_prefix {
            let replace_start = (line_index, col.saturating_sub(prefix.chars().count()));
            let anchor = self.point_from_position(line_index, col)?;
            return Some(CompletionContext {
                kind: CompletionContextKind::GenericWord,
                prefix,
                replace_start,
                replace_end: (line_index, col),
                anchor,
                in_string: true,
                in_comment: false,
                include_path_prefix: Some(chars[replace_start.1..col].iter().collect::<String>()),
            });
        }

        if in_comment {
            return None;
        }

        let kind = if self.syntax == "lilypond" {
            self.detect_lilypond_completion_kind(&before)
        } else {
            CompletionContextKind::GenericWord
        };

        let allow = match kind {
            CompletionContextKind::LilyCommand | CompletionContextKind::LilyScales => {
                is_lily_command_char
            }
            CompletionContextKind::LilyGrobProperties => is_property_token_char,
            _ => is_document_word_char,
        };

        let (start_col, end_col, prefix) = current_token_range(&chars, col, allow);
        if !manual && prefix.is_empty() && matches!(kind, CompletionContextKind::GenericWord) {
            return None;
        }

        let anchor = self.point_from_position(line_index, end_col)?;

        Some(CompletionContext {
            kind,
            prefix,
            replace_start: (line_index, start_col),
            replace_end: (line_index, end_col),
            anchor,
            in_string,
            in_comment,
            include_path_prefix: None,
        })
    }

    fn detect_lilypond_completion_kind(&self, before: &str) -> CompletionContextKind {
        let last_command = last_escaped_word(before).unwrap_or_default();
        let trimmed = before.trim_end();

        if trimmed.ends_with('\\') || current_command_prefix(trimmed).is_some() {
            return CompletionContextKind::LilyCommand;
        }
        if matches!(last_command.as_str(), "new" | "context") {
            return CompletionContextKind::LilyContexts;
        }
        if matches!(last_command.as_str(), "consists" | "remove") {
            return CompletionContextKind::LilyTranslators;
        }
        if last_command == "clef" {
            return CompletionContextKind::LilyClefs;
        }
        if last_command == "key" {
            return CompletionContextKind::LilyScales;
        }
        if last_command == "repeat" {
            return CompletionContextKind::LilyRepeatTypes;
        }
        if last_command == "language" {
            return CompletionContextKind::LilyLanguages;
        }
        if let Some(block) = self.enclosing_lilypond_block(before) {
            if block == "paper" && !before.contains('=') {
                return CompletionContextKind::LilyPaperVariables;
            }
            if block == "header" && !before.contains('=') {
                return CompletionContextKind::LilyHeaderVariables;
            }
        }
        if matches!(last_command.as_str(), "override" | "revert" | "tweak") {
            if property_token_from_line(before).contains('.') {
                return CompletionContextKind::LilyGrobProperties;
            }
            return CompletionContextKind::LilyGrobs;
        }

        CompletionContextKind::GenericWord
    }

    fn enclosing_lilypond_block(&self, before: &str) -> Option<&'static str> {
        let mut depth = 0isize;
        for line_index in (0..=self.cursor.0).rev() {
            let line = if line_index == self.cursor.0 {
                before.to_string()
            } else {
                self.buffer.line(line_index).to_string()
            };
            let chars: Vec<char> = line.chars().collect();
            for idx in (0..chars.len()).rev() {
                match chars[idx] {
                    '}' => depth += 1,
                    '{' => {
                        if depth == 0 {
                            let prefix: String = chars[..idx].iter().collect();
                            if prefix.contains("\\paper") {
                                return Some("paper");
                            }
                            if prefix.contains("\\header") {
                                return Some("header");
                            }
                        } else {
                            depth -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn is_inside_comment(&self, before: &str) -> bool {
        let Some(token) = self.language_config().line_comment else {
            return false;
        };

        let mut quote: Option<char> = None;
        let chars: Vec<char> = before.chars().collect();
        let token_chars: Vec<char> = token.chars().collect();
        let mut idx = 0usize;
        while idx < chars.len() {
            let ch = chars[idx];
            if ch == '\\' {
                idx += 2;
                continue;
            }
            if matches!(ch, '"' | '\'') {
                if quote == Some(ch) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(ch);
                }
                idx += 1;
                continue;
            }
            if quote.is_none()
                && idx + token_chars.len() <= chars.len()
                && chars[idx..idx + token_chars.len()] == token_chars
            {
                return true;
            }
            idx += 1;
        }
        false
    }

    fn is_inside_string(&self, before: &str) -> bool {
        let mut quote: Option<char> = None;
        let mut escaped = false;
        for ch in before.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if matches!(ch, '"' | '\'') {
                if quote == Some(ch) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(ch);
                }
            }
        }
        quote.is_some()
    }

    fn is_inside_lilypond_include_string(&self) -> bool {
        let line = self.buffer.line(self.cursor.0);
        let chars: Vec<char> = line.chars().collect();
        let col = self.cursor.1.min(chars.len());
        let before: String = chars[..col].iter().collect();
        self.lilypond_include_prefix(&before).is_some()
    }

    fn lilypond_include_prefix(&self, before: &str) -> Option<String> {
        if self.syntax != "lilypond" || self.is_inside_comment(before) {
            return None;
        }
        let include_index = before.rfind("\\include")?;
        let after_include = &before[include_index + "\\include".len()..];
        let quote_index = after_include.rfind('"')?;
        let tail = &after_include[quote_index + 1..];
        (!tail.contains('"')).then_some(tail.to_string())
    }

    fn include_path_completion_items(
        &self,
        prefix: &str,
        context: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        let relative_dir = prefix
            .rsplit_once('/')
            .map(|(dir, _)| format!("{dir}/"))
            .unwrap_or_default();
        let partial = prefix
            .rsplit_once('/')
            .map(|(_, tail)| tail)
            .unwrap_or(prefix)
            .to_lowercase();

        let mut bases = Vec::new();
        if let Some(path) = self.document_path.as_deref().and_then(Path::parent) {
            bases.push(path.to_path_buf());
        }
        if let Some(project_root) = self.project_root.clone()
            && !bases.contains(&project_root)
        {
            bases.push(project_root);
        }

        for base in bases {
            let search_dir = base.join(relative_dir.trim_end_matches('/'));
            let Ok(entries) = fs::read_dir(search_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if !partial.is_empty() && !name.to_lowercase().starts_with(&partial) {
                    continue;
                }
                let is_dir = entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false);
                let insert = if is_dir {
                    format!("{relative_dir}{name}/")
                } else {
                    format!("{relative_dir}{name}")
                };
                if seen.insert(insert.clone()) {
                    results.push(CompletionItem::new(
                        insert.clone(),
                        insert,
                        CompletionKind::FilePath,
                        0,
                    ));
                }
            }
        }

        if results.is_empty() && context.prefix.is_empty() {
            self.project_root
                .as_ref()
                .into_iter()
                .filter_map(|root| root.file_name().and_then(|name| name.to_str()))
                .for_each(|name| {
                    results.push(CompletionItem::new(
                        name.to_string(),
                        name.to_string(),
                        CompletionKind::FilePath,
                        0,
                    ));
                });
        }

        results
    }
}

#[derive(Debug, Clone)]
struct CompletionResult {
    anchor: Point,
    prefix: String,
    replace_start: (usize, usize),
    replace_end: (usize, usize),
    items: Vec<CompletionItem>,
}

#[derive(Debug, Clone)]
struct CompletionContext {
    kind: CompletionContextKind,
    prefix: String,
    replace_start: (usize, usize),
    replace_end: (usize, usize),
    anchor: Point,
    in_string: bool,
    in_comment: bool,
    include_path_prefix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionContextKind {
    GenericWord,
    LilyCommand,
    LilyContexts,
    LilyTranslators,
    LilyClefs,
    LilyScales,
    LilyRepeatTypes,
    LilyLanguages,
    LilyPaperVariables,
    LilyHeaderVariables,
    LilyGrobs,
    LilyGrobProperties,
}

fn static_items(items: &[&str], kind: CompletionKind, rank_group: u8) -> Vec<CompletionItem> {
    items
        .iter()
        .map(|item| CompletionItem::new((*item).to_string(), (*item).to_string(), kind, rank_group))
        .collect()
}

fn current_token_range<F>(chars: &[char], col: usize, allow: F) -> (usize, usize, String)
where
    F: Fn(char) -> bool,
{
    let mut start = col.min(chars.len());
    while start > 0 && allow(chars[start - 1]) {
        start -= 1;
    }

    let mut end = col.min(chars.len());
    while end < chars.len() && allow(chars[end]) {
        end += 1;
    }

    (start, end, chars[start..end].iter().collect())
}

fn completion_match_rank(candidate: &str, prefix: &str) -> u8 {
    if prefix.is_empty() {
        return 0;
    }
    if candidate.starts_with(prefix) {
        0
    } else if candidate.to_lowercase().starts_with(&prefix.to_lowercase()) {
        1
    } else {
        2
    }
}

fn last_escaped_word(before: &str) -> Option<String> {
    current_command_prefix(before).or_else(|| {
        before
            .split_whitespace()
            .rev()
            .find(|token| token.starts_with('\\'))
            .map(|token| token.trim_start_matches('\\').to_string())
    })
}

fn current_command_prefix(before: &str) -> Option<String> {
    let token = before.split_whitespace().next_back().unwrap_or_default();
    if !token.starts_with('\\') {
        return None;
    }
    Some(token.trim_start_matches('\\').to_string())
}

fn property_token_from_line(before: &str) -> String {
    before
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .to_string()
}

fn is_document_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-')
}

fn is_lily_command_char(ch: char) -> bool {
    is_document_word_char(ch) || ch == '\\'
}

fn is_property_token_char(ch: char) -> bool {
    is_document_word_char(ch) || matches!(ch, '.' | ':')
}
