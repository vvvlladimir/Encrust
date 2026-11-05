//! The condition language: what a `when`, a `stop_if` or an `=` setting says, parsed once and
//! evaluated against a model. The grammar is in `docs/design/recipes.md`.

use core_geometry::Scalar;
use serde::{Deserialize, Serialize};

use crate::error::RecipeError;

/// What a condition or a part of one evaluates to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(Scalar),
    Bool(bool),
}

/// The two kinds a value comes in, which the parser checks before anything runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Number,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    Min,
    Max,
    Abs,
    Clamp,
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(Scalar),
    Bool(bool),
    Name(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        function: Function,
        args: Vec<Expr>,
    },
}

/// An expression with the text it was read from, which errors and the report quote.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    text: String,
    expr: Expr,
}

impl Condition {
    /// Parses `text`, a `when` or the part of a setting after its `=`.
    pub fn parse(text: &str) -> Result<Self, RecipeError> {
        // TODO(step-1): the recursive-descent parser of the grammar in docs/design/recipes.md.
        let _ = text;
        todo!()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Every name the expression reads, once each, in the order they appear.
    pub fn names(&self) -> Vec<&str> {
        // TODO(step-1): walk `expr`.
        todo!()
    }

    /// The kind it evaluates to, given the kind of every name, or why the kinds do not fit.
    pub fn kind(&self, kind_of: &dyn Fn(&str) -> Option<Kind>) -> Result<Kind, RecipeError> {
        // TODO(step-1): check operands as the grammar's typing in docs/design/recipes.md says.
        let _ = kind_of;
        todo!()
    }

    /// Evaluates it, each name looked up by `value_of`, which may note what it read.
    pub fn eval(
        &self,
        value_of: &mut dyn FnMut(&str) -> Result<Value, RecipeError>,
    ) -> Result<Value, RecipeError> {
        // TODO(step-1): evaluate `expr`; `&&` and `||` stop at the first operand that decides.
        let _ = value_of;
        todo!()
    }
}
