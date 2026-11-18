//! What a parsed expression reads and what kind it gives, before anything runs.

use super::{BinaryOp, Expr, Kind};
use crate::error::RecipeError;

pub(super) fn names(expr: &Expr) -> Vec<&str> {
    let mut found = Vec::new();
    collect(expr, &mut found);
    found
}

fn collect<'a>(expr: &'a Expr, found: &mut Vec<&'a str>) {
    match expr {
        Expr::Number(_) | Expr::Bool(_) => {}
        Expr::Name(name) => {
            if !found.contains(&name.as_str()) {
                found.push(name);
            }
        }
        Expr::Not(inner) | Expr::Neg(inner) => collect(inner, found),
        Expr::Binary { left, right, .. } => {
            collect(left, found);
            collect(right, found);
        }
        Expr::Call { args, .. } => args.iter().for_each(|arg| collect(arg, found)),
    }
}

pub(super) fn kind(
    text: &str,
    expr: &Expr,
    kind_of: &dyn Fn(&str) -> Option<Kind>,
) -> Result<Kind, RecipeError> {
    let want = |expected: Kind, operand: &Expr| {
        let found = kind(text, operand, kind_of)?;
        if found == expected {
            Ok(())
        } else {
            Err(wrong_kind(text, expected, found))
        }
    };
    match expr {
        Expr::Number(_) => Ok(Kind::Number),
        Expr::Bool(_) => Ok(Kind::Bool),
        Expr::Name(name) => {
            kind_of(name).ok_or_else(|| RecipeError::UnknownName { name: name.clone() })
        }
        Expr::Not(inner) => want(Kind::Bool, inner).map(|()| Kind::Bool),
        Expr::Neg(inner) => want(Kind::Number, inner).map(|()| Kind::Number),
        Expr::Binary { op, left, right } => match op {
            BinaryOp::Or | BinaryOp::And => {
                want(Kind::Bool, left)?;
                want(Kind::Bool, right)?;
                Ok(Kind::Bool)
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let expected = kind(text, left, kind_of)?;
                want(expected, right)?;
                Ok(Kind::Bool)
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                want(Kind::Number, left)?;
                want(Kind::Number, right)?;
                Ok(Kind::Bool)
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                want(Kind::Number, left)?;
                want(Kind::Number, right)?;
                Ok(Kind::Number)
            }
        },
        Expr::Call { args, .. } => {
            args.iter().try_for_each(|arg| want(Kind::Number, arg))?;
            Ok(Kind::Number)
        }
    }
}

pub(super) fn wrong_kind(text: &str, expected: Kind, found: Kind) -> RecipeError {
    RecipeError::WrongKind {
        text: text.to_owned(),
        expected,
        found,
    }
}
