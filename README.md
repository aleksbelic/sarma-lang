# Sarma Programming Language

Requires Python 3.9 or newer. The language is described in [SPECS.md](SPECS.md).

Execute Sarma programs using the following command:
```bash
python3 sarma.py examples/print.sarma
```

or
```bash
./sarma ./examples/print.sarma
```

## Tests

```bash
python3 tests/run_tests.py        # add -v to list passing tests, or a name to filter
```

Each `examples/*.sarma` and `tests/cases/*.sarma` has a `.out` file with its expected stdout.
If a `.err` file exists, the program must fail with exactly that error message.

