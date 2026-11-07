use core_geometry::Scalar;
use proptest::prelude::*;

use super::*;

fn parse(text: &str) -> Condition {
    Condition::parse(text).expect("the test text is a valid condition")
}

fn lookup(name: &str) -> Result<Value, RecipeError> {
    match name {
        "x" => Ok(Value::Number(3.0)),
        "y" => Ok(Value::Number(0.0)),
        "a" => Ok(Value::Bool(true)),
        "b" => Ok(Value::Bool(false)),
        _ => Err(RecipeError::Unmeasured { name: name.into() }),
    }
}

fn kinds(name: &str) -> Option<Kind> {
    match name {
        "x" | "y" => Some(Kind::Number),
        "a" | "b" => Some(Kind::Bool),
        _ => None,
    }
}

fn run(text: &str) -> Result<Value, RecipeError> {
    parse(text).eval(&mut lookup)
}

fn syntax_at(text: &str) -> usize {
    match Condition::parse(text) {
        Err(RecipeError::Syntax { at, .. }) => at,
        other => panic!("`{text}` should be a syntax error, got {other:?}"),
    }
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(run("1 + 2 * 3 == 7"), Ok(Value::Bool(true)));
    assert_eq!(run("(1 + 2) * 3"), Ok(Value::Number(9.0)));
    assert_eq!(run("8 - 2 - 1"), Ok(Value::Number(5.0)));
}

#[test]
fn not_binds_looser_than_comparison_and_tighter_than_and() {
    assert_eq!(run("!a && b"), Ok(Value::Bool(false)));
    assert_eq!(run("!b && a"), Ok(Value::Bool(true)));
    assert_eq!(run("!x == 3"), Ok(Value::Bool(false)).and(run("!(x == 3)")));
    assert_eq!(run("a || b && b"), Ok(Value::Bool(true)));
}

#[test]
fn unary_minus_nests_and_binds_tighter_than_multiplication() {
    assert_eq!(run("-2 * 3"), Ok(Value::Number(-6.0)));
    assert_eq!(run("--x"), Ok(Value::Number(3.0)));
    assert_eq!(run("2 - -x"), Ok(Value::Number(5.0)));
}

#[test]
fn numbers_read_with_and_without_a_leading_digit() {
    assert_eq!(run("2.5 + .5"), Ok(Value::Number(3.0)));
    assert_eq!(run("  2  "), Ok(Value::Number(2.0)));
}

#[test]
fn functions_compute_their_closed_forms() {
    assert_eq!(run("min(x, 1)"), Ok(Value::Number(1.0)));
    assert_eq!(run("max(x, 1)"), Ok(Value::Number(3.0)));
    assert_eq!(run("abs(-x)"), Ok(Value::Number(3.0)));
    assert_eq!(run("clamp(x, 0, 2)"), Ok(Value::Number(2.0)));
    assert_eq!(run("clamp(-x, 0, 2)"), Ok(Value::Number(0.0)));
}

#[test]
fn comparison_chains_are_refused() {
    assert_eq!(syntax_at("1 < 2 < 3"), 6);
    assert_eq!(syntax_at("1 == 2 != 3"), 7);
}

#[test]
fn malformed_text_is_a_syntax_error_at_its_byte_offset() {
    assert_eq!(syntax_at(""), 0);
    assert_eq!(syntax_at("1 +"), 3);
    assert_eq!(syntax_at("1 2"), 2);
    assert_eq!(syntax_at("(1"), 2);
    assert_eq!(syntax_at("a & b"), 2);
    assert_eq!(syntax_at("a = b"), 2);
    assert_eq!(syntax_at("2."), 1);
    assert_eq!(syntax_at("X"), 0);
    assert_eq!(syntax_at("1 + $"), 4);
}

#[test]
fn unknown_function_and_wrong_arity_are_refused_when_read() {
    assert!(matches!(
        Condition::parse("sqrt(4)"),
        Err(RecipeError::UnknownFunction { name }) if name == "sqrt"
    ));
    assert!(matches!(
        Condition::parse("min(1)"),
        Err(RecipeError::Arity {
            name: "min",
            expected: 2,
            found: 1
        })
    ));
    assert!(matches!(
        Condition::parse("clamp(1, 2, 3, 4)"),
        Err(RecipeError::Arity {
            name: "clamp",
            expected: 3,
            found: 4
        })
    ));
}

#[test]
fn names_are_listed_once_in_first_appearance_order_without_functions() {
    let condition = parse("max(y, x) > x && !a || y < 1");
    assert_eq!(condition.names(), vec!["y", "x", "a"]);
    assert!(parse("1 + 2").names().is_empty());
}

#[test]
fn kinds_follow_the_operators() {
    let kind = |text: &str| parse(text).kind(&kinds);
    assert_eq!(kind("x + 1"), Ok(Kind::Number));
    assert_eq!(kind("min(x, y)"), Ok(Kind::Number));
    assert_eq!(kind("x < 1"), Ok(Kind::Bool));
    assert_eq!(kind("a == b"), Ok(Kind::Bool));
    assert_eq!(kind("a && !b || x >= y"), Ok(Kind::Bool));
}

#[test]
fn mixed_kinds_are_wrong_kind_with_the_condition_text() {
    for text in [
        "x && a", "!x", "-a", "a + 1", "a < 1", "x == a", "a == x", "abs(a)",
    ] {
        match parse(text).kind(&kinds) {
            Err(RecipeError::WrongKind { text: quoted, .. }) => assert_eq!(quoted, text),
            other => panic!("`{text}` should be a WrongKind, got {other:?}"),
        }
    }
    assert!(matches!(
        parse("x && a").kind(&kinds),
        Err(RecipeError::WrongKind {
            expected: Kind::Bool,
            found: Kind::Number,
            ..
        })
    ));
}

#[test]
fn a_name_without_a_kind_is_unknown() {
    assert!(matches!(
        parse("x < nope").kind(&kinds),
        Err(RecipeError::UnknownName { name }) if name == "nope"
    ));
}

#[test]
fn division_by_zero_is_an_error_not_infinity() {
    assert!(matches!(
        run("1 / y"),
        Err(RecipeError::DivisionByZero { .. })
    ));
    assert!(matches!(
        run("1 / (x - 3)"),
        Err(RecipeError::DivisionByZero { .. })
    ));
    assert_eq!(run("y / x"), Ok(Value::Number(0.0)));
}

#[test]
fn eval_of_mixed_kinds_is_wrong_kind() {
    assert!(matches!(run("x && a"), Err(RecipeError::WrongKind { .. })));
    assert!(matches!(run("x == a"), Err(RecipeError::WrongKind { .. })));
    assert!(matches!(run("a == x"), Err(RecipeError::WrongKind { .. })));
    assert!(matches!(run("a + 1"), Err(RecipeError::WrongKind { .. })));
}

#[test]
fn lookup_errors_propagate() {
    assert!(matches!(
        run("ghost > 1"),
        Err(RecipeError::Unmeasured { .. })
    ));
}

#[test]
fn and_and_or_do_not_look_up_a_right_side_that_cannot_decide() {
    let mut read = Vec::new();
    let mut tracking = |name: &str| {
        read.push(name.to_owned());
        lookup(name)
    };
    assert_eq!(
        parse("b && ghost").eval(&mut tracking),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        parse("a || ghost").eval(&mut tracking),
        Ok(Value::Bool(true))
    );
    assert_eq!(read, ["b", "a"]);
    assert!(matches!(
        run("a && ghost"),
        Err(RecipeError::Unmeasured { .. })
    ));
    assert!(matches!(
        run("b || ghost"),
        Err(RecipeError::Unmeasured { .. })
    ));
}

fn number_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        (0u8..10).prop_map(|n| Expr::Number(Scalar::from(n))),
        Just(Expr::Name("x".into())),
    ];
    leaf.prop_recursive(4, 24, 3, |inner| {
        let op = prop_oneof![
            Just(BinaryOp::Add),
            Just(BinaryOp::Sub),
            Just(BinaryOp::Mul),
            Just(BinaryOp::Div)
        ];
        let function = prop_oneof![
            Just((Function::Min, 2)),
            Just((Function::Max, 2)),
            Just((Function::Abs, 1)),
            Just((Function::Clamp, 3)),
        ];
        prop_oneof![
            inner.clone().prop_map(|e| Expr::Neg(Box::new(e))),
            (op, inner.clone(), inner.clone()).prop_map(|(op, l, r)| Expr::Binary {
                op,
                left: Box::new(l),
                right: Box::new(r)
            }),
            (function, prop::collection::vec(inner, 3)).prop_map(|((function, n), mut args)| {
                args.truncate(n);
                Expr::Call { function, args }
            }),
        ]
    })
}

fn bool_expr() -> impl Strategy<Value = Expr> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(Expr::Bool),
        Just(Expr::Name("a".into())),
        Just(Expr::Name("b".into())),
    ];
    leaf.prop_recursive(4, 24, 2, |inner| {
        let logic = prop_oneof![Just(BinaryOp::And), Just(BinaryOp::Or)];
        let compare = prop_oneof![
            Just(BinaryOp::Lt),
            Just(BinaryOp::Le),
            Just(BinaryOp::Gt),
            Just(BinaryOp::Ge),
            Just(BinaryOp::Eq),
            Just(BinaryOp::Ne)
        ];
        prop_oneof![
            inner.clone().prop_map(|e| Expr::Not(Box::new(e))),
            (logic, inner.clone(), inner).prop_map(|(op, l, r)| Expr::Binary {
                op,
                left: Box::new(l),
                right: Box::new(r)
            }),
            (compare, number_expr(), number_expr()).prop_map(|(op, l, r)| Expr::Binary {
                op,
                left: Box::new(l),
                right: Box::new(r)
            }),
        ]
    })
}

fn print(expr: &Expr) -> String {
    match expr {
        Expr::Number(n) => format!("{n:?}"),
        Expr::Bool(b) => b.to_string(),
        Expr::Name(name) => name.clone(),
        Expr::Not(e) => format!("!({})", print(e)),
        Expr::Neg(e) => format!("-({})", print(e)),
        Expr::Binary { op, left, right } => {
            let symbol = match op {
                BinaryOp::Or => "||",
                BinaryOp::And => "&&",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
            };
            format!("({}) {symbol} ({})", print(left), print(right))
        }
        Expr::Call { function, args } => {
            let name = match function {
                Function::Min => "min",
                Function::Max => "max",
                Function::Abs => "abs",
                Function::Clamp => "clamp",
            };
            let args: Vec<_> = args.iter().map(print).collect();
            format!("{name}({})", args.join(", "))
        }
    }
}

fn direct(expr: Expr) -> Condition {
    Condition {
        text: String::new(),
        expr,
    }
}

proptest! {
    #[test]
    fn a_printed_expression_evaluates_as_the_expression_built_directly(expr in bool_expr()) {
        let parsed = Condition::parse(&print(&expr)).expect("a printed expression parses");
        let built = direct(expr);
        prop_assert_eq!(
            parsed.eval(&mut lookup).ok(),
            built.eval(&mut lookup).ok()
        );
        prop_assert_eq!(parsed.eval(&mut lookup).is_err(), built.eval(&mut lookup).is_err());
    }

    #[test]
    fn a_number_expression_prints_and_parses_to_the_same_value(expr in number_expr()) {
        let parsed = Condition::parse(&print(&expr)).expect("a printed expression parses");
        let built = direct(expr);
        prop_assert_eq!(parsed.eval(&mut lookup).ok(), built.eval(&mut lookup).ok());
    }

    #[test]
    fn double_negation_of_a_bool_expression_is_the_expression(expr in bool_expr()) {
        let text = print(&expr);
        let twice = Condition::parse(&format!("!(!({text}))")).expect("it parses");
        let once = Condition::parse(&text).expect("it parses");
        prop_assert_eq!(twice.eval(&mut lookup).ok(), once.eval(&mut lookup).ok());
    }
}
