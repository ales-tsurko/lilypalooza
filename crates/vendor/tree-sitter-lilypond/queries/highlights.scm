(comment) @comment

(punctuation) @punctuation

(
  (assignment_lhs)
  .
  (
    (punctuation) @operator
    (#match? @operator "^=$")
  )
)

(assignment_lhs
  (symbol) @property
)

(assignment_lhs
  (property_expression) @property
)

(named_context
  [
    (escaped_word)
    (embedded_scheme)
  ]
  .
  (symbol) @type
)

(named_context
  [
    (escaped_word)
    (embedded_scheme)
  ]
  .
  (symbol)
  .
  (
    (punctuation) @operator
    (#match? @operator "^=$")
  )
  .
  [(symbol) (string)] @variable
)

(chord
  .
  "<" @punctuation.bracket
  ">" @punctuation.bracket
  .
)

(
  (escaped_word) @identifier.variable
  (#not-match? @identifier.variable "^\\\\(?:accepts|alias|book|bookpart|clef|column|consists|context|fill-line|fromproperty|header|if|include|language|layout|maininput|markup|markuplist|midi|new|paper|remove|score|set|tweak|unless|unset|version|vspace|with)$")
)
(
  (escaped_word) @keyword.directive
  (#match? @keyword.directive "^\\\\(?:include|language|maininput|version)$")
)
(
  (escaped_word) @keyword
  (#match? @keyword "^\\\\(?:book|bookpart|context|header|layout|markup|markuplist|midi|new|paper|score|with)$")
)
(
  (escaped_word) @function.builtin
  (#match? @function.builtin "^\\\\(?:accepts|alias|bar|clef|column|consists|fill-line|fromproperty|if|numericTimeSignature|once|override|remove|repeat|revert|set|tempo|time|tweak|unless|unset|vspace)$")
)
(
  (escaped_word) @value.number
  (#match? @value.number "^\\\\(?:breve|longa|maxima)$")
)
(
  (escaped_word) @identifier.core.function
  (#match? @identifier.core.function "^\\\\\\^$")
)

(quoted_identifier
  "\"" @bracket
)

(
  (symbol) @keyword
  (#match? @keyword "^q$")
)

(property_expression) @property

[
  (fraction)
  (decimal_number)
  (unsigned_integer)
] @value.number

(dynamic) @identifier.core.global

(instrument_string_number) @identifier.core.function

(
  (string
    "\"" @string.delimiter.left
    [
      (string_fragment)?
      (escape_sequence)? @string.escape
    ]
    "\"" @string.delimiter.right
  )
) @string

[
  "{" "}"
  "<<" (parallel_music_separator) ">>"
  "#{" "#}"
] @punctuation.bracket

(chord
  ">>" @invalid
)

(embedded_scheme_prefix) @processing

(
  (embedded_scheme
    (embedded_scheme_prefix) @processing
    (embedded_scheme_text) @value.boolean
  )
  (#match? @value.boolean "^#(?:[tT](?:[rR][uU][eE])?|[fF](?:[aA][lL][sS][eE])?)$")
)

(
  (embedded_scheme
    (embedded_scheme_prefix) @processing
    (embedded_scheme_text) @function.builtin
  )
  (#match? @function.builtin "^(?:ly:[^\\s()]+|notehead-link-engraver)$")
)

(
  (embedded_scheme
    (embedded_scheme_prefix) @processing
    (embedded_scheme_text) @property
  )
  (#match? @property "^'?[A-Za-z][A-Za-z0-9-]*:[A-Za-z0-9-]+$")
)
