#!/usr/bin/env python3
"""Sarma test runner (standard library only).

Usage:
    python3 tests/run_tests.py            # run everything
    python3 tests/run_tests.py -v         # also show passing tests
    python3 tests/run_tests.py escapes    # only tests whose name contains "escapes"

Three kinds of tests:

1. File cases: examples/*.sarma and tests/cases/*.sarma, run through the real
   CLI. Each needs a NAME.out file (expected stdout). If NAME.err exists the
   program must fail with exit code 1 and exactly that error message;
   otherwise it must succeed with exit code 0 and empty stderr.
2. In-process tests: call the lexer/parser/interpreter directly.
3. CLI tests: behaviours that need a temporary file (bad encoding, usage...).

Exit code is 0 if everything passed, 1 otherwise.
"""

import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

import sarma  # noqa: E402
from sarma import Lexer, SarmaError, TokenType, execute, run_and_capture, run_source  # noqa: E402

SARMA_PY = os.path.join(ROOT, "sarma.py")


def run_cli(*args):
    return subprocess.run(
        [sys.executable, SARMA_PY, *args],
        cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
    )


def read(path):
    with open(path, encoding="utf-8", newline="") as f:
        return f.read()


# ---------------------------------------------------------------- file cases

def file_cases():
    for d in ("examples", os.path.join("tests", "cases")):
        for name in sorted(os.listdir(os.path.join(ROOT, d))):
            if name.endswith(".sarma"):
                yield os.path.join(d, name)


def check_file_case(rel):
    """Return a list of problems (empty list = pass)."""
    base = os.path.join(ROOT, rel[: -len(".sarma")])
    if not os.path.exists(base + ".out"):
        return [f"missing {os.path.basename(base)}.out"]
    problems = []
    r = run_cli(rel)
    if r.stdout != read(base + ".out"):
        problems.append(f"stdout mismatch:\n    expected {read(base + '.out')!r}\n    got      {r.stdout!r}")
    if os.path.exists(base + ".err"):
        expected = f"sarma: error in {rel}: {read(base + '.err').strip()}"
        if r.returncode != 1:
            problems.append(f"expected exit code 1, got {r.returncode}")
        if r.stderr.strip() != expected:
            problems.append(f"stderr mismatch:\n    expected {expected!r}\n    got      {r.stderr.strip()!r}")
    else:
        if r.returncode != 0:
            problems.append(f"expected exit code 0, got {r.returncode}")
        if r.stderr:
            problems.append(f"unexpected stderr: {r.stderr!r}")
    return problems


# ------------------------------------------------------- in-process tests

TESTS = []


def test(fn):
    TESTS.append((fn.__name__, fn))
    return fn


@test
def test_output_streams_before_error():
    lines = []
    err = None
    try:
        execute(run_source('print("before")\nprint(missing)\n'), out=lines.append)
    except SarmaError as e:
        err = e
    assert lines == ["before"], lines
    assert err is not None and "missing" in str(err)


@test
def test_execute_returns_none():
    assert execute(run_source('print("x")\n'), out=lambda line: None) is None


@test
def test_run_and_capture_success():
    assert run_and_capture('var a = "x"\nprint(a + "y")\n') == ["xy"]


@test
def test_crlf_source():
    assert run_and_capture('print("a")\r\nprint("b")\r\n') == ["a", "b"]


@test
def test_all_escapes():
    assert run_and_capture(r'print("a\n\t\r\\\'\"\0b")') == ["a\n\t\r\\'\"\0b"]


@test
def test_two_char_operators_are_single_tokens():
    for op in ("==", "!=", "<=", ">=", "&&", "||", "->"):
        for src in (f"a {op} b", f"a{op}b"):
            toks = Lexer(src).lex()
            kinds = [(t.type, t.value) for t in toks[:-1]]
            assert kinds == [
                (TokenType.IDENTIFIER, "a"), (TokenType.OP, op), (TokenType.IDENTIFIER, "b"),
            ], (src, kinds)


@test
def test_single_equals_and_single_char_ops():
    toks = Lexer("a = b < c").lex()
    assert [(t.type, t.value) for t in toks[:-1]] == [
        (TokenType.IDENTIFIER, "a"), (TokenType.ASSIGN, "="), (TokenType.IDENTIFIER, "b"),
        (TokenType.OP, "<"), (TokenType.IDENTIFIER, "c"),
    ]


@test
def test_string_token_points_at_opening_quote():
    toks = Lexer('print("abc")').lex()
    string_tok = next(t for t in toks if t.type == TokenType.STRING)
    assert (string_tok.line, string_tok.col) == (1, 7), string_tok


@test
def test_long_concat_chain():
    n = 5000
    src = "print(" + " + ".join(['"a"'] * n) + ")\n"
    assert run_and_capture(src) == ["a" * n]


@test
def test_nesting_limit():
    def nested(n):
        return "print(" + "(" * n + '"a"' + ")" * n + ")\n"

    assert run_and_capture(nested(sarma.MAX_NESTING)) == ["a"]
    for n in (sarma.MAX_NESTING + 1, 1000):
        try:
            run_and_capture(nested(n))
        except SarmaError as e:
            assert "nested too deeply" in str(e), e
        else:
            raise AssertionError(f"depth {n} should have failed")


@test
def test_nesting_counter_resets_between_expressions():
    src = "".join('print(("a"))\n' for _ in range(sarma.MAX_NESTING * 2))
    assert run_and_capture(src) == ["a"] * (sarma.MAX_NESTING * 2)


# ------------------------------------------------------------- CLI tests

@test
def test_cli_missing_file():
    r = run_cli("no_such_file.sarma")
    assert r.returncode == 1, r.returncode
    assert r.stderr.startswith("sarma: cannot read no_such_file.sarma"), r.stderr
    assert "Traceback" not in r.stderr, r.stderr


@test
def test_cli_directory_instead_of_file():
    with tempfile.TemporaryDirectory() as d:
        r = run_cli(d)
    assert r.returncode == 1 and r.stderr.startswith("sarma: cannot read"), (r.returncode, r.stderr)
    assert "Traceback" not in r.stderr


@test
def test_cli_invalid_utf8():
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "bad.sarma")
        with open(path, "wb") as f:
            f.write(b'print("\xff")')
        r = run_cli(path)
    assert r.returncode == 1, r.returncode
    assert "not valid UTF-8" in r.stderr and "Traceback" not in r.stderr, r.stderr


@test
def test_cli_usage_error():
    r = run_cli()
    assert r.returncode == 2 and "Usage" in r.stderr, (r.returncode, r.stderr)


@test
def test_cli_deep_nesting_is_clean_error():
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "deep.sarma")
        with open(path, "w", encoding="utf-8") as f:
            f.write("print(" + "(" * 500 + '"a"' + ")" * 500 + ")\n")
        r = run_cli(path)
    assert r.returncode == 1 and "nested too deeply" in r.stderr, (r.returncode, r.stderr[-200:])
    assert "Traceback" not in r.stderr


@test
def test_cli_broken_pipe_is_quiet():
    # Enough output (~1.2 MB) that the interpreter is still writing when the reader leaves.
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "many.sarma")
        with open(path, "w", encoding="utf-8") as f:
            f.write(('print("%s")\n' % ("x" * 60)) * 20000)
        p = subprocess.Popen([sys.executable, SARMA_PY, path],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        first = p.stdout.readline()
        p.stdout.close()
        err = p.stderr.read()
        p.wait()
        p.stderr.close()
    assert first == "x" * 60 + "\n", first
    assert "Traceback" not in err and "BrokenPipe" not in err, err[-300:]


@test
def test_cli_long_concat_chain():
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "long.sarma")
        with open(path, "w", encoding="utf-8") as f:
            f.write("print(" + " + ".join(['"a"'] * 5000) + ")\n")
        r = run_cli(path)
    assert r.returncode == 0 and r.stdout == "a" * 5000 + "\n", (r.returncode, r.stderr[-200:])


# ----------------------------------------------------------------- runner

def main(argv):
    verbose = "-v" in argv
    filters = [a for a in argv if not a.startswith("-")]

    def selected(name):
        return not filters or any(f in name for f in filters)

    results = []  # (name, problems)

    for rel in file_cases():
        if selected(rel):
            results.append((rel, check_file_case(rel)))

    for name, fn in TESTS:
        if selected(name):
            try:
                fn()
                results.append((name, []))
            except AssertionError as e:
                results.append((name, [f"assertion failed: {e}"]))
            except Exception as e:  # a crash is a failure too
                results.append((name, [f"{type(e).__name__}: {e}"]))

    failed = 0
    for name, problems in results:
        if problems:
            failed += 1
            print(f"FAIL  {name}")
            for p in problems:
                print(f"      {p}")
        elif verbose:
            print(f"ok    {name}")

    total = len(results)
    print(f"\n{total - failed}/{total} passed" + (f", {failed} FAILED" if failed else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
