# Sarma Language Specification (MVP v0.1)

## 1. Print statement

Grammar:

```
program      := { statement } EOF
statement    := print-stmt
print-stmt   := "print" "(" string ")" terminator-or-eof
```

- `print` is a keyword.
- Exactly one string argument is required (MVP).
- Both quote styles are supported: `"..."` and `'...'`.
- Escape sequences inside strings: `\n`, `\t`, `\r`, `\\`, `\'`, `\"`, `\0`. Unknown escapes resolve to the escaped char (e.g. `\x` → `x`).
- Strings cannot span lines (unterminated newline = error).
- `print` writes its argument + newline to stdout.

Examples:

```sarma
print("Hello world")
print('Hello world')
print("a\nb")  // prints two lines
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
| string, number, identifier, `print`, `)`, `]`, `}` | `TERMINATOR` |
| operator, `,`, `(`, `[`, `{`, start-of-file | no token |
| inside brackets (depth > 0) | no token |
| `;` anywhere | `TERMINATOR` |
