//! Parser: tokens to AST, with the full precedence chain, blocks, `if`/`else`
//! (including bare single-statement branches), and the MAX_NESTING guard.
//! Mirrors `sarma.py`, including exact error message text.

use crate::error::{py_repr, SarmaError};
use crate::lexer::{Token, TokenKind};

/// Maximum depth of nested parentheses inside an expression.
pub const MAX_NESTING: usize = 100;

#[derive(Debug, Clone, Copy)]
pub enum NumVal {
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // `line` fields mirror the Python AST; most are diagnostics-only.
pub enum Expr {
    Str(String),
    Num(NumVal),
    Bool(bool),
    Var(String, usize),
    Neg(Box<Expr>, usize),
    Not(Box<Expr>, usize),
    /// Flat `+`/`-` chain (left-associative): operands, operators, first-op line.
    Add(Vec<Expr>, Vec<String>, usize),
    /// Flat `*`/`/`/`%` chain (left-associative).
    Mul(Vec<Expr>, Vec<String>, usize),
    /// Flat `==`/`!=` chain (left-associative).
    Eq(Vec<Expr>, Vec<String>, usize),
    /// Flat `<`/`<=`/`>`/`>=` chain (left-associative).
    Rel(Vec<Expr>, Vec<String>, usize),
    /// `&&` chain (short-circuits).
    And(Vec<Expr>),
    /// `||` chain (short-circuits).
    Or(Vec<Expr>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // `line` fields mirror the Python AST; most are diagnostics-only.
pub enum Stmt {
    Print(Expr, usize),
    Var(String, Expr, usize),
    Assign(String, Expr, usize),
    Block(Vec<Stmt>, usize),
    If {
        cond: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
        line: usize,
    },
}

pub struct Parser {
    tokens: Vec<Token>,
    i: usize,
    nesting: usize, // parenthesis depth inside the current expression
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            i: 0,
            nesting: 0,
        }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.i]
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.i].clone();
        self.i += 1;
        tok
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, SarmaError> {
        let tok = self.peek().clone();
        if tok.kind != kind {
            return Err(SarmaError(format!(
                "Expected {} but got {} ({}) at {}:{}",
                kind.name(),
                tok.kind.name(),
                py_repr(&tok.value),
                tok.line,
                tok.col
            )));
        }
        Ok(self.advance())
    }

    pub fn parse(mut self) -> Result<Vec<Stmt>, SarmaError> {
        let mut stmts = Vec::new();
        loop {
            self.skip_terminators();
            if self.peek().kind == TokenKind::Eof {
                break;
            }
            stmts.push(self.parse_statement()?);
            self.finish_statement()?;
        }
        Ok(stmts)
    }

    fn skip_terminators(&mut self) {
        while self.peek().kind == TokenKind::Terminator {
            self.advance();
        }
    }

    fn finish_statement(&mut self) -> Result<(), SarmaError> {
        // Each statement ends with TERMINATOR or EOF; a closing brace ends one
        // too, so single-line blocks like `{ print("x") }` need no `;`.
        let nxt = self.peek().clone();
        match nxt.kind {
            TokenKind::Terminator => {
                self.advance();
                Ok(())
            }
            TokenKind::RBrace | TokenKind::Eof => Ok(()),
            _ => Err(SarmaError(format!(
                "Expected end of statement (newline or ';') at {}:{}, got {}",
                nxt.line,
                nxt.col,
                nxt.kind.name()
            ))),
        }
    }

    fn parse_statement(&mut self) -> Result<Stmt, SarmaError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Print => self.parse_print(),
            TokenKind::Var => self.parse_var(),
            TokenKind::If => self.parse_if(),
            TokenKind::Identifier => self.parse_assign(),
            _ => Err(SarmaError(format!(
                "Unknown statement starting with {} ({}) at {}:{}. Expected print, var, if, or assignment.",
                tok.kind.name(),
                py_repr(&tok.value),
                tok.line,
                tok.col
            ))),
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, SarmaError> {
        let kw = self.expect(TokenKind::If)?;
        self.expect(TokenKind::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(TokenKind::RParen)?;
        self.skip_terminators();
        let then = self.parse_branch()?;
        // Look for `else` past any newlines, but restore them if absent:
        // they still separate this statement from the next one.
        let mark = self.i;
        self.skip_terminators();
        let mut otherwise = None;
        if self.peek().kind == TokenKind::Else {
            self.advance();
            self.skip_terminators();
            if self.peek().kind == TokenKind::If {
                otherwise = Some(Box::new(self.parse_if()?));
            } else {
                otherwise = Some(Box::new(self.parse_branch()?));
            }
        } else {
            self.i = mark;
        }
        Ok(Stmt::If {
            cond,
            then: Box::new(then),
            otherwise,
            line: kw.line,
        })
    }

    fn parse_branch(&mut self) -> Result<Stmt, SarmaError> {
        // A branch is a braced block or a single bare statement.
        if self.peek().kind == TokenKind::LBrace {
            self.parse_block()
        } else {
            self.parse_statement()
        }
    }

    fn parse_block(&mut self) -> Result<Stmt, SarmaError> {
        let tok = self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        loop {
            self.skip_terminators();
            if self.peek().kind == TokenKind::RBrace {
                self.advance();
                break;
            }
            stmts.push(self.parse_statement()?);
            self.finish_statement()?;
        }
        Ok(Stmt::Block(stmts, tok.line))
    }

    fn parse_var(&mut self) -> Result<Stmt, SarmaError> {
        let kw = self.expect(TokenKind::Var)?;
        let name = self.expect(TokenKind::Identifier)?;
        self.expect(TokenKind::Assign)?;
        let expr = self.parse_expr()?;
        Ok(Stmt::Var(name.value, expr, kw.line))
    }

    fn parse_assign(&mut self) -> Result<Stmt, SarmaError> {
        let name = self.expect(TokenKind::Identifier)?;
        self.expect(TokenKind::Assign)?;
        let expr = self.parse_expr()?;
        Ok(Stmt::Assign(name.value, expr, name.line))
    }

    fn parse_print(&mut self) -> Result<Stmt, SarmaError> {
        let kw = self.expect(TokenKind::Print)?;
        self.expect(TokenKind::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(TokenKind::RParen)?;
        Ok(Stmt::Print(expr, kw.line))
    }

    fn parse_expr(&mut self) -> Result<Expr, SarmaError> {
        // Logical-or level (lowest precedence).
        let mut parts = vec![self.parse_and()?];
        while self.peek().kind == TokenKind::Op && self.peek().value == "||" {
            self.advance();
            parts.push(self.parse_and()?);
        }
        if parts.len() == 1 {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::Or(parts))
        }
    }

    fn parse_and(&mut self) -> Result<Expr, SarmaError> {
        let mut parts = vec![self.parse_equality()?];
        while self.peek().kind == TokenKind::Op && self.peek().value == "&&" {
            self.advance();
            parts.push(self.parse_equality()?);
        }
        if parts.len() == 1 {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::And(parts))
        }
    }

    fn parse_equality(&mut self) -> Result<Expr, SarmaError> {
        let mut parts = vec![self.parse_relational()?];
        let mut ops: Vec<String> = Vec::new();
        let mut op_line = 0;
        while self.peek().kind == TokenKind::Op
            && (self.peek().value == "==" || self.peek().value == "!=")
        {
            let tok = self.advance();
            if ops.is_empty() {
                op_line = tok.line;
            }
            ops.push(tok.value);
            parts.push(self.parse_relational()?);
        }
        if ops.is_empty() {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::Eq(parts, ops, op_line))
        }
    }

    fn parse_relational(&mut self) -> Result<Expr, SarmaError> {
        let mut parts = vec![self.parse_additive()?];
        let mut ops: Vec<String> = Vec::new();
        let mut op_line = 0;
        while self.peek().kind == TokenKind::Op
            && ["<", "<=", ">", ">="].contains(&self.peek().value.as_str())
        {
            let tok = self.advance();
            if ops.is_empty() {
                op_line = tok.line;
            }
            ops.push(tok.value);
            parts.push(self.parse_additive()?);
        }
        if ops.is_empty() {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::Rel(parts, ops, op_line))
        }
    }

    fn parse_additive(&mut self) -> Result<Expr, SarmaError> {
        let mut parts = vec![self.parse_term()?];
        let mut ops: Vec<String> = Vec::new();
        let mut op_line = 0;
        loop {
            let tok = self.peek().clone();
            let op = if tok.kind == TokenKind::Plus {
                "+"
            } else if tok.kind == TokenKind::Op && tok.value == "-" {
                "-"
            } else {
                break;
            };
            self.advance();
            if ops.is_empty() {
                op_line = tok.line;
            }
            ops.push(op.to_string());
            parts.push(self.parse_term()?);
        }
        if ops.is_empty() {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::Add(parts, ops, op_line))
        }
    }

    fn parse_term(&mut self) -> Result<Expr, SarmaError> {
        let mut parts = vec![self.parse_unary()?];
        let mut ops: Vec<String> = Vec::new();
        let mut op_line = 0;
        while self.peek().kind == TokenKind::Op
            && ["*", "/", "%"].contains(&self.peek().value.as_str())
        {
            let tok = self.advance();
            if ops.is_empty() {
                op_line = tok.line;
            }
            ops.push(tok.value);
            parts.push(self.parse_unary()?);
        }
        if ops.is_empty() {
            Ok(parts.into_iter().next().unwrap())
        } else {
            Ok(Expr::Mul(parts, ops, op_line))
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, SarmaError> {
        // Prefix `-`/`!` operators are collected iteratively (no recursion),
        // then wrapped outside-in so long chains can't overflow the stack.
        let mut ops: Vec<(bool, usize)> = Vec::new(); // (is_not, line)
        loop {
            let tok = self.peek().clone();
            if tok.kind == TokenKind::Op && (tok.value == "-" || tok.value == "!") {
                ops.push((tok.value == "!", tok.line));
                self.advance();
            } else {
                break;
            }
        }
        let mut node = self.parse_atom()?;
        for (is_not, line) in ops.into_iter().rev() {
            node = if is_not {
                Expr::Not(Box::new(node), line)
            } else {
                Expr::Neg(Box::new(node), line)
            };
        }
        Ok(node)
    }

    fn parse_atom(&mut self) -> Result<Expr, SarmaError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::String => {
                self.advance();
                Ok(Expr::Str(tok.value))
            }
            TokenKind::Number => {
                self.advance();
                make_number(&tok)
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Identifier => {
                self.advance();
                Ok(Expr::Var(tok.value, tok.line))
            }
            TokenKind::LParen => {
                self.advance();
                self.nesting += 1;
                if self.nesting > MAX_NESTING {
                    return Err(SarmaError(format!(
                        "Expression nested too deeply (more than {MAX_NESTING} levels) at {}:{}",
                        tok.line, tok.col
                    )));
                }
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                self.nesting -= 1;
                Ok(expr)
            }
            _ => Err(SarmaError(format!(
                "Expected string, number, boolean, variable, or '(' but got {} ({}) at {}:{}",
                tok.kind.name(),
                py_repr(&tok.value),
                tok.line,
                tok.col
            ))),
        }
    }
}

fn make_number(tok: &Token) -> Result<Expr, SarmaError> {
    let s = &tok.value;
    let value = if s.contains('.') {
        NumVal::Float(s.parse::<f64>().map_err(|_| {
            SarmaError(format!(
                "Invalid number {} at {}:{}",
                py_repr(s),
                tok.line,
                tok.col
            ))
        })?)
    } else {
        NumVal::Int(s.parse::<i64>().map_err(|_| {
            SarmaError(format!(
                "Invalid number {} at {}:{}",
                py_repr(s),
                tok.line,
                tok.col
            ))
        })?)
    };
    Ok(Expr::Num(value))
}
