# Sarma Language Specification (v0.2)

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
- `var name = expr` declares a new variable; redeclaring with `var` is an error.
- `name = expr` reassigns; the variable must already exist (declare with `var` first).
- `+` is left-associative string concatenation: `"a" + "b" + "c"` → `"abc"`.
- Referencing an undefined variable is an error.
- A line ending with `+` (or `=`) continues the statement on the next line (no terminator emitted).
- Strings: both `"..."` and `'...'` quotes; valid escapes are `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\0` — any other `\x` is an error (no silent backslash-drop, so `"C:\dir"` fails loudly). A raw newline inside a string is an error; backslash-newline is a line continuation (both characters skipped, value gets no newline).

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
print("a"               // ERROR: missing ")"
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

## 4. Runtime behavior and errors

- Output streams: each `print` writes immediately. If a later statement fails, earlier output is kept — e.g. `print("before")` followed by `print(missing)` still shows `before` before the error.
- Errors (all reported as `sarma: error in <file>: <message>`, exit 1 — never a raw traceback):
  - undefined variable, variable redefinition, assignment without `var`;
  - unterminated string or block comment, unknown escape sequence;
  - `expression too deeply nested` for absurdly deep `((((...))))` nesting (long `+` chains are fine — no practical limit).
- Unreadable files (missing file, bad encoding) report `sarma: cannot read file <path>: ...`, exit 1.
