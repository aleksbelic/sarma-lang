#!/usr/bin/env python3
"""Sarma programming language - interpreter (print + variables + concat).

Usage:
    python3 sarma.py <file.sarma>
    python3 sarma.py examples/print.sarma

Syntax:
    var name = "world"
    name = "sarma"
    print("hi " + name)
    print("one"); print("two")   # ; is a statement terminator, same as newline

Terminator rule (Go-style, as specified):
    - Newline emits a TERMINATOR token only if the previous token could end
      a statement (identifier, literal, ), ], }).
    - If a line ends with an operator, comma, or opening bracket, the
      statement continues naturally (no terminator).
    - `;` is always a TERMINATOR token.
    - Inside (...)/[...]/ {...} newlines never terminate.
    - `//` starts a line comment, `/* ... */` is a block comment.
"""

import sys
from dataclasses import dataclass
from enum import Enum, auto


class TokenType(Enum):
    PRINT = auto()
    VAR = auto()
    IDENTIFIER = auto()
    STRING = auto()
    NUMBER = auto()
    PLUS = auto()     # +
    ASSIGN = auto()   # =
    LPAREN = auto()   # (
    RPAREN = auto()   # )
    LBRACK = auto()   # [
    RBRACK = auto()   # ]
    LBRACE = auto()   # {
    RBRACE = auto()   # }
    COMMA = auto()
    OP = auto()       # - * / etc. (for continuation logic / future use)
    TERMINATOR = auto()  # newline-inserted or `;`
    EOF = auto()


@dataclass
class Token:
    type: TokenType
    value: str
    line: int
    col: int

    def __repr__(self):
        return f"{self.type.name}({self.value!r})@{self.line}:{self.col}"


class SarmaError(Exception):
    pass


class Lexer:
    def __init__(self, source: str):
        self.src = source
        self.n = len(source)
        self.i = 0
        self.line = 1
        self.col = 1
        self.tokens: list[Token] = []
        # Go-style: does previous token allow a statement end?
        self.prev_can_end = False
        self.depth = 0  # (), [], {} nesting

    def peek(self) -> str:
        return self.src[self.i] if self.i < self.n else ""

    def advance(self) -> str:
        ch = self.src[self.i]
        self.i += 1
        if ch == "\n":
            self.line += 1
            self.col = 1
        else:
            self.col += 1
        return ch

    def add(self, type: TokenType, value: str, line: int, col: int):
        self.tokens.append(Token(type, value, line, col))

    def lex(self) -> list[Token]:
        while self.i < self.n:
            ch = self.peek()
            if ch in (" ", "\t", "\r"):
                self.advance()
                continue
            if ch == "\n":
                line, col = self.line, self.col
                self.advance()
                # Suppress newline-terminators inside brackets so
                #   print(
                #     "hi"
                #   )
                # works naturally.
                if self.depth == 0 and self.prev_can_end:
                    self.add(TokenType.TERMINATOR, "\\n", line, col)
                    self.prev_can_end = False
                continue
            if ch == "/" and self.i + 1 < self.n and self.src[self.i + 1] == "*":
                # /* block comment */: skipped like whitespace.
                # Newlines inside do NOT emit TERMINATOR.
                line, col = self.line, self.col
                self.advance()  # /
                self.advance()  # *
                closed = False
                while self.i < self.n:
                    if self.peek() == "*" and self.i + 1 < self.n and self.src[self.i + 1] == "/":
                        self.advance()  # *
                        self.advance()  # /
                        closed = True
                        break
                    self.advance()
                if not closed:
                    raise SarmaError(f"Unterminated block comment starting at {line}:{col}")
                continue
            if ch == "/" and self.i + 1 < self.n and self.src[self.i + 1] == "/":
                # // comment: skip to end of line, newline handled next loop
                while self.i < self.n and self.peek() != "\n":
                    self.advance()
                continue
            if ch == ";":
                self.add(TokenType.TERMINATOR, ";", self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            if ch in ("'", '"'):
                self.lex_string()
                continue
            if ch == "(":
                self.add(TokenType.LPAREN, ch, self.line, self.col)
                self.advance()
                self.depth += 1
                self.prev_can_end = False
                continue
            if ch == ")":
                self.add(TokenType.RPAREN, ch, self.line, self.col)
                self.advance()
                self.depth = max(0, self.depth - 1)
                self.prev_can_end = True
                continue
            if ch == "[":
                self.add(TokenType.LBRACK, ch, self.line, self.col)
                self.advance()
                self.depth += 1
                self.prev_can_end = False
                continue
            if ch == "]":
                self.add(TokenType.RBRACK, ch, self.line, self.col)
                self.advance()
                self.depth = max(0, self.depth - 1)
                self.prev_can_end = True
                continue
            if ch == "{":
                self.add(TokenType.LBRACE, ch, self.line, self.col)
                self.advance()
                self.depth += 1
                self.prev_can_end = False
                continue
            if ch == "}":
                self.add(TokenType.RBRACE, ch, self.line, self.col)
                self.advance()
                self.depth = max(0, self.depth - 1)
                self.prev_can_end = True
                continue
            if ch == ",":
                self.add(TokenType.COMMA, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "+":
                self.add(TokenType.PLUS, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "=":
                if self.i + 1 < self.n and self.src[self.i + 1] == "=":
                    self.add(TokenType.OP, "==", self.line, self.col)
                    self.advance()
                    self.advance()
                else:
                    self.add(TokenType.ASSIGN, ch, self.line, self.col)
                    self.advance()
                self.prev_can_end = False
                continue
            if ch in "-*/%<>!&|:.":
                self.add(TokenType.OP, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            if ch.isalpha() or ch == "_":
                self.lex_ident()
                continue
            if ch.isdigit():
                self.lex_number()
                continue
            raise SarmaError(f"Unexpected character {ch!r} at {self.line}:{self.col}")
        self.tokens.append(Token(TokenType.EOF, "", self.line, self.col))
        return self.tokens

    def lex_ident(self):
        line, col = self.line, self.col
        start = self.i
        while self.i < self.n and (self.src[self.i].isalnum() or self.src[self.i] == "_"):
            self.advance()
        word = self.src[start:self.i]
        if word == "print":
            self.add(TokenType.PRINT, word, line, col)
        elif word == "var":
            self.add(TokenType.VAR, word, line, col)
        else:
            self.add(TokenType.IDENTIFIER, word, line, col)
        self.prev_can_end = True

    def lex_number(self):
        line, col = self.line, self.col
        start = self.i
        while self.i < self.n and (self.src[self.i].isdigit() or self.src[self.i] == "."):
            self.advance()
        self.add(TokenType.NUMBER, self.src[start:self.i], line, col)
        self.prev_can_end = True

    def lex_string(self):
        quote = self.advance()  # consume opening quote
        line, col = self.line, self.col
        buf: list[str] = []
        closed = False
        while self.i < self.n:
            ch = self.advance()
            if ch == "\\" and self.i < self.n:
                esc_line, esc_col = self.line, self.col  # position of escaped char
                esc = self.advance()
                if esc == "\n":
                    continue  # line continuation: skip both, value gets no newline
                if esc == "\r":
                    if self.i < self.n and self.peek() == "\n":
                        self.advance()
                    continue  # line continuation for \r / \r\n
                mapping = {"n": "\n", "t": "\t", "r": "\r",
                           "\\": "\\", "'": "'", '"': '"', "0": "\0"}
                if esc not in mapping:
                    raise SarmaError(f"Unknown escape sequence '\\{esc}' at {esc_line}:{esc_col}")
                buf.append(mapping[esc])
                continue
            if ch == quote:
                closed = True
                break
            if ch == "\n":
                raise SarmaError(f"Unterminated string starting at {line}:{col}")
            buf.append(ch)
        if not closed:
            raise SarmaError(f"Unterminated string starting at {line}:{col}")
        self.add(TokenType.STRING, "".join(buf), line, col)
        self.prev_can_end = True


@dataclass
class StringLit:
    value: str


@dataclass
class VarRef:
    name: str
    line: int


@dataclass
class Concat:
    parts: list


@dataclass
class PrintStmt:
    expr: object
    line: int


@dataclass
class VarStmt:
    name: str
    expr: object
    line: int


@dataclass
class AssignStmt:
    name: str
    expr: object
    line: int


class Parser:
    def __init__(self, tokens: list[Token]):
        self.tokens = tokens
        self.i = 0

    def peek(self) -> Token:
        return self.tokens[self.i]

    def advance(self) -> Token:
        tok = self.tokens[self.i]
        self.i += 1
        return tok

    def expect(self, type: TokenType) -> Token:
        tok = self.peek()
        if tok.type != type:
            raise SarmaError(
                f"Expected {type.name} but got {tok.type.name} ({tok.value!r}) at {tok.line}:{tok.col}"
            )
        return self.advance()

    def parse(self) -> list:
        stmts: list = []
        while self.peek().type != TokenType.EOF:
            # skip blank / duplicate terminators
            while self.peek().type == TokenType.TERMINATOR:
                self.advance()
            if self.peek().type == TokenType.EOF:
                break
            stmts.append(self.parse_statement())
            # each statement must end with TERMINATOR or EOF
            nxt = self.peek()
            if nxt.type == TokenType.TERMINATOR:
                self.advance()
            elif nxt.type == TokenType.EOF:
                break
            else:
                raise SarmaError(
                    f"Expected end of statement (newline or ';') at {nxt.line}:{nxt.col}, got {nxt.type.name}"
                )
        return stmts

    def parse_statement(self):
        tok = self.peek()
        if tok.type == TokenType.PRINT:
            return self.parse_print()
        if tok.type == TokenType.VAR:
            return self.parse_var()
        if tok.type == TokenType.IDENTIFIER:
            return self.parse_assign()
        raise SarmaError(
            f"Unknown statement starting with {tok.type.name} ({tok.value!r}) at {tok.line}:{tok.col}. "
            f"Expected print, var, or assignment."
        )

    def parse_var(self) -> VarStmt:
        kw = self.expect(TokenType.VAR)
        name = self.expect(TokenType.IDENTIFIER)
        self.expect(TokenType.ASSIGN)
        expr = self.parse_expr()
        return VarStmt(name=name.value, expr=expr, line=kw.line)

    def parse_assign(self) -> AssignStmt:
        name = self.expect(TokenType.IDENTIFIER)
        self.expect(TokenType.ASSIGN)
        expr = self.parse_expr()
        return AssignStmt(name=name.value, expr=expr, line=name.line)

    def parse_print(self) -> PrintStmt:
        kw = self.expect(TokenType.PRINT)
        self.expect(TokenType.LPAREN)
        expr = self.parse_expr()
        self.expect(TokenType.RPAREN)
        return PrintStmt(expr=expr, line=kw.line)

    def parse_expr(self):
        parts = [self.parse_primary()]
        while self.peek().type == TokenType.PLUS:
            self.advance()
            parts.append(self.parse_primary())
        if len(parts) == 1:
            return parts[0]
        return Concat(parts=parts)

    def parse_primary(self):
        tok = self.peek()
        if tok.type == TokenType.STRING:
            self.advance()
            return StringLit(value=tok.value)
        if tok.type == TokenType.IDENTIFIER:
            self.advance()
            return VarRef(name=tok.value, line=tok.line)
        if tok.type == TokenType.LPAREN:
            self.advance()
            expr = self.parse_expr()
            self.expect(TokenType.RPAREN)
            return expr
        raise SarmaError(
            f"Expected string, variable, or '(' but got {tok.type.name} ({tok.value!r}) at {tok.line}:{tok.col}"
        )


def eval_expr(node, env: dict) -> str:
    try:
        if isinstance(node, StringLit):
            return node.value
        if isinstance(node, VarRef):
            if node.name not in env:
                raise SarmaError(f"Undefined variable {node.name!r} at line {node.line}")
            return env[node.name]
        if isinstance(node, Concat):
            return "".join(eval_expr(p, env) for p in node.parts)
        raise SarmaError(f"Unknown expression {node!r}")
    except RecursionError:
        raise SarmaError("expression too deeply nested")


def execute(stmts: list, out=print) -> None:
    env: dict = {}
    for s in stmts:
        if isinstance(s, PrintStmt):
            out(eval_expr(s.expr, env))
        elif isinstance(s, VarStmt):
            if s.name in env:
                raise SarmaError(f"Variable {s.name!r} already defined (line {s.line})")
            env[s.name] = eval_expr(s.expr, env)
        elif isinstance(s, AssignStmt):
            if s.name not in env:
                raise SarmaError(f"Undefined variable {s.name!r} at line {s.line} (declare with var first)")
            env[s.name] = eval_expr(s.expr, env)
        else:
            raise SarmaError(f"Unknown statement {s!r}")


def run_source(source: str) -> list:
    try:
        tokens = Lexer(source).lex()
        return Parser(tokens).parse()
    except RecursionError:
        raise SarmaError("expression too deeply nested")


def run_and_capture(source: str) -> list[str]:
    lines: list[str] = []
    execute(run_source(source), out=lines.append)
    return lines


def run_file(path: str):
    try:
        with open(path, "r", encoding="utf-8") as f:
            source = f.read()
    except (OSError, UnicodeDecodeError) as e:
        print(f"sarma: cannot read file {path}: {e}", file=sys.stderr)
        sys.exit(1)
    try:
        stmts = run_source(source)
        execute(stmts)
    except SarmaError as e:
        print(f"sarma: error in {path}: {e}", file=sys.stderr)
        sys.exit(1)


def main(argv: list[str]):
    if len(argv) != 2:
        print(f"Usage: {argv[0]} <file.sarma>", file=sys.stderr)
        sys.exit(2)
    run_file(argv[1])


if __name__ == "__main__":
    main(sys.argv)
