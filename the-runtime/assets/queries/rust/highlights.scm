[
  "fn"
  "let"
  "if"
  "else"
  "match"
  "for"
  "while"
  "loop"
  "return"
  "break"
  "continue"
  "impl"
  "trait"
  "struct"
  "enum"
  "mod"
  "use"
  "pub"
  "const"
  "static"
] @keyword

(type_identifier) @type
(primitive_type) @type
(identifier) @variable
(field_identifier) @variable.other.member
(parameter pattern: (identifier) @variable.parameter)

(function_item
  name: (identifier) @function)
(call_expression
  function: (identifier) @function)

(boolean_literal) @constant.builtin.boolean
(integer_literal) @constant.numeric.integer
(float_literal) @constant.numeric.float
(char_literal) @string
(string_literal) @string
(raw_string_literal) @string

(line_comment) @comment
(block_comment) @comment

[
  "::"
  "."
  ","
  ":"
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket
