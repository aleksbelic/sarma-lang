//! Tree-walking interpreter: values, strict operators, and streaming execution.
//! Mirrors `sarma.py` semantics, including exact error message text.
//!
//! One deliberate divergence: integers are 64-bit, so absurdly large literals
//! or overflowing arithmetic report `Integer overflow` instead of computing
//! arbitrary-precision results like the Python version.

use std::collections::HashMap;

use crate::error::SarmaError;
use crate::parser::{Expr, NumVal, Stmt};

#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

/// How a runtime value prints: booleans as true/false, strings as-is,
/// whole floats without decimals.
pub fn display(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(v) => {
            if v.fract() == 0.0 && v.abs() < 9.223_372_036_854_776e18 {
                (*v as i64).to_string()
            } else {
                format!("{v}")
            }
        }
    }
}

/// Falsiness: `false`, `0`, and `""` are falsy; everything else is truthy.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(v) => *v != 0.0,
        Value::Str(s) => !s.is_empty(),
    }
}

fn same_family(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Bool(_), Value::Bool(_)) => true,
        (Value::Bool(_), _) | (_, Value::Bool(_)) => false,
        (Value::Str(_), Value::Str(_)) => true,
        (Value::Str(_), _) | (_, Value::Str(_)) => false,
        _ => true, // two numbers
    }
}

fn strict_eq(a: &Value, b: &Value) -> bool {
    if !same_family(a, b) {
        return false;
    }
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => int_float_eq(*x, *y),
        (Value::Float(x), Value::Int(y)) => int_float_eq(*y, *x),
        _ => false, // unreachable: families already checked
    }
}

/// Exact mixed int/float equality when possible, float approximation otherwise.
fn int_float_eq(n: i64, f: f64) -> bool {
    if f.fract() == 0.0 && f.abs() < 9.223_372_036_854_776e18 {
        n == f as i64
    } else {
        n as f64 == f
    }
}

fn is_number(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Float(_))
}

fn strict_rel(op: &str, a: &Value, b: &Value, line: usize) -> Result<bool, SarmaError> {
    if let (Value::Str(x), Value::Str(y)) = (a, b) {
        return Ok(match op {
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        });
    }
    if is_number(a) && is_number(b) {
        return Ok(num_cmp(op, a, b));
    }
    Err(SarmaError(format!(
        "Operator '{op}' requires two numbers or two strings at line {line}"
    )))
}

fn num_cmp(op: &str, a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => match op {
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        },
        (Value::Int(x), Value::Float(y)) => cmp_mixed(op, *x, *y),
        (Value::Float(x), Value::Int(y)) => cmp_mixed_flipped(op, *x, *y),
        (Value::Float(x), Value::Float(y)) => match op {
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        },
        _ => unreachable!(),
    }
}

fn cmp_mixed(op: &str, n: i64, f: f64) -> bool {
    if f.fract() == 0.0 && f.abs() < 9.223_372_036_854_776e18 {
        let g = f as i64;
        match op {
            "<" => n < g,
            "<=" => n <= g,
            ">" => n > g,
            _ => n >= g,
        }
    } else {
        match op {
            "<" => (n as f64) < f,
            "<=" => (n as f64) <= f,
            ">" => (n as f64) > f,
            _ => (n as f64) >= f,
        }
    }
}

fn cmp_mixed_flipped(op: &str, f: f64, n: i64) -> bool {
    // Same as cmp_mixed with the comparison direction swapped.
    let flipped = match op {
        "<" => ">",
        "<=" => ">=",
        ">" => "<",
        _ => "<=",
    };
    cmp_mixed(flipped, n, f)
}

fn eval_expr(node: &Expr, env: &HashMap<String, Value>) -> Result<Value, SarmaError> {
    match node {
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::Num(NumVal::Int(n)) => Ok(Value::Int(*n)),
        Expr::Num(NumVal::Float(f)) => Ok(Value::Float(*f)),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Var(name, line) => env.get(name).cloned().ok_or_else(|| {
            SarmaError(format!("Undefined variable '{name}' at line {line}"))
        }),
        // Neg/Not chains are applied iteratively so long prefix runs
        // (e.g. `!!!...!x`) can't overflow the stack.
        Expr::Neg(_, _) | Expr::Not(_, _) => {
            let mut ops: Vec<(bool, usize)> = Vec::new(); // (is_not, line)
            let mut cur = node;
            loop {
                match cur {
                    Expr::Neg(inner, line) => {
                        ops.push((false, *line));
                        cur = inner;
                    }
                    Expr::Not(inner, line) => {
                        ops.push((true, *line));
                        cur = inner;
                    }
                    _ => break,
                }
            }
            let mut value = eval_expr(cur, env)?;
            for (is_not, line) in ops.into_iter().rev() {
                if is_not {
                    value = Value::Bool(!truthy(&value));
                } else {
                    match value {
                        Value::Bool(_) => {
                            return Err(SarmaError(format!(
                                "Operator '-' does not accept booleans at line {line}"
                            )))
                        }
                        Value::Str(_) => {
                            return Err(SarmaError(format!(
                                "Cannot negate a string at line {line}"
                            )))
                        }
                        Value::Int(n) => {
                            value = Value::Int(n.checked_neg().ok_or_else(|| {
                                SarmaError(format!("Integer overflow at line {line}"))
                            })?)
                        }
                        Value::Float(f) => value = Value::Float(-f),
                    }
                }
            }
            Ok(value)
        }
        Expr::Add(parts, ops, line) => {
            let mut values = Vec::with_capacity(parts.len());
            for p in parts {
                values.push(eval_expr(p, env)?);
            }
            if values.iter().all(|v| matches!(v, Value::Str(_))) {
                if ops.iter().any(|op| op == "-") {
                    return Err(SarmaError(format!(
                        "Operator '-' requires numbers at line {line}"
                    )));
                }
                let mut out = String::new();
                for v in &values {
                    if let Value::Str(s) = v {
                        out.push_str(s);
                    }
                }
                return Ok(Value::Str(out));
            }
            if values.iter().any(|v| matches!(v, Value::Bool(_))) {
                return Err(SarmaError(format!(
                    "Operator '{}' does not accept booleans at line {line}",
                    ops[0]
                )));
            }
            if values.iter().any(|v| matches!(v, Value::Str(_))) {
                return Err(SarmaError(format!(
                    "Cannot mix strings and numbers with '+' at line {line}"
                )));
            }
            let mut nums: Vec<NumVal> = values
                .iter()
                .map(|v| match v {
                    Value::Int(n) => NumVal::Int(*n),
                    Value::Float(f) => NumVal::Float(*f),
                    _ => unreachable!(),
                })
                .collect();
            let mut acc = nums.remove(0);
            for (op, v) in ops.iter().zip(nums.into_iter()) {
                acc = if op == "+" {
                    add_num(acc, v, *line)?
                } else {
                    sub_num(acc, v, *line)?
                };
            }
            Ok(match acc {
                NumVal::Int(n) => Value::Int(n),
                NumVal::Float(f) => Value::Float(f),
            })
        }
        Expr::Mul(parts, ops, line) => {
            let mut values = Vec::with_capacity(parts.len());
            for p in parts {
                values.push(eval_expr(p, env)?);
            }
            if values.iter().any(|v| matches!(v, Value::Bool(_))) {
                return Err(SarmaError(format!(
                    "Operator '{}' does not accept booleans at line {line}",
                    ops[0]
                )));
            }
            if values.iter().any(|v| matches!(v, Value::Str(_))) {
                return Err(SarmaError(format!(
                    "Operator '{}' requires numbers at line {line}",
                    ops[0]
                )));
            }
            let mut acc = to_num(&values[0]);
            for (op, v) in ops.iter().zip(values[1..].iter()) {
                let rhs = to_num(v);
                acc = match op.as_str() {
                    "*" => mul_num(acc, rhs, *line)?,
                    "/" => div_num(acc, rhs, *line)?,
                    _ => rem_num(acc, rhs, *line)?,
                };
            }
            Ok(match acc {
                NumVal::Int(n) => Value::Int(n),
                NumVal::Float(f) => Value::Float(f),
            })
        }
        Expr::Eq(parts, ops, _) => {
            let mut result = strict_eq(&eval_expr(&parts[0], env)?, &eval_expr(&parts[1], env)?);
            for (op, part) in ops[1..].iter().zip(parts[2..].iter()) {
                let current = strict_eq(&bool_val(result), &eval_expr(part, env)?);
                result = if op == "==" { current } else { !current };
            }
            if ops[0] == "!=" {
                result = !result;
            }
            Ok(Value::Bool(result))
        }
        Expr::Rel(parts, ops, line) => {
            let mut result = strict_rel(
                &ops[0],
                &eval_expr(&parts[0], env)?,
                &eval_expr(&parts[1], env)?,
                *line,
            )?;
            for (op, part) in ops[1..].iter().zip(parts[2..].iter()) {
                result = strict_rel(op, &bool_val(result), &eval_expr(part, env)?, *line)?;
            }
            Ok(Value::Bool(result))
        }
        Expr::And(parts) => {
            let mut result = Value::Bool(true);
            for p in parts {
                result = eval_expr(p, env)?;
                if !truthy(&result) {
                    return Ok(result);
                }
            }
            Ok(result)
        }
        Expr::Or(parts) => {
            let mut result = Value::Bool(false);
            for p in parts {
                result = eval_expr(p, env)?;
                if truthy(&result) {
                    return Ok(result);
                }
            }
            Ok(result)
        }
    }
}

fn bool_val(b: bool) -> Value {
    Value::Bool(b)
}

fn to_num(v: &Value) -> NumVal {
    match v {
        Value::Int(n) => NumVal::Int(*n),
        Value::Float(f) => NumVal::Float(*f),
        _ => unreachable!(),
    }
}

fn overflow(line: usize) -> SarmaError {
    SarmaError(format!("Integer overflow at line {line}"))
}

/// Truncating toward negative infinity (Python-style floor division
/// quotient); `b` must be nonzero. `None` only on MIN / -1.
fn div_floor(a: i64, b: i64) -> Option<i64> {
    let q = a.checked_div(b)?;
    let r = a.checked_rem(b)?;
    if r != 0 && (r < 0) != (b < 0) {
        q.checked_sub(1)
    } else {
        Some(q)
    }
}

fn add_num(a: NumVal, b: NumVal, line: usize) -> Result<NumVal, SarmaError> {
    match (a, b) {
        (NumVal::Int(x), NumVal::Int(y)) => Ok(NumVal::Int(x.checked_add(y).ok_or_else(|| overflow(line))?)),
        _ => Ok(NumVal::Float(num_f64(&a) + num_f64(&b))),
    }
}

fn sub_num(a: NumVal, b: NumVal, line: usize) -> Result<NumVal, SarmaError> {
    match (a, b) {
        (NumVal::Int(x), NumVal::Int(y)) => Ok(NumVal::Int(x.checked_sub(y).ok_or_else(|| overflow(line))?)),
        _ => Ok(NumVal::Float(num_f64(&a) - num_f64(&b))),
    }
}

fn mul_num(a: NumVal, b: NumVal, line: usize) -> Result<NumVal, SarmaError> {
    match (a, b) {
        (NumVal::Int(x), NumVal::Int(y)) => Ok(NumVal::Int(x.checked_mul(y).ok_or_else(|| overflow(line))?)),
        _ => Ok(NumVal::Float(num_f64(&a) * num_f64(&b))),
    }
}

fn div_num(a: NumVal, b: NumVal, line: usize) -> Result<NumVal, SarmaError> {
    if is_zero(&b) {
        return Err(SarmaError(format!("Division by zero at line {line}")));
    }
    Ok(NumVal::Float(num_f64(&a) / num_f64(&b)))
}

fn rem_num(a: NumVal, b: NumVal, line: usize) -> Result<NumVal, SarmaError> {
    if is_zero(&b) {
        return Err(SarmaError(format!("Division by zero at line {line}")));
    }
    match (a, b) {
        // Floored remainder, matching Python's `%` (sign follows the divisor).
        (NumVal::Int(x), NumVal::Int(y)) => {
            let q = div_floor(x, y).ok_or_else(|| overflow(line))?;
            let prod = y.checked_mul(q).ok_or_else(|| overflow(line))?;
            Ok(NumVal::Int(x.checked_sub(prod).ok_or_else(|| overflow(line))?))
        }
        _ => {
            let (af, bf) = (num_f64(&a), num_f64(&b));
            Ok(NumVal::Float(af - bf * (af / bf).floor()))
        }
    }
}

fn num_f64(v: &NumVal) -> f64 {
    match v {
        NumVal::Int(n) => *n as f64,
        NumVal::Float(f) => *f,
    }
}

fn is_zero(v: &NumVal) -> bool {
    match v {
        NumVal::Int(n) => *n == 0,
        NumVal::Float(f) => *f == 0.0,
    }
}

/// Run statements, calling `out` with each printed line as it happens.
pub fn execute(stmts: &[Stmt], out: &mut dyn FnMut(String)) -> Result<(), SarmaError> {
    exec_block(stmts, &mut HashMap::new(), out)
}

fn exec_block(
    stmts: &[Stmt],
    env: &mut HashMap<String, Value>,
    out: &mut dyn FnMut(String),
) -> Result<(), SarmaError> {
    for s in stmts {
        exec_stmt(s, env, out)?;
    }
    Ok(())
}

fn exec_stmt(
    s: &Stmt,
    env: &mut HashMap<String, Value>,
    out: &mut dyn FnMut(String),
) -> Result<(), SarmaError> {
    match s {
        Stmt::Print(expr, _) => {
            out(display(&eval_expr(expr, env)?));
            Ok(())
        }
        Stmt::Var(name, expr, line) => {
            if env.contains_key(name) {
                return Err(SarmaError(format!(
                    "Variable '{name}' already defined (line {line})"
                )));
            }
            let value = eval_expr(expr, env)?;
            env.insert(name.clone(), value);
            Ok(())
        }
        Stmt::Assign(name, expr, line) => {
            if !env.contains_key(name) {
                return Err(SarmaError(format!(
                    "Undefined variable '{name}' at line {line} (declare with var first)"
                )));
            }
            let value = eval_expr(expr, env)?;
            env.insert(name.clone(), value);
            Ok(())
        }
        Stmt::Block(stmts, _) => exec_block(stmts, env, out),
        Stmt::If {
            cond,
            then,
            otherwise,
            ..
        } => {
            let branch = if truthy(&eval_expr(cond, env)?) {
                then.as_ref()
            } else {
                match otherwise {
                    None => return Ok(()),
                    Some(b) => b.as_ref(),
                }
            };
            match branch {
                Stmt::Block(stmts, _) => exec_block(stmts, env, out),
                other => exec_stmt(other, env, out),
            }
        }
    }
}
