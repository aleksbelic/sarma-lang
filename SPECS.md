# Sarma Language Specification (v0.3)

## 1. Statements and expressions

Grammar:

```
program      := { statement } EOF
statement    := print-stmt | var-stmt | assign-stmt
print-stmt   := "print" "(" expr ")" terminator-or-eof
var-stmt     := "var" identifier "=" expr terminator-or-eof
assign-stmt  := identifier "=" expr terminator-or-eof
expr         := primary { "+" primary }
primary      := string | identifier | "(" expr ")"
```

- `print` and `var` are keywords.
- `print(expr)` evaluates `expr` (must be a string) and writes it + newline to stdout.
  Output is written as each `print` runs, so output produced before a runtime error is kept
  and nothing after the error runs.
- `var name = expr` declares a new variable; redeclaring with `var` is an error.
- `name = expr` reassigns; the variable must already exist (declare with `var` first).
- `+` is left-associative string concatenation: `"a" + "b" + "c"` → `"abc"`.
- Referencing an undefined variable is an error.
- A line ending with `+` (or `=`) continues the statement on the next line (no terminator emitted).
- Strings: both `"..."` and `'...'` quotes; valid escapes are `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\0` — any other `\x` is an error (no silent backslash-drop, so `"C:\dir"` fails loudly). Strings cannot span lines, and a backslash immediately before a newline is an error too.
- Parentheses inside an expression may nest at most 100 levels deep; deeper is an error.

Examples:

```sarma
var name = "world"
print("hi " + name)   // hi world
name = "sarma"
print(name)           // sarma
print("a" + "b")      // ab
```

## 2. Statement terminators / semicolon rules (Go-style)

Sarma is newline-terminated with optional `;`. The lexer produces a single `TERMINATOR` token for both cases:

- `;` is always a `TERMINATOR`.
- Newline (`\n`) emits a `TERMINATOR` **only if** the previous token could end a statement:
  - `identifier`, string/number literal, `)`, `]`, `}`
- Otherwise the newline is ignored (statement continues):
  - line ends with operator (`+ - * / % = < > ! & | : .`), comma `,`, or opening bracket (`(`, `[`, `{`).
- Newlines are suppressed inside `(...)` / `[...]` / `{...}` (nesting depth > 0), so multi-line calls work.
- Brackets must be balanced and correctly paired. An unmatched closing bracket, a mismatched
  pair such as `(]`, and an opening bracket still unclosed at end of file are errors, reported
  with the position of the offending (or unclosed) bracket.
- The two-character operators `==`, `!=`, `<=`, `>=`, `&&`, `||` and `->` are each a single `OP`
  token. A lone `=` is assignment.
- Consecutive terminators collapse (blank lines and `;;` are allowed, skipped by parser).
- Each statement must end with `TERMINATOR` or `EOF`. Two statements on the same line **must** be separated by `;`.

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
| string, number, identifier, keyword (`print`, `var`), `)`, `]`, `}` | `TERMINATOR` |
| operator, `,`, `(`, `[`, `{`, start-of-file | no token |
| inside brackets (depth > 0) | no token |
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
