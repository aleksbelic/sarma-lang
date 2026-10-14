//! Sarma test suite: no Python required (`cargo test`).
//!
//! Same oracle as `tests/run_tests.py`: every `.sarma` file under
//! `examples/` and `tests/cases/` runs through the real CLI binary and is
//! compared against its `.out` (expected stdout) and optional `.err`
//! (expected failure message, exit code 1). Plus CLI behavior tests and
//! in-process lexer/parser/interpreter checks.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sarma::error::SarmaError;
use sarma::interp::execute;
use sarma::lexer::{Lexer, TokenKind};
use sarma::parser::{Parser, MAX_NESTING};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sarma"))
}

fn run_cli(cwd: &Path, args: &[&str]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let out = Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to run sarma binary");
    (out.status.code(), out.stdout, out.stderr)
}

/// Run source through the library, collecting printed lines.
fn run_src(source: &str) -> Result<Vec<String>, SarmaError> {
    let tokens = Lexer::new(source).lex()?;
    let stmts = Parser::new(tokens).parse()?;
    let mut lines = Vec::new();
    execute(&stmts, &mut |line: String| lines.push(line))?;
    Ok(lines)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sarma-rs-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }

    fn file(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// ------------------------------------------------------------ file cases

fn discover_cases() -> Vec<String> {
    let root = repo_root();
    let mut cases = Vec::new();
    for dir in ["examples", "tests/cases"] {
        let mut names: Vec<String> = fs::read_dir(root.join(dir))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".sarma"))
            .collect();
        names.sort();
        for n in names {
            cases.push(format!("{dir}/{n}"));
        }
    }
    cases
}

fn check_file_case(root: &Path, rel: &str) -> Result<(), String> {
    let base = root.join(&rel[..rel.len() - ".sarma".len()]);
    let out_path = base.with_extension("out");
    if !out_path.exists() {
        return Err(format!(
            "missing {}.out",
            out_path.file_name().unwrap().to_string_lossy()
        ));
    }
    let expected_out = fs::read(&out_path).map_err(|e| e.to_string())?;
    let (code, stdout, stderr) = run_cli(root, &[rel]);
    if stdout != expected_out {
        return Err(format!(
            "stdout mismatch:\n    expected {expected_out:?}\n    got      {stdout:?}"
        ));
    }
    let err_path = base.with_extension("err");
    if err_path.exists() {
        let expected_err = fs::read(&err_path).map_err(|e| e.to_string())?;
        let expected = format!(
            "sarma: error in {rel}: {}",
            String::from_utf8_lossy(&expected_err).trim()
        );
        if code != Some(1) {
            return Err(format!("expected exit code 1, got {code:?}"));
        }
        if String::from_utf8_lossy(&stderr).trim() != expected {
            return Err(format!(
                "stderr mismatch:\n    expected {expected:?}\n    got      {:?}",
                String::from_utf8_lossy(&stderr).trim()
            ));
        }
    } else {
        if code != Some(0) {
            return Err(format!("expected exit code 0, got {code:?}"));
        }
        if !stderr.is_empty() {
            return Err(format!("unexpected stderr: {stderr:?}"));
        }
    }
    Ok(())
}

#[test]
fn file_cases() {
    let root = repo_root();
    let cases = discover_cases();
    assert!(!cases.is_empty(), "no .sarma cases found");
    let mut failures = Vec::new();
    for rel in &cases {
        if let Err(problem) = check_file_case(&root, rel) {
            failures.push(format!("{rel}: {problem}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ------------------------------------------------------------ CLI tests

#[test]
fn test_cli_missing_file() {
    let (code, _, stderr) = run_cli(&repo_root(), &["no_such_file.sarma"]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(1));
    assert!(err.starts_with("sarma: cannot read no_such_file.sarma"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn test_cli_directory_instead_of_file() {
    let tmp = TempDir::new("isdir");
    let dir = tmp.path.to_string_lossy().to_string();
    let (code, _, stderr) = run_cli(&repo_root(), &[&dir]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(1));
    assert!(err.starts_with("sarma: cannot read"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn test_cli_invalid_utf8() {
    let tmp = TempDir::new("badutf8");
    let path = tmp.file("bad.sarma", b"print(\"\xff\")");
    let path = path.to_string_lossy().to_string();
    let (code, _, stderr) = run_cli(&repo_root(), &[&path]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(1));
    assert!(err.contains("not valid UTF-8"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn test_cli_usage_error() {
    let (code, _, stderr) = run_cli(&repo_root(), &[]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(2));
    assert!(err.contains("Usage"), "{err}");
}

#[test]
fn test_cli_deep_nesting_is_clean_error() {
    let tmp = TempDir::new("deep");
    let src = format!("print({} \"a\" {})\n", "(".repeat(500), ")".repeat(500));
    let path = tmp.file("deep.sarma", src.as_bytes());
    let path = path.to_string_lossy().to_string();
    let (code, _, stderr) = run_cli(&repo_root(), &[&path]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(1));
    assert!(err.contains("nested too deeply"), "{}", &err[err.len().saturating_sub(200)..]);
    assert!(!err.contains("panicked"), "{err}");
}

#[test]
fn test_cli_broken_pipe_is_quiet() {
    // Enough output (~1.2 MB) that the interpreter is still writing when the
    // reader leaves. Must exit 1 silently, not die of SIGPIPE.
    let tmp = TempDir::new("many");
    let src = format!("print(\"{}\")\n", "x".repeat(60)).repeat(20_000);
    let path = tmp.file("many.sarma", src.as_bytes());
    let mut child = Command::new(bin())
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    // Drop the reader: the writer must notice EPIPE and exit quietly.
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    let code = child.wait().unwrap().code();
    assert_eq!(first, format!("{}\n", "x".repeat(60)));
    assert_eq!(code, Some(1), "broken pipe must exit 1, stderr: {err}");
    assert!(!err.contains("panicked") && !err.contains("BrokenPipe"), "{err}");
}

#[test]
fn test_cli_long_concat_chain() {
    let tmp = TempDir::new("longchain");
    let src = format!("print({})\n", vec!["\"a\""; 5000].join(" + "));
    let path = tmp.file("long.sarma", src.as_bytes());
    let path = path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_cli(&repo_root(), &[&path]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout, format!("{}\n", "a".repeat(5000)).into_bytes());
    assert!(stderr.is_empty());
}

// --------------------------------------------------- in-process tests

#[test]
fn test_output_streams_before_error() {
    let tokens = Lexer::new("print(\"before\")\nprint(missing)\n")
        .lex()
        .unwrap();
    let stmts = Parser::new(tokens).parse().unwrap();
    let mut lines = Vec::new();
    let err = execute(&stmts, &mut |line: String| lines.push(line)).unwrap_err();
    assert_eq!(lines, vec!["before".to_string()]);
    assert!(err.to_string().contains("missing"), "{err}");
}

#[test]
fn test_basic_run() {
    assert_eq!(
        run_src("var a = \"x\"\nprint(a + \"y\")\n").unwrap(),
        vec!["xy".to_string()]
    );
}

#[test]
fn test_crlf_source() {
    assert_eq!(
        run_src("print(\"a\")\r\nprint(\"b\")\r\n").unwrap(),
        vec!["a".to_string(), "b".to_string()]
    );
}

#[test]
fn test_all_escapes() {
    assert_eq!(
        run_src("print(\"a\\n\\t\\r\\\\\\'\\\"\\0b\")").unwrap(),
        vec!["a\n\t\r\\'\"\0b".to_string()]
    );
}

#[test]
fn test_two_char_operators_are_single_tokens() {
    for op in ["==", "!=", "<=", ">=", "&&", "||", "->"] {
        for src in [format!("a {op} b"), format!("a{op}b")] {
            let toks = Lexer::new(&src).lex().unwrap();
            let kinds: Vec<(TokenKind, String)> = toks[..toks.len() - 1]
                .iter()
                .map(|t| (t.kind, t.value.clone()))
                .collect();
            assert_eq!(
                kinds,
                vec![
                    (TokenKind::Identifier, "a".to_string()),
                    (TokenKind::Op, op.to_string()),
                    (TokenKind::Identifier, "b".to_string()),
                ],
                "{src}"
            );
        }
    }
}

#[test]
fn test_single_equals_and_single_char_ops() {
    let toks = Lexer::new("a = b < c").lex().unwrap();
    let kinds: Vec<(TokenKind, String)> = toks[..toks.len() - 1]
        .iter()
        .map(|t| (t.kind, t.value.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (TokenKind::Identifier, "a".to_string()),
            (TokenKind::Assign, "=".to_string()),
            (TokenKind::Identifier, "b".to_string()),
            (TokenKind::Op, "<".to_string()),
            (TokenKind::Identifier, "c".to_string()),
        ]
    );
}

#[test]
fn test_string_token_points_at_opening_quote() {
    let toks = Lexer::new("print(\"abc\")").lex().unwrap();
    let tok = toks
        .iter()
        .find(|t| t.kind == TokenKind::String)
        .unwrap();
    assert_eq!((tok.line, tok.col), (1, 7));
}

#[test]
fn test_long_concat_chain() {
    let src = format!("print({})\n", vec!["\"a\""; 5000].join(" + "));
    assert_eq!(run_src(&src).unwrap(), vec!["a".repeat(5000)]);
}

#[test]
fn test_nesting_limit() {
    let nested = |n: usize| format!("print({} \"a\" {})\n", "(".repeat(n), ")".repeat(n));
    assert_eq!(run_src(&nested(MAX_NESTING)).unwrap(), vec!["a".to_string()]);
    for n in [MAX_NESTING + 1, 1000] {
        let err = run_src(&nested(n)).unwrap_err();
        assert!(err.to_string().contains("nested too deeply"), "{err}");
    }
}

#[test]
fn test_nesting_counter_resets_between_expressions() {
    let src = "print((\"a\"))\n".repeat(MAX_NESTING * 2);
    assert_eq!(
        run_src(&src).unwrap(),
        vec!["a".to_string(); MAX_NESTING * 2]
    );
}
