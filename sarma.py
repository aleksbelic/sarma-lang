#!/usr/bin/env python3
"""Sarma programming language - interpreter (print, variables, strings, numbers,
strict comparisons).

Usage:
    python3 sarma.py <file.sarma>
    python3 sarma.py examples/print.sarma

Syntax:
    var name = "world"
    name = "sarma"
    print("hi " + name)
    print(1 + 2 * 3)     # numbers: 7
    print(1 == 1)        # true (strict equality: 1 == "1" is false)
    if (1 < 2) {         # braces required; else optional
        print("yes")
    }
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

import os
import sys
from dataclasses import dataclass
from enum import Enum, auto


class TokenType(Enum):
    PRINT = auto()
    VAR = auto()
    IF = auto()
    ELSE = auto()
    TRUE = auto()
    FALSE = auto()
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


# Closing bracket for each opening bracket.
CLOSING = {"(": ")", "[": "]", "{": "}"}
# Supported string escapes; any other backslash sequence is an error.
ESCAPES = {"n": "\n", "t": "\t", "r": "\r",
           "\\": "\\", "'": "'", '"': '"', "0": "\0"}
# Two-character operators are lexed as a single OP token.
TWO_CHAR_OPS = ("==", "!=", "<=", ">=", "&&", "||", "->")
# Maximum depth of nested parentheses inside an expression.
MAX_NESTING = 100


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
        self.brackets: list[tuple[str, int, int]] = []  # open (), [], {}: (char, line, col)

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

    def close_bracket(self, ch: str):
        line, col = self.line, self.col
        if not self.brackets:
            raise SarmaError(f"Unmatched {ch!r} at {line}:{col}")
        open_ch, open_line, open_col = self.brackets[-1]
        if CLOSING[open_ch] != ch:
            raise SarmaError(
                f"Mismatched {ch!r} at {line}:{col}: expected {CLOSING[open_ch]!r} "
                f"to close {open_ch!r} opened at {open_line}:{open_col}"
            )
        self.brackets.pop()

    def lex(self) -> list[Token]:
        while self.i < self.n:
            ch = self.peek()
            if ch in (" ", "\t", "\r"):
                self.advance()
                continue
            if ch == "\n":
                line, col = self.line, self.col
                self.advance()
                # Suppress newline-terminators inside (...) and [...] so
                #   print(
                #     "hi"
                #   )
                # works naturally. Inside {...} blocks newlines DO terminate,
                # since blocks hold statements, not expressions.
                if (not self.brackets or self.brackets[-1][0] == "{") and self.prev_can_end:
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
                self.brackets.append((ch, self.line, self.col))
                self.advance()
                self.prev_can_end = False
                continue
            if ch == ")":
                self.close_bracket(ch)
                self.add(TokenType.RPAREN, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = True
                continue
            if ch == "[":
                self.add(TokenType.LBRACK, ch, self.line, self.col)
                self.brackets.append((ch, self.line, self.col))
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "]":
                self.close_bracket(ch)
                self.add(TokenType.RBRACK, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = True
                continue
            if ch == "{":
                self.add(TokenType.LBRACE, ch, self.line, self.col)
                self.brackets.append((ch, self.line, self.col))
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "}":
                self.close_bracket(ch)
                self.add(TokenType.RBRACE, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = True
                continue
            if ch == ",":
                self.add(TokenType.COMMA, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            two = self.src[self.i:self.i + 2]
            if two in TWO_CHAR_OPS:
                self.add(TokenType.OP, two, self.line, self.col)
                self.advance()
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "+":
                self.add(TokenType.PLUS, ch, self.line, self.col)
                self.advance()
                self.prev_can_end = False
                continue
            if ch == "=":
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
        if self.brackets:
            ch, line, col = self.brackets[-1]
            raise SarmaError(f"Unclosed {ch!r} opened at {line}:{col}")
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
        elif word == "if":
            self.add(TokenType.IF, word, line, col)
        elif word == "else":
            self.add(TokenType.ELSE, word, line, col)
        elif word == "true":
            self.add(TokenType.TRUE, word, line, col)
        elif word == "false":
            self.add(TokenType.FALSE, word, line, col)
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
        line, col = self.line, self.col  # position of the opening quote
        quote = self.advance()  # consume opening quote
        buf: list[str] = []
        closed = False
        while self.i < self.n:
            esc_line, esc_col = self.line, self.col
            ch = self.advance()
            if ch == "\\" and self.i < self.n:
                esc = self.advance()
                if esc in ("\n", "\r"):
                    raise SarmaError(
                        f"Strings cannot span lines (backslash at end of line at {esc_line}:{esc_col})"
                    )
                if esc not in ESCAPES:
                    raise SarmaError(f"Unknown escape sequence '\\{esc}' at {esc_line}:{esc_col}")
                buf.append(ESCAPES[esc])
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
class NumberLit:
    value: object  # int or float
    line: int


@dataclass
class BoolLit:
    value: bool
    line: int


@dataclass
class VarRef:
    name: str
    line: int


@dataclass
class Add:
    """Flat '+' / '-' chain (left-associative). Operands all strings (+) or all numbers."""
    parts: list
    ops: list  # each "+" or "-", len == len(parts) - 1
    line: int  # line of the first operator


@dataclass
class Mul:
    """Flat '*', '/', '%' chain (left-associative, numbers only)."""
    parts: list
    ops: list  # each "*", "/" or "%", len == len(parts) - 1
    line: int  # line of the first operator


@dataclass
class Neg:
    """Unary minus (numbers only)."""
    operand: object
    line: int


@dataclass
class Not:
    """Logical NOT; result is always a boolean (JavaScript truthiness)."""
    operand: object
    line: int


@dataclass
class Eq:
    """Flat '==' / '!=' chain, left-associative (strict: mixed families never equal)."""
    parts: list
    ops: list  # each "==" or "!=", len == len(parts) - 1
    line: int


@dataclass
class Rel:
    """Flat '<', '<=', '>', '>=' chain, left-associative (numbers or two strings)."""
    parts: list
    ops: list  # each "<", "<=", ">" or ">=", len == len(parts) - 1
    line: int


@dataclass
class And:
    """'&&' chain; value is the first falsy operand or the last one (short-circuits)."""
    parts: list


@dataclass
class Or:
    """'||' chain; value is the first truthy operand or the last one (short-circuits)."""
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


@dataclass
class Block:
    stmts: list
    line: int


@dataclass
class IfStmt:
    cond: object
    then: object  # Block or single statement
    otherwise: object  # Block, single statement, IfStmt (else-if), or None
    line: int


class Parser:
    def __init__(self, tokens: list[Token]):
        self.tokens = tokens
        self.i = 0
        self.nesting = 0  # parenthesis depth inside the current expression

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
        while True:
            self.skip_terminators()
            if self.peek().type == TokenType.EOF:
                break
            stmts.append(self.parse_statement())
            self.finish_statement()
        return stmts

    def skip_terminators(self):
        while self.peek().type == TokenType.TERMINATOR:
            self.advance()

    def finish_statement(self):
        # Each statement ends with TERMINATOR or EOF; a closing brace ends one too,
        # so single-line blocks like `{ print("x") }` need no `;`.
        nxt = self.peek()
        if nxt.type == TokenType.TERMINATOR:
            self.advance()
        elif nxt.type in (TokenType.RBRACE, TokenType.EOF):
            pass
        else:
            raise SarmaError(
                f"Expected end of statement (newline or ';') at {nxt.line}:{nxt.col}, got {nxt.type.name}"
            )

    def parse_statement(self):
        tok = self.peek()
        if tok.type == TokenType.PRINT:
            return self.parse_print()
        if tok.type == TokenType.VAR:
            return self.parse_var()
        if tok.type == TokenType.IF:
            return self.parse_if()
        if tok.type == TokenType.IDENTIFIER:
            return self.parse_assign()
        raise SarmaError(
            f"Unknown statement starting with {tok.type.name} ({tok.value!r}) at {tok.line}:{tok.col}. "
            f"Expected print, var, if, or assignment."
        )

    def parse_if(self) -> IfStmt:
        kw = self.expect(TokenType.IF)
        self.expect(TokenType.LPAREN)
        cond = self.parse_expr()
        self.expect(TokenType.RPAREN)
        # Newlines may surround the branch, so both K&R and Allman styles work,
        # `else` may start on the line after the branch, and a bare
        # `if (cond) print("x")` needs no braces.
        self.skip_terminators()
        then = self.parse_branch()
        # Look for `else` past any newlines, but restore them if absent:
        # they still separate this statement from the next one.
        mark = self.i
        self.skip_terminators()
        otherwise = None
        if self.peek().type == TokenType.ELSE:
            self.advance()
            self.skip_terminators()
            if self.peek().type == TokenType.IF:
                otherwise = self.parse_if()
            else:
                otherwise = self.parse_branch()
        else:
            self.i = mark
        return IfStmt(cond=cond, then=then, otherwise=otherwise, line=kw.line)

    def parse_branch(self):
        # A branch is a braced block or a single bare statement.
        # A nested `if` as a bare branch binds a following `else` to itself
        # (dangling else goes to the nearest `if`).
        if self.peek().type == TokenType.LBRACE:
            return self.parse_block()
        return self.parse_statement()

    def parse_block(self) -> Block:
        tok = self.expect(TokenType.LBRACE)
        stmts: list = []
        while True:
            self.skip_terminators()
            if self.peek().type == TokenType.RBRACE:
                self.advance()
                break
            stmts.append(self.parse_statement())
            self.finish_statement()
        return Block(stmts=stmts, line=tok.line)

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
        # Logical-or level (lowest precedence).
        parts = [self.parse_and()]
        while self.peek().type == TokenType.OP and self.peek().value == "||":
            self.advance()
            parts.append(self.parse_and())
        if len(parts) == 1:
            return parts[0]
        return Or(parts=parts)

    def parse_and(self):
        # Logical-and level.
        parts = [self.parse_equality()]
        while self.peek().type == TokenType.OP and self.peek().value == "&&":
            self.advance()
            parts.append(self.parse_equality())
        if len(parts) == 1:
            return parts[0]
        return And(parts=parts)

    def parse_equality(self):
        # '==' / '!=' level.
        parts = [self.parse_relational()]
        ops: list[str] = []
        op_line = 0
        while self.peek().type == TokenType.OP and self.peek().value in ("==", "!="):
            tok = self.advance()
            if not ops:
                op_line = tok.line
            ops.append(tok.value)
            parts.append(self.parse_relational())
        if not ops:
            return parts[0]
        return Eq(parts=parts, ops=ops, line=op_line)

    def parse_relational(self):
        # '<', '<=', '>', '>=' level.
        parts = [self.parse_additive()]
        ops: list[str] = []
        op_line = 0
        while self.peek().type == TokenType.OP and self.peek().value in ("<", "<=", ">", ">="):
            tok = self.advance()
            if not ops:
                op_line = tok.line
            ops.append(tok.value)
            parts.append(self.parse_additive())
        if not ops:
            return parts[0]
        return Rel(parts=parts, ops=ops, line=op_line)

    def parse_additive(self):
        # Additive level: '+' and '-' (left-associative).
        parts = [self.parse_term()]
        ops: list[str] = []
        op_line = 0
        while True:
            tok = self.peek()
            if tok.type == TokenType.PLUS:
                op = "+"
            elif tok.type == TokenType.OP and tok.value == "-":
                op = "-"
            else:
                break
            self.advance()
            if not ops:
                op_line = tok.line
            ops.append(op)
            parts.append(self.parse_term())
        if not ops:
            return parts[0]
        return Add(parts=parts, ops=ops, line=op_line)

    def parse_term(self):
        # Multiplicative level: '*', '/', '%' (left-associative, tighter than '+'/'-').
        parts = [self.parse_primary()]
        ops: list[str] = []
        op_line = 0
        while self.peek().type == TokenType.OP and self.peek().value in ("*", "/", "%"):
            tok = self.advance()
            if not ops:
                op_line = tok.line
            ops.append(tok.value)
            parts.append(self.parse_primary())
        if not ops:
            return parts[0]
        return Mul(parts=parts, ops=ops, line=op_line)

    def parse_primary(self):
        tok = self.peek()
        if tok.type == TokenType.STRING:
            self.advance()
            return StringLit(value=tok.value)
        if tok.type == TokenType.NUMBER:
            self.advance()
            return make_number(tok)
        if tok.type == TokenType.TRUE:
            self.advance()
            return BoolLit(value=True, line=tok.line)
        if tok.type == TokenType.FALSE:
            self.advance()
            return BoolLit(value=False, line=tok.line)
        if tok.type == TokenType.IDENTIFIER:
            self.advance()
            return VarRef(name=tok.value, line=tok.line)
        if tok.type == TokenType.OP and tok.value == "-":
            self.advance()
            return Neg(operand=self.parse_primary(), line=tok.line)
        if tok.type == TokenType.OP and tok.value == "!":
            self.advance()
            return Not(operand=self.parse_primary(), line=tok.line)
        if tok.type == TokenType.LPAREN:
            self.advance()
            self.nesting += 1
            if self.nesting > MAX_NESTING:
                raise SarmaError(
                    f"Expression nested too deeply (more than {MAX_NESTING} levels) at {tok.line}:{tok.col}"
                )
            expr = self.parse_expr()
            self.expect(TokenType.RPAREN)
            self.nesting -= 1
            return expr
        raise SarmaError(
            f"Expected string, number, boolean, variable, or '(' but got {tok.type.name} ({tok.value!r}) at {tok.line}:{tok.col}"
        )


def make_number(tok: Token) -> NumberLit:
    s = tok.value
    try:
        value = int(s) if "." not in s else float(s)
    except ValueError:
        raise SarmaError(f"Invalid number {s!r} at {tok.line}:{tok.col}")
    return NumberLit(value=value, line=tok.line)


def display(value) -> str:
    """How a runtime value prints: booleans as true/false, strings as-is,
    whole floats without decimals."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, float) and value.is_integer():
        return str(int(value))
    return str(value)


def truthy(value) -> bool:
    """JavaScript truthiness: false, 0, and "" are falsy; everything else is truthy."""
    if isinstance(value, bool):
        return value
    if isinstance(value, (int, float)):
        return value != 0
    return value != ""


def same_family(a, b) -> bool:
    """Strict comparison families: numbers (int and float together), strings, booleans."""
    if isinstance(a, bool) or isinstance(b, bool):
        return isinstance(a, bool) and isinstance(b, bool)
    if isinstance(a, str) or isinstance(b, str):
        return isinstance(a, str) and isinstance(b, str)
    return True  # two numbers


def strict_eq(a, b) -> bool:
    """Equality: same family compares by value, mixed families are never equal."""
    if not same_family(a, b):
        return False
    return a == b


def strict_rel(op: str, a, b, line: int) -> bool:
    """Relational: two numbers compare numerically, two strings lexicographically;
    anything else is a type error."""
    if isinstance(a, str) and isinstance(b, str):
        if op == "<":
            return a < b
        if op == "<=":
            return a <= b
        if op == ">":
            return a > b
        return a >= b
    a_num = isinstance(a, (int, float)) and not isinstance(a, bool)
    b_num = isinstance(b, (int, float)) and not isinstance(b, bool)
    if a_num and b_num:
        if op == "<":
            return a < b
        if op == "<=":
            return a <= b
        if op == ">":
            return a > b
        return a >= b
    raise SarmaError(f"Operator '{op}' requires two numbers or two strings at line {line}")


def eval_expr(node, env: dict):
    """Evaluate to a runtime value: str, int, float, or bool."""
    try:
        if isinstance(node, StringLit):
            return node.value
        if isinstance(node, NumberLit):
            return node.value
        if isinstance(node, BoolLit):
            return node.value
        if isinstance(node, VarRef):
            if node.name not in env:
                raise SarmaError(f"Undefined variable {node.name!r} at line {node.line}")
            return env[node.name]
        if isinstance(node, Neg):
            value = eval_expr(node.operand, env)
            if isinstance(value, bool):
                raise SarmaError(f"Operator '-' does not accept booleans at line {node.line}")
            if isinstance(value, str):
                raise SarmaError(f"Cannot negate a string at line {node.line}")
            return -value
        if isinstance(node, Not):
            return not truthy(eval_expr(node.operand, env))
        if isinstance(node, Add):
            values = [eval_expr(p, env) for p in node.parts]
            if all(isinstance(v, str) for v in values):
                if any(op == "-" for op in node.ops):
                    raise SarmaError(f"Operator '-' requires numbers at line {node.line}")
                return "".join(values)
            if any(isinstance(v, bool) for v in values):
                raise SarmaError(f"Operator '{node.ops[0]}' does not accept booleans at line {node.line}")
            if any(isinstance(v, str) for v in values):
                raise SarmaError(f"Cannot mix strings and numbers with '+' at line {node.line}")
            result = values[0]
            for op, v in zip(node.ops, values[1:]):
                result = result + v if op == "+" else result - v
            return result
        if isinstance(node, Mul):
            values = [eval_expr(p, env) for p in node.parts]
            if any(isinstance(v, bool) for v in values):
                raise SarmaError(f"Operator '{node.ops[0]}' does not accept booleans at line {node.line}")
            if any(isinstance(v, str) for v in values):
                raise SarmaError(f"Operator '{node.ops[0]}' requires numbers at line {node.line}")
            result = values[0]
            try:
                for op, v in zip(node.ops, values[1:]):
                    if op == "*":
                        result *= v
                    elif op == "/":
                        result /= v
                    else:
                        result %= v
            except ZeroDivisionError:
                raise SarmaError(f"Division by zero at line {node.line}")
            return result
        if isinstance(node, Eq):
            result = strict_eq(eval_expr(node.parts[0], env), eval_expr(node.parts[1], env))
            for op, part in zip(node.ops[1:], node.parts[2:]):
                current = strict_eq(result, eval_expr(part, env))
                result = current if op == "==" else not current
            if node.ops[0] == "!=":
                result = not result
            return result
        if isinstance(node, Rel):
            result = strict_rel(node.ops[0], eval_expr(node.parts[0], env), eval_expr(node.parts[1], env), node.line)
            for op, part in zip(node.ops[1:], node.parts[2:]):
                result = strict_rel(op, result, eval_expr(part, env), node.line)
            return result
        if isinstance(node, And):
            result = True
            for p in node.parts:
                result = eval_expr(p, env)
                if not truthy(result):
                    return result
            return result
        if isinstance(node, Or):
            result = False
            for p in node.parts:
                result = eval_expr(p, env)
                if truthy(result):
                    return result
            return result
        raise SarmaError(f"Unknown expression {node!r}")
    except RecursionError:
        raise SarmaError("expression too deeply nested")


def execute(stmts: list, out=print) -> None:
    """Run statements, calling out(line) for each print as it happens."""
    exec_block(stmts, {}, out)


def exec_block(stmts: list, env: dict, out) -> None:
    for s in stmts:
        exec_stmt(s, env, out)


def exec_stmt(s, env: dict, out) -> None:
    if isinstance(s, PrintStmt):
        out(display(eval_expr(s.expr, env)))
    elif isinstance(s, VarStmt):
        if s.name in env:
            raise SarmaError(f"Variable {s.name!r} already defined (line {s.line})")
        env[s.name] = eval_expr(s.expr, env)
    elif isinstance(s, AssignStmt):
        if s.name not in env:
            raise SarmaError(f"Undefined variable {s.name!r} at line {s.line} (declare with var first)")
        env[s.name] = eval_expr(s.expr, env)
    elif isinstance(s, IfStmt):
        branch = s.then if truthy(eval_expr(s.cond, env)) else s.otherwise
        if branch is None:
            return
        if isinstance(branch, Block):
            exec_block(branch.stmts, env, out)
        else:
            exec_stmt(branch, env, out)  # else-if chain
    else:
        raise SarmaError(f"Unknown statement {s!r}")


def run_source(source: str) -> list:
    try:
        tokens = Lexer(source).lex()
        return Parser(tokens).parse()
    except RecursionError:
        raise SarmaError("expression too deeply nested")


def run_and_capture(source: str) -> list[str]:
    """Run a program and return its output lines (raises on error, losing them)."""
    lines: list[str] = []
    execute(run_source(source), out=lines.append)
    return lines


def run_file(path: str):
    try:
        with open(path, "r", encoding="utf-8") as f:
            source = f.read()
    except OSError as e:
        print(f"sarma: cannot read {path}: {e.strerror or e}", file=sys.stderr)
        sys.exit(1)
    except UnicodeDecodeError:
        print(f"sarma: cannot read {path}: file is not valid UTF-8", file=sys.stderr)
        sys.exit(1)
    try:
        execute(run_source(source))
    except SarmaError as e:
        print(f"sarma: error in {path}: {e}", file=sys.stderr)
        sys.exit(1)
    except RecursionError:
        print(f"sarma: error in {path}: program is nested too deeply", file=sys.stderr)
        sys.exit(1)
    except BrokenPipeError:
        # The reader went away (e.g. `sarma prog.sarma | head`): exit quietly.
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(1)


def main(argv: list[str]):
    if len(argv) != 2:
        print(f"Usage: {argv[0]} <file.sarma>", file=sys.stderr)
        sys.exit(2)
    run_file(argv[1])


if __name__ == "__main__":
    main(sys.argv)
