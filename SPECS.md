# Sarma Language Specification (v0.7)

## 1. Statements and expressions

Grammar:

```
program      := { statement } EOF
statement    := print-stmt | var-stmt | assign-stmt | if-stmt
print-stmt   := "print" "(" expr ")" terminator-or-eof
var-stmt     := "var" identifier "=" expr terminator-or-eof
assign-stmt  := identifier "=" expr terminator-or-eof
if-stmt      := "if" "(" expr ")" branch [ "else" (branch | if-stmt) ]
branch       := block | statement
block        := "{" { statement } "}"
expr         := or-expr
or-expr      := and-expr { "||" and-expr }
and-expr     := equality { "&&" equality }
equality     := relational { ("==" | "!=") relational }
relational   := additive { ("<" | "<=" | ">" | ">=") additive }
additive     := term { ("+" | "-") term }
term         := primary { ("*" | "/" | "%") primary }
primary      := string | number | boolean | identifier
              | "-" primary | "!" primary | "(" expr ")"
```

- `print`, `var`, `if`, `else`, `true`, and `false` are keywords.
- Sarma is case-sensitive: keywords must be lowercase (`PRINT` is not `print`),
  `Name` and `name` are different variables, and string comparison is code-point
  exact (`"a" == "A"` is false).
- `print(expr)` evaluates `expr` and writes it + newline to stdout.
  Output is written as each `print` runs, so output produced before a runtime error is kept
  and nothing after the error runs.
- `if (cond) { ... }` runs the block when `cond` is truthy (`false`, `0`, and `""`
  are falsy); `else` is optional, and `else if (...) { ... }` chains work.
  A branch is a braced block or a single bare statement: `if (ok) print("yes")`
  needs no braces. A bare nested `if` grabs a following `else` (dangling `else`
  binds to the nearest `if`).
- `var name = expr` declares a new variable; redeclaring with `var` is an error.
- `name = expr` reassigns; the variable must already exist (declare with `var` first).
- Blocks share the surrounding scope: a `var` declared inside a block is visible after it.
- Newlines may surround the braces (`else` may start on the line after `}`), and a
  closing `}` ends a statement, so `if (x) { print("y") }` needs no `;`.
   An `if` statement itself still ends like any other statement (newline, `;`, or EOF).
- `+` is left-associative: string concatenation (`"a" + "b"` → `"ab"`) or numeric addition.
  `*`, `/`, `%` bind tighter than `+`, `-` (`1 + 2 * 3` → `7`); `-` before a primary is unary minus (`-5`).
- Numbers: integer (`42`) or float (`0.5`) literals. `+ - *` on integers stay integers;
  `/` always produces a float (`7 / 2` → `3.5`, `6 / 2` → `3.0`); `%` is remainder.
  Division (or remainder) by zero is an error.
- Types are strict: `+` needs two strings or two numbers (mixing is an error),
  `- * / %` and unary `-` need numbers. Variables hold whatever they were assigned.
- Comparisons are strict (no type coercion):
  - `==` / `!=`: same family compares by value — numbers numerically (`1 == 1.0`
    → `true`), strings and booleans by identity. Mixed families are never equal:
    `1 == "1"` → `false`, `true == 1` → `false`.
  - `< <= > >=`: two numbers compare numerically, two strings character by character
    (`"10" < "9"` → `true`). Anything else — mixed types or booleans — is an error.
  - `!` gives a boolean: `false`, `0`, and `""` are falsy, everything else truthy.
  - `&&` / `||` return an operand, not a boolean: `a && b` is the first falsy side
    or `b`; `a || b` is the first truthy side or `b`. Both short-circuit
    (`false && missing` is `false`, no error).
  - Booleans print as `true` / `false` and are not numbers or strings:
    arithmetic on them is an error.
- `print` displays strings as-is, integers plainly, and whole floats without decimals
  (`3.0` prints as `3`, `3.5` as `3.5`).
- Referencing an undefined variable is an error.
- A line ending with an operator (`+ - * / % = < > ! & |`, two-char operators included) continues the statement on the next line (no terminator emitted).
- Strings: both `"..."` and `'...'` quotes; valid escapes are `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\0` — any other `\x` is an error (no silent backslash-drop, so `"C:\dir"` fails loudly). Strings cannot span lines, and a backslash immediately before a newline is an error too.
- Parentheses inside an expression may nest at most 100 levels deep; deeper is an error.

Examples:

```sarma
var name = "world"
print("hi " + name)   // hi world
name = "sarma"
print(name)           // sarma
print("a" + "b")      // ab
var n = 1 + 2 * 3
print(n)              // 7
print(1 == 1)         // true (strict: 1 == "1" is false)
print(false || "dflt") // dflt (|| returns the operand)
if (n > 5) {
  print("big")
} else {
  print("small")
}                   // small
```

## 2. Statement terminators / semicolon rules (Go-style)

Sarma is newline-terminated with optional `;`. The lexer produces a single `TERMINATOR` token for both cases:

- `;` is always a `TERMINATOR`.
- Newline (`\n`) emits a `TERMINATOR` **only if** the previous token could end a statement:
  - `identifier`, string/number literal, `)`, `]`, `}`
- Otherwise the newline is ignored (statement continues):
  - line ends with an operator (single-char `+ - * / % = < > ! & | : .` or two-char `== != <= >= && || ->`), comma `,`, or opening bracket (`(`, `[`, `{`).
- Newlines are suppressed inside `(...)` and `[...]` (nesting depth > 0), so multi-line
  calls work. Inside `{...}` blocks newlines DO terminate — blocks hold statements.
  (An `if` condition split across lines still works: the line ends with an operator or
  an opening bracket.)
- Brackets must be balanced and correctly paired. An unmatched closing bracket, a mismatched
  pair such as `(]`, and an opening bracket still unclosed at end of file are errors, reported
  with the position of the offending (or unclosed) bracket.
- The two-character operators `==`, `!=`, `<=`, `>=`, `&&`, `||` and `->` are each a single `OP`
  token. A lone `=` is assignment.
- Consecutive terminators collapse (blank lines and `;;` are allowed, skipped by parser).
- Each statement must end with `TERMINATOR`, `}`, or `EOF`. Two statements on the same line
  **must** be separated by `;` (a `}` counts as an ending, so `{ print("x") }` needs none).

Valid:

```sarma
print("a")
print("b");
print("c"); print("d")

print(
  "multiline"
)

// comment line
print("e") // trailing comment
```

Invalid:

```sarma
print("a") print("b")   // ERROR: missing TERMINATOR between statements
print("a"               // ERROR: unclosed '(' (reported at its opening position)
print("a"))             // ERROR: unmatched ')'
print("a"]              // ERROR: mismatched ']' (expected ')')
print('unterminated)    // ERROR: unterminated string
print("C:\dir")         // ERROR: unknown escape sequence '\d'
```

## 3. Comments

- `//` starts a line comment: ignored up to (not including) the newline. The following newline still follows the terminator rule.
- `/* ... */` is a block comment: skipped like whitespace, may span multiple lines. Newlines *inside* the block do **not** emit `TERMINATOR`. Unterminated block = error.
- Block comments do not nest; the first `*/` closes the comment.
- Comment markers inside strings are not comments.

```sarma
// line comment
print("a") // trailing comment
print("b") /* inline block */
print(/* inside parens */ "c")
/* multi
   line
   block */
print("d")
print("/* not a comment */")
```

Token-level summary:

| Previous token ends with | Newline → |
|---|---|
| string, number, identifier, keyword (`print`, `var`, `if`, `else`, `true`, `false`), `)`, `]`, `}` | `TERMINATOR` |
| operator, `,`, `(`, `[`, `{`, start-of-file | no token |
| inside `(...)` / `[...]` (depth > 0) | no token |
| `;` anywhere | `TERMINATOR` |

## 4. Errors and exit codes

Errors are reported on stderr and never as a Python traceback:

```
sarma: error in <path>: <message>        // lexer, parser or runtime error
sarma: cannot read <path>: <reason>      // missing/unreadable file, or not valid UTF-8
Usage: <program> <file.sarma>            // wrong number of arguments
```

Positions are `line:col`, 1-based. Lexing happens before parsing and parsing before execution,
so a lexer error (e.g. an unclosed bracket anywhere in the file) is reported before any parse or
runtime error, and a file with a lexer or parse error runs none of its statements.

| Exit code | Meaning |
|---|---|
| 0 | program ran to completion |
| 1 | lexer, parser or runtime error; file could not be read; stdout closed early (e.g. piped into `head`), silently |
| 2 | usage error |
