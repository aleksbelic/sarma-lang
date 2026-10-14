//! Shared error type. Every user-facing failure is a `SarmaError` with the exact
//! message text the Python interpreter produces; the CLI turns it into
//! `sarma: error in <path>: <message>` on stderr with exit code 1.

#[derive(Debug)]
pub struct SarmaError(pub String);

impl std::fmt::Display for SarmaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SarmaError {}

/// Single-quoted representation mirroring Python's `repr()` for the simple
/// values that appear in error messages (identifiers, operators, numbers).
pub fn py_repr(s: &str) -> String {
    if !s.contains('\'') {
        return format!("'{}'", esc_py(s, '\''));
    }
    if !s.contains('"') {
        return format!("\"{}\"", esc_py(s, '"'));
    }
    format!("'{}'", esc_py(s, '\''))
}

fn esc_py(s: &str, quote: char) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Python `repr()` of a single character, e.g. `'@'`, `'\\'`, `'\n'`.
pub fn py_char_repr(c: char) -> String {
    match c {
        '\\' => "'\\\\'".to_string(),
        '\n' => "'\\n'".to_string(),
        '\r' => "'\\r'".to_string(),
        '\t' => "'\\t'".to_string(),
        '\'' => "\"'\"".to_string(),
        c if (c as u32) < 0x20 => format!("'\\x{:02x}'", c as u32),
        c => format!("'{c}'"),
    }
}
