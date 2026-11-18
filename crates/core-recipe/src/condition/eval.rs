//! Evaluation of a parsed expression.

use core_geometry::Scalar;

use super::analyse::wrong_kind;
use super::{BinaryOp, Expr, Function, Kind, Value};
use crate::error::RecipeError;

type Lookup<'a> = &'a mut dyn FnMut(&str) -> Result<Value, RecipeError>;

pub(super) fn eval(text: &str, expr: &Expr, value_of: Lookup) -> Result<Value, RecipeError> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Name(name) => value_of(name),
        Expr::Not(inner) => Ok(Value::Bool(!boolean(text, inner, value_of)?)),
        Expr::Neg(inner) => Ok(Value::Number(-number(text, inner, value_of)?)),
        Expr::Binary { op, left, right } => binary(text, *op, left, right, value_of),
        Expr::Call { function, args } => call(text, *function, args, value_of),
    }
}

fn number(text: &str, expr: &Expr, value_of: Lookup) -> Result<Scalar, RecipeError> {
    match eval(text, expr, value_of)? {
        Value::Number(n) => Ok(n),
        Value::Bool(_) => Err(wrong_kind(text, Kind::Number, Kind::Bool)),
    }
}

fn boolean(text: &str, expr: &Expr, value_of: Lookup) -> Result<bool, RecipeError> {
    match eval(text, expr, value_of)? {
        Value::Bool(b) => Ok(b),
        Value::Number(_) => Err(wrong_kind(text, Kind::Bool, Kind::Number)),
    }
}

fn binary(
    text: &str,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    value_of: Lookup,
) -> Result<Value, RecipeError> {
    match op {
        BinaryOp::Or => {
            let decided = boolean(text, left, value_of)? || boolean(text, right, value_of)?;
            Ok(Value::Bool(decided))
        }
        BinaryOp::And => {
            let decided = boolean(text, left, value_of)? && boolean(text, right, value_of)?;
            Ok(Value::Bool(decided))
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            let same = match (eval(text, left, value_of)?, eval(text, right, value_of)?) {
                (Value::Number(a), Value::Number(b)) => exactly_equal(a, b),
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::Number(_), Value::Bool(_)) => {
                    return Err(wrong_kind(text, Kind::Number, Kind::Bool));
                }
                (Value::Bool(_), Value::Number(_)) => {
                    return Err(wrong_kind(text, Kind::Bool, Kind::Number));
                }
            };
            Ok(Value::Bool(same == (op == BinaryOp::Eq)))
        }
        _ => {
            let a = number(text, left, value_of)?;
            let b = number(text, right, value_of)?;
            arithmetic(text, op, a, b)
        }
    }
}

fn arithmetic(text: &str, op: BinaryOp, a: Scalar, b: Scalar) -> Result<Value, RecipeError> {
    Ok(match op {
        BinaryOp::Lt => Value::Bool(a < b),
        BinaryOp::Le => Value::Bool(a <= b),
        BinaryOp::Gt => Value::Bool(a > b),
        BinaryOp::Ge => Value::Bool(a >= b),
        BinaryOp::Add => Value::Number(a + b),
        BinaryOp::Sub => Value::Number(a - b),
        BinaryOp::Mul => Value::Number(a * b),
        BinaryOp::Div if b == 0.0 => {
            return Err(RecipeError::DivisionByZero {
                text: text.to_owned(),
            });
        }
        BinaryOp::Div => Value::Number(a / b),
        BinaryOp::Or | BinaryOp::And | BinaryOp::Eq | BinaryOp::Ne => {
            unreachable!("handled by `binary`")
        }
    })
}

fn call(
    text: &str,
    function: Function,
    args: &[Expr],
    value_of: Lookup,
) -> Result<Value, RecipeError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(number(text, arg, value_of)?);
    }
    let result = match (function, values.as_slice()) {
        (Function::Min, [a, b]) => a.min(*b),
        (Function::Max, [a, b]) => a.max(*b),
        (Function::Abs, [a]) => a.abs(),
        (Function::Clamp, [x, lo, hi]) => x.max(*lo).min(*hi),
        _ => unreachable!("the parser fixes each function's arity"),
    };
    Ok(Value::Number(result))
}

#[expect(clippy::float_cmp, reason = "`==` in a recipe means exact equality")]
fn exactly_equal(a: Scalar, b: Scalar) -> bool {
    a == b
}
