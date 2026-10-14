# Sarma Programming Language

Requires Python 3.9 or newer. The language is described in [SPECS.md](SPECS.md).

Execute Sarma programs using the following command:
```bash
python3 python/sarma.py examples/print.sarma
```

or
```bash
./python/sarma ./examples/print.sarma
```

## Rust interpreter

A faithful port lives in `rust/` (no dependencies, edition 2021). It passes
the same suite, including byte-exact error messages:

```bash
cargo build --manifest-path rust/Cargo.toml
./rust/target/debug/sarma examples/print.sarma
python3 tests/run_tests.py --bin rust/target/debug/sarma
```

No Python? The Rust crate has its own suite with the same oracle
(`cargo test` runs it, no Python needed):

```bash
cargo test --manifest-path rust/Cargo.toml
```

Known divergences from the Python version (all outside the test suite):
integer literals and arithmetic are 64-bit — absurd magnitudes fail with a
clean `Integer overflow` instead of computing big integers; astronomically
large floats may format differently (`1e+21` vs full digits).

## Tests

```bash
python3 tests/run_tests.py        # add -v to list passing tests, or a name to filter
```

Each `examples/*.sarma` and `tests/cases/*.sarma` has a `.out` file with its expected stdout.
If a `.err` file exists, the program must fail with exactly that error message.

