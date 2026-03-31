[
  (scheme_comment)
] @comment

(
  (scheme_symbol) @operator
  (#match? @operator "^([*+/=<>-]|[<>]=)$")
)

(
  (scheme_string
    "\"" @string.delimiter.left
    [
      (scheme_string_fragment)?
      (scheme_escape_sequence)? @string.escape
    ]
    "\"" @string.delimiter.right
  )
) @string

(scheme_keyword
  "#:" @punctuation
  (scheme_keyword_name) @parameter
)

(scheme_boolean) @value.boolean

(scheme_number) @value.number

(scheme_character) @value.entity

(scheme_quote . "'" @operator)
(scheme_quasiquote . "`" @operator)
(scheme_unquote . "," @operator)
(scheme_unquote_splicing . ",@" @operator)

(
  (scheme_symbol) @keyword
  (#match? @keyword "^(?:and|begin|case|cond|define(?:-[A-Za-z0-9_!+*/:<>=?-]+)?|delay|do|if|lambda|let(?:\\*|rec(?:\\*)?)?|or|quasiquote|quote|set!|unquote|unquote-splicing|unless|when)$")
)

(
  (scheme_symbol) @function.builtin
  (#match? @function.builtin "^(?:call-with-[^\\s()]+|format|ly:[^\\s()]+|make-[^\\s()]+|map|notehead-link-engraver|set-global-fonts|string-[^\\s()]+)$")
)

(
  (scheme_symbol) @property
  (#match? @property "^[A-Za-z][A-Za-z0-9-]*:[A-Za-z0-9-]+$")
)

(
  (scheme_list
    .
    (scheme_symbol) @function
  )
  (#not-match? @function "^(?:and|begin|case|cond|define(?:-[A-Za-z0-9_!+*/:<>=?-]+)?|delay|do|if|lambda|let(?:\\*|rec(?:\\*)?)?|or|quasiquote|quote|set!|unquote|unquote-splicing|unless|when|call-with-[^\\s()]+|format|ly:[^\\s()]+|make-[^\\s()]+|map|notehead-link-engraver|set-global-fonts|string-[^\\s()]+|[A-Za-z][A-Za-z0-9-]*:[A-Za-z0-9-]+)$")
)

(
  (scheme_symbol) @identifier
  (#not-match? @identifier "^([*+/=<>-]|[<>]=|and|begin|case|cond|define(?:-[A-Za-z0-9_!+*/:<>=?-]+)?|delay|do|if|lambda|let(?:\\*|rec(?:\\*)?)?|or|quasiquote|quote|set!|unquote|unquote-splicing|unless|when|call-with-[^\\s()]+|format|ly:[^\\s()]+|make-[^\\s()]+|map|notehead-link-engraver|set-global-fonts|string-[^\\s()]+|[A-Za-z][A-Za-z0-9-]*:[A-Za-z0-9-]+)$")
)

[
  "(" ")"
  "#{" "#}"
] @punctuation.bracket
