//! Sarma interpreter binary: `sarma <file.sarma>`.
//!
//! CLI contract mirrors `sarma.py` exactly:
//! - exit 0 on success, 1 on any file/lex/parse/runtime error, 2 on misuse;
//! - `sarma: error in <path>: <message>` and `sarma: cannot read <path>: ...`
//!   on stderr, never a traceback;
//! - output streams line by line; a closed stdout (e.g. `| head`) exits 1 quietly.
//!
//! The program runs on a roomy thread stack so deeply nested programs fail
//! with clean errors (via MAX_NESTING) instead of crashing the process.

use std::io::Write;
use std::process::ExitCode;

use sarma::error::SarmaError;
use sarma::interp::execute;
use sarma::lexer::Lexer;
use sarma::parser::Parser;

fn os_message(e: &std::io::Error) -> String {
    // Python-style short reasons (`strerror`), which is what `sarma.py` prints.
    match e.kind() {
        std::io::ErrorKind::NotFound => "No such file or directory".to_string(),
        std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
        std::io::ErrorKind::IsADirectory => "Is a directory".to_string(),
        _ => e.to_string(),
    }
}

fn run_file(path: &str) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sarma: cannot read {path}: {}", os_message(&e));
            return ExitCode::from(1);
        }
    };
    let source = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("sarma: cannot read {path}: file is not valid UTF-8");
            return ExitCode::from(1);
        }
    };
    let stmts = match Lexer::new(&source).lex().and_then(|toks| Parser::new(toks).parse()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sarma: error in {path}: {e}");
            return ExitCode::from(1);
        }
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let mut emit = |line: String| {
        if let Err(e) = writeln!(handle, "{line}") {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                eprintln!("sarma: error writing stdout: {e}");
            }
            std::process::exit(1);
        }
    };
    match execute(&stmts, &mut emit) {
        Ok(()) => ExitCode::SUCCESS,
        Err(SarmaError(msg)) => {
            eprintln!("sarma: error in {path}: {msg}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() != 2 {
        eprintln!("Usage: {} <file.sarma>", argv[0]);
        return ExitCode::from(2);
    }
    let path = argv[1].clone();
    // A generous stack keeps pathological nesting a clean error, not a crash.
    match std::thread::Builder::new()
        .name("sarma-run".to_string())
        .stack_size(128 << 20)
        .spawn(move || run_file(&path))
    {
        Ok(child) => match child.join() {
            Ok(code) => code,
            Err(_) => {
                eprintln!("sarma: internal error");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("sarma: cannot start: {e}");
            ExitCode::from(1)
        }
    }
}
