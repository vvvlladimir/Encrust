//! The recursive-descent parser of the grammar in `docs/design/recipes.md`.

use core_geometry::Scalar;

use super::{BinaryOp, Expr, Function};
use crate::error::RecipeError;

pub(super) fn parse_expr(text: &str) -> Result<Expr, RecipeError> {
    let mut parser = Parser { text, pos: 0 };
    let expr = parser.or()?;
    parser.skip_space();
    if parser.pos < text.len() {
        return Err(parser.error("an operator or the end of the condition"));
    }
    Ok(expr)
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, expected: &'static str) -> RecipeError {
        RecipeError::Syntax {
            text: self.text.to_owned(),
            at: self.pos,
            expected,
        }
    }

    fn rest(&self) -> &str {
        &self.text[self.pos..]
    }

    fn skip_space(&mut self) {
        let trimmed = self.rest().trim_start();
        self.pos = self.text.len() - trimmed.len();
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip_space();
        let found = self.rest().starts_with(token);
        if found {
            self.pos += token.len();
        }
        found
    }

    fn expect(&mut self, token: &str, expected: &'static str) -> Result<(), RecipeError> {
        if self.eat(token) {
            Ok(())
        } else {
            Err(self.error(expected))
        }
    }

    fn binary_chain(
        &mut self,
        operators: &[(&str, BinaryOp)],
        operand: fn(&mut Self) -> Result<Expr, RecipeError>,
    ) -> Result<Expr, RecipeError> {
        let mut left = operand(self)?;
        'chain: loop {
            for &(token, op) in operators {
                if self.eat(token) {
                    let right = operand(self)?;
                    left = Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    };
                    continue 'chain;
                }
            }
            return Ok(left);
        }
    }

    fn or(&mut self) -> Result<Expr, RecipeError> {
        self.binary_chain(&[("||", BinaryOp::Or)], Self::and)
    }

    fn and(&mut self) -> Result<Expr, RecipeError> {
        self.binary_chain(&[("&&", BinaryOp::And)], Self::not)
    }

    fn not(&mut self) -> Result<Expr, RecipeError> {
        self.skip_space();
        if self.rest().starts_with('!') && !self.rest().starts_with("!=") {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.cmp()
    }

    fn cmp(&mut self) -> Result<Expr, RecipeError> {
        const OPERATORS: [(&str, BinaryOp); 6] = [
            ("<=", BinaryOp::Le),
            ("<", BinaryOp::Lt),
            (">=", BinaryOp::Ge),
            (">", BinaryOp::Gt),
            ("==", BinaryOp::Eq),
            ("!=", BinaryOp::Ne),
        ];
        let left = self.sum()?;
        for (token, op) in OPERATORS {
            if self.eat(token) {
                let right = self.sum()?;
                return Ok(Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
        }
        Ok(left)
    }

    fn sum(&mut self) -> Result<Expr, RecipeError> {
        self.binary_chain(&[("+", BinaryOp::Add), ("-", BinaryOp::Sub)], Self::prod)
    }

    fn prod(&mut self) -> Result<Expr, RecipeError> {
        self.binary_chain(&[("*", BinaryOp::Mul), ("/", BinaryOp::Div)], Self::unary)
    }

    fn unary(&mut self) -> Result<Expr, RecipeError> {
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Expr, RecipeError> {
        self.skip_space();
        if self.eat("(") {
            let inner = self.or()?;
            self.expect(")", "a closing parenthesis")?;
            return Ok(inner);
        }
        match self.rest().chars().next() {
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_ascii_lowercase() || c == '_' => self.name_or_call(),
            _ => Err(self.error("a number, a name or a parenthesis")),
        }
    }

    fn number(&mut self) -> Result<Expr, RecipeError> {
        let rest = self.rest();
        let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
        let whole = digits(rest);
        let mut len = whole;
        let after_dot = rest[len..].strip_prefix('.').map_or(0, digits);
        if after_dot > 0 {
            len += 1 + after_dot;
        } else if whole == 0 {
            return Err(self.error("a digit after the decimal point"));
        }
        let value: Scalar = rest[..len]
            .parse()
            .map_err(|_| self.error("a number this build can hold"))?;
        self.pos += len;
        Ok(Expr::Number(value))
    }

    fn name_or_call(&mut self) -> Result<Expr, RecipeError> {
        let rest = self.rest();
        let len = rest
            .bytes()
            .take_while(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
            .count();
        let name = rest[..len].to_owned();
        self.pos += len;
        if self.eat("(") {
            return self.call(&name);
        }
        Ok(match name.as_str() {
            "true" => Expr::Bool(true),
            "false" => Expr::Bool(false),
            _ => Expr::Name(name),
        })
    }

    fn call(&mut self, name: &str) -> Result<Expr, RecipeError> {
        let (function, label, expected) = match name {
            "min" => (Function::Min, "min", 2),
            "max" => (Function::Max, "max", 2),
            "abs" => (Function::Abs, "abs", 1),
            "clamp" => (Function::Clamp, "clamp", 3),
            _ => {
                return Err(RecipeError::UnknownFunction {
                    name: name.to_owned(),
                });
            }
        };
        let mut args = vec![self.or()?];
        while self.eat(",") {
            args.push(self.or()?);
        }
        self.expect(")", "a comma or a closing parenthesis")?;
        if args.len() != expected {
            return Err(RecipeError::Arity {
                name: label,
                expected,
                found: args.len(),
            });
        }
        Ok(Expr::Call { function, args })
    }
}
