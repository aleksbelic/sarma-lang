//! Lexer: characters to tokens, including the Go-style statement-terminator
//! rule, bracket-balance checking, comments, strings, numbers, and keywords.
//! Mirrors `sarma.py` token for token.

use crate::error::{py_char_repr, SarmaError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Print,
    Var,
    If,
    Else,
    True,
    False,
    Identifier,
    String,
    Number,
    Plus,
    Assign,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
    Comma,
    Op,
    Terminator,
    Eof,
}

impl TokenKind {
    /// Exact variant names, as shown in parser error messages.
    pub fn name(self) -> &'static str {
        match self {
            TokenKind::Print => "PRINT",
            TokenKind::Var => "VAR",
            TokenKind::If => "IF",
            TokenKind::Else => "ELSE",
            TokenKind::True => "TRUE",
            TokenKind::False => "FALSE",
            TokenKind::Identifier => "IDENTIFIER",
            TokenKind::String => "STRING",
            TokenKind::Number => "NUMBER",
            TokenKind::Plus => "PLUS",
            TokenKind::Assign => "ASSIGN",
            TokenKind::LParen => "LPAREN",
            TokenKind::RParen => "RPAREN",
            TokenKind::LBrack => "LBRACK",
            TokenKind::RBrack => "RBRACK",
            TokenKind::LBrace => "LBRACE",
            TokenKind::RBrace => "RBRACE",
            TokenKind::Comma => "COMMA",
            TokenKind::Op => "OP",
            TokenKind::Terminator => "TERMINATOR",
            TokenKind::Eof => "EOF",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub value: String,
    pub line: usize,
    pub col: usize,
}

/// Closing bracket for each opening bracket.
fn closing(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => unreachable!(),
    }
}

/// Supported string escapes; any other backslash sequence is an error.
fn escape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        '\'' => Some('\''),
        '"' => Some('"'),
        '0' => Some('\0'),
        _ => None,
    }
}

/// Two-character operators are lexed as a single OP token.
const TWO_CHAR_OPS: [&str; 7] = ["==", "!=", "<=", ">=", "&&", "||", "->"];

/// Single-character operators (each its own OP token, except `+` and `=`).
const SINGLE_OPS: &str = "-*/%<>!&|:.";

pub struct Lexer {
    chars: Vec<char>,
    n: usize,
    i: usize,
    line: usize,
    col: usize,
    tokens: Vec<Token>,
    /// Go-style: does the previous token allow a statement end?
    prev_can_end: bool,
    /// Open `(`/`[`/`{`: (char, line, col).
    brackets: Vec<(char, usize, usize)>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let n = chars.len();
        Lexer {
            chars,
            n,
            i: 0,
            line: 1,
            col: 1,
            tokens: Vec::new(),
            prev_can_end: false,
            brackets: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn advance(&mut self) -> char {
        let ch = self.chars[self.i];
        self.i += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        ch
    }

    fn add(&mut self, kind: TokenKind, value: String, line: usize, col: usize) {
        self.tokens.push(Token {
            kind,
            value,
            line,
            col,
        });
    }

    fn close_bracket(&mut self, ch: char) -> Result<(), SarmaError> {
        let (line, col) = (self.line, self.col);
        match self.brackets.last() {
            None => Err(SarmaError(format!(
                "Unmatched {} at {line}:{col}",
                py_char_repr(ch)
            ))),
            Some(&(open, _, _)) if closing(open) == ch => {
                self.brackets.pop();
                Ok(())
            }
            Some(&(open, open_line, open_col)) => Err(SarmaError(format!(
                "Mismatched {} at {line}:{col}: expected {} to close {} opened at {open_line}:{open_col}",
                py_char_repr(ch),
                py_char_repr(closing(open)),
                py_char_repr(open),
            ))),
        }
    }

    pub fn lex(mut self) -> Result<Vec<Token>, SarmaError> {
        while self.i < self.n {
            let ch = self.peek().unwrap();
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
                continue;
            }
            if ch == '\n' {
                let (line, col) = (self.line, self.col);
                self.advance();
                // Suppress newline-terminators inside (...) and [...] so
                // multi-line calls work naturally. Inside {...} blocks
                // newlines DO terminate, since blocks hold statements.
                let innermost = self.brackets.last().map(|&(c, _, _)| c);
                if (innermost.is_none() || innermost == Some('{')) && self.prev_can_end {
                    self.add(TokenKind::Terminator, "\\n".to_string(), line, col);
                    self.prev_can_end = false;
                }
                continue;
            }
            if ch == '/'
                && self.i + 1 < self.n
                && self.chars[self.i + 1] == '*'
            {
                // /* block comment */: skipped like whitespace.
                let (line, col) = (self.line, self.col);
                self.advance(); // /
                self.advance(); // *
                let mut closed = false;
                while self.i < self.n {
                    if self.peek() == Some('*')
                        && self.i + 1 < self.n
                        && self.chars[self.i + 1] == '/'
                    {
                        self.advance(); // *
                        self.advance(); // /
                        closed = true;
                        break;
                    }
                    self.advance();
                }
                if !closed {
                    return Err(SarmaError(format!(
                        "Unterminated block comment starting at {line}:{col}"
                    )));
                }
                continue;
            }
            if ch == '/'
                && self.i + 1 < self.n
                && self.chars[self.i + 1] == '/'
            {
                // // comment: skip to end of line, newline handled next loop.
                while self.i < self.n && self.peek() != Some('\n') {
                    self.advance();
                }
                continue;
            }
            if ch == ';' {
                self.add(
                    TokenKind::Terminator,
                    ";".to_string(),
                    self.line,
                    self.col,
                );
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch == '\'' || ch == '"' {
                self.lex_string()?;
                continue;
            }
            if ch == '(' {
                self.add(TokenKind::LParen, "(".to_string(), self.line, self.col);
                self.brackets.push((ch, self.line, self.col));
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch == ')' {
                self.close_bracket(ch)?;
                self.add(TokenKind::RParen, ")".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = true;
                continue;
            }
            if ch == '[' {
                self.add(TokenKind::LBrack, "[".to_string(), self.line, self.col);
                self.brackets.push((ch, self.line, self.col));
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch == ']' {
                self.close_bracket(ch)?;
                self.add(TokenKind::RBrack, "]".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = true;
                continue;
            }
            if ch == '{' {
                self.add(TokenKind::LBrace, "{".to_string(), self.line, self.col);
                self.brackets.push((ch, self.line, self.col));
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch == '}' {
                self.close_bracket(ch)?;
                self.add(TokenKind::RBrace, "}".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = true;
                continue;
            }
            if ch == ',' {
                self.add(TokenKind::Comma, ",".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if self.i + 1 < self.n {
                let two: String = [ch, self.chars[self.i + 1]].iter().collect();
                if TWO_CHAR_OPS.contains(&two.as_str()) {
                    self.add(TokenKind::Op, two, self.line, self.col);
                    self.advance();
                    self.advance();
                    self.prev_can_end = false;
                    continue;
                }
            }
            if ch == '+' {
                self.add(TokenKind::Plus, "+".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch == '=' {
                self.add(TokenKind::Assign, "=".to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if SINGLE_OPS.contains(ch) {
                self.add(TokenKind::Op, ch.to_string(), self.line, self.col);
                self.advance();
                self.prev_can_end = false;
                continue;
            }
            if ch.is_alphabetic() || ch == '_' {
                self.lex_ident();
                continue;
            }
            if ch.is_numeric() {
                self.lex_number();
                continue;
            }
            return Err(SarmaError(format!(
                "Unexpected character {} at {}:{}",
                py_char_repr(ch),
                self.line,
                self.col
            )));
        }
        if let Some(&(ch, line, col)) = self.brackets.last() {
            return Err(SarmaError(format!(
                "Unclosed {} opened at {line}:{col}",
                py_char_repr(ch)
            )));
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            value: String::new(),
            line: self.line,
            col: self.col,
        });
        Ok(self.tokens)
    }

    fn lex_ident(&mut self) {
        let (line, col) = (self.line, self.col);
        let start = self.i;
        while self.i < self.n
            && (self.chars[self.i].is_alphanumeric() || self.chars[self.i] == '_')
        {
            self.advance();
        }
        let word: String = self.chars[start..self.i].iter().collect();
        let kind = match word.as_str() {
            "print" => TokenKind::Print,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Identifier,
        };
        self.add(kind, word, line, col);
        self.prev_can_end = true;
    }

    fn lex_number(&mut self) {
        let (line, col) = (self.line, self.col);
        let start = self.i;
        while self.i < self.n
            && (self.chars[self.i].is_numeric() || self.chars[self.i] == '.')
        {
            self.advance();
        }
        let text: String = self.chars[start..self.i].iter().collect();
        self.add(TokenKind::Number, text, line, col);
        self.prev_can_end = true;
    }

    fn lex_string(&mut self) -> Result<(), SarmaError> {
        let (line, col) = (self.line, self.col); // position of the opening quote
        let quote = self.advance(); // consume opening quote
        let mut buf = String::new();
        let mut closed = false;
        while self.i < self.n {
            let (esc_line, esc_col) = (self.line, self.col);
            let ch = self.advance();
            if ch == '\\' && self.i < self.n {
                let esc = self.advance();
                if esc == '\n' || esc == '\r' {
                    return Err(SarmaError(format!(
                        "Strings cannot span lines (backslash at end of line at {esc_line}:{esc_col})"
                    )));
                }
                match escape(esc) {
                    Some(v) => buf.push(v),
                    None => {
                        return Err(SarmaError(format!(
                            "Unknown escape sequence '\\{esc}' at {esc_line}:{esc_col}"
                        )))
                    }
                }
                continue;
            }
            if ch == quote {
                closed = true;
                break;
            }
            if ch == '\n' {
                return Err(SarmaError(format!(
                    "Unterminated string starting at {line}:{col}"
                )));
            }
            buf.push(ch);
        }
        if !closed {
            return Err(SarmaError(format!(
                "Unterminated string starting at {line}:{col}"
            )));
        }
        self.add(TokenKind::String, buf, line, col);
        self.prev_can_end = true;
        Ok(())
    }
}
