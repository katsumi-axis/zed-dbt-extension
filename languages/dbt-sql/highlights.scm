(identifier) @variable

(object_reference (identifier) @type)
(cte (identifier) @type)
(relation (identifier) @type)

(invocation
  (object_reference
    name: (identifier) @function.call))

(field
  name: (identifier) @field)

(column_definition
  name: (identifier) @field)

(column
  name: (identifier) @field)

(term
  alias: (identifier) @variable)

(literal) @string
(comment) @comment
(marginalia) @comment

((literal) @number
  (#match? @number "^[-+]?%d+$"))

((literal) @float
  (#match? @float "^[-+]?%d*\\.\\d*$"))

(parameter) @parameter

[
  (keyword_true)
  (keyword_false)
] @boolean

(keyword_null) @constant.builtin

[
  (keyword_select)
  (keyword_from)
  (keyword_where)
  (keyword_join)
  (keyword_left)
  (keyword_right)
  (keyword_outer)
  (keyword_inner)
  (keyword_full)
  (keyword_order)
  (keyword_group)
  (keyword_with)
  (keyword_as)
  (keyword_having)
  (keyword_limit)
  (keyword_table)
  (keyword_view)
  (keyword_end)
  (keyword_is)
  (keyword_between)
  (keyword_all)
  (keyword_case)
  (keyword_when)
  (keyword_then)
  (keyword_else)
  (keyword_distinct)
] @keyword

[
  (keyword_int)
  (keyword_boolean)
  (keyword_float)
  (keyword_numeric)
  (keyword_date)
  (keyword_datetime)
  (keyword_time)
  (keyword_timestamp)
  (keyword_text)
  (keyword_string)
] @type.builtin

[
  (keyword_in)
  (keyword_and)
  (keyword_or)
  (keyword_not)
  (keyword_by)
  (keyword_on)
  (keyword_union)
] @keyword.operator

[
  "+"
  "-"
  "*"
  "/"
  "%"
  "^"
  ":="
  "="
  "<"
  "<="
  "!="
  ">="
  ">"
  "<>"
  (op_other)
  (op_unary_other)
] @operator

[
  "("
  ")"
] @punctuation.bracket

[
  ";"
  ","
  "."
] @punctuation.delimiter
