//! Sarma interpreter library: lexer, parser, and tree-walking interpreter.
//! The binary (`src/main.rs`) is a thin CLI over this API.

pub mod error;
pub mod interp;
pub mod lexer;
pub mod parser;
