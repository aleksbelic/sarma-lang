# Sarma Programming Language

_Requires Python 3.9 or newer. The language is described in [SPECS.md](SPECS.md)._

## Python interpreter

The reference implementation lives in `python/` (standard library only, no dependencies):

```bash
python3 python/sarma.py examples/print.sarma
```

or
```bash
./python/sarma ./examples/print.sarma
```

This is the default interpreter used by `python3 tests/run_tests.py` (see Tests below).

## Rust interpreter

A faithful port lives in `rust/` (no dependencies, edition 2021). It passes
the same suite, including byte-exact error messages:

```bash
cargo build --manifest-path rust/Cargo.toml
./rust/target/debug/sarma examples/print.sarma
python3 tests/run_tests.py --bin rust/target/debug/sarma
```

Known divergences from the Python version (all outside the test suite):
integer literals and arithmetic are 64-bit — absurd magnitudes fail with a
clean `Integer overflow` instead of computing big integers; astronomically
large floats may format differently (`1e+21` vs full digits).

## Tests

```bash
python3 tests/run_tests.py
```

or

```bash
python3 tests/run_tests.py -v        # add -v to list passing tests, or a name to filter
```

No Python? The Rust crate has its own suite with the same oracle
(`cargo test` runs it, no Python needed):

```bash
cargo test --manifest-path rust/Cargo.toml
```

Each `examples/*.sarma` and `tests/cases/*.sarma` has a `.out` file with its expected stdout.
If a `.err` file exists, the program must fail with exactly that error message.

