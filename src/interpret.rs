use crate::parser;
use crate::util::Span;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

struct Variable {
    value: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum ErrorKind {
    UndefinedVariable(String),
    Io(String),
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ErrorKind::*;
        match self {
            UndefinedVariable(var) => write!(f, "undefined variable `{}`", var),
            Io(err) => write!(f, "{}", err),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Error {
    span: Span,
    kind: ErrorKind,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InterpretError at {span}: {kind}",
            span = self.span,
            kind = self.kind,
        )
    }
}

struct Interpreter<'a> {
    variables: HashMap<String, Variable>,
    tree: parser::Tree,
    out: &'a mut dyn io::Write,
}

impl<'a> Interpreter<'a> {
    fn new(tree: parser::Tree, out: &'a mut dyn io::Write) -> Self {
        Self {
            tree,
            out,
            variables: HashMap::new(),
        }
    }

    fn interpret(&mut self) {
        for index in 0..self.tree.statements.len() {
            let statement = self.tree.statements[index].clone();
            match self.interpret_statement(statement) {
                Err(err) => {
                    println!("{}", err);
                    break;
                }
                _ => {}
            }
        }
    }

    fn lookup(&mut self, var: &parser::Variable) -> Result<usize, Error> {
        match self.variables.get(&var.identifier) {
            Some(var) => Ok(var.value),
            None => Err(Error {
                span: var.span,
                kind: ErrorKind::UndefinedVariable(var.identifier.clone()),
            }),
        }
    }

    fn binary_op(
        &mut self,
        kind: parser::BinaryOpKind,
        lhs: &Rc<parser::Expression>,
        rhs: &Rc<parser::Expression>,
    ) -> Result<usize, Error> {
        use parser::BinaryOpKind::*;
        let lhs = self.expr_value(&*lhs)?;
        let rhs = self.expr_value(&*rhs)?;
        Ok(match kind {
            Add => lhs + rhs,
            Sub => lhs - rhs,
            Mul => lhs * rhs,
            Div => lhs / rhs,
        })
    }

    fn expr_value(&mut self, expr: &parser::Expression) -> Result<usize, Error> {
        use parser::ExpressionKind::*;
        match &expr.kind {
            Variable(var) => self.lookup(&var),
            Constant(cons) => Ok(cons.value),
            Group(expr) => self.expr_value(expr),
            BinaryOp { kind, lhs, rhs } => self.binary_op(*kind, lhs, rhs),
        }
    }

    fn interpret_statement(&mut self, statement: parser::Statement) -> Result<(), Error> {
        use parser::StatementKind::*;
        match statement.kind.clone() {
            Declaration {
                var: parser::Variable { identifier, .. },
                initializer,
            } => {
                let value = match initializer {
                    Some(initializer) => self.expr_value(&initializer)?,
                    None => 0,
                };
                self.variables.insert(identifier, Variable { value });
            }
            Assignment {
                dst:
                    parser::Expression {
                        kind: parser::ExpressionKind::Variable(dst),
                        ..
                    },
                src,
            } => {
                let value = self.expr_value(&src)?;
                let identifier = &dst.identifier;
                match self.variables.get_mut(identifier) {
                    Some(var) => var.value = value,
                    None => {
                        write!(
                            self.out,
                            "Error cannot assign undeclared variable {var} at ({line}, {column})",
                            var = identifier,
                            line = dst.span.start.line,
                            column = dst.span.start.column,
                        )
                        .map_err(|e| Error {
                            span: statement.span,
                            kind: ErrorKind::Io(e.to_string()),
                        })?;
                    }
                }
            }
            Print(expr) => {
                let val = self.expr_value(&expr)?;
                writeln!(self.out, "{}", val);
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn interpret<'a>(tree: parser::Tree, out: &'a mut dyn io::Write) {
    Interpreter::new(tree, out).interpret()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{self, *};
    use crate::span;

    macro_rules! null_span { () => (span!(0:0, 0:0)); }
    macro_rules! stmt {
        ($kind:ident $($args:tt)*) => {
            parser::Statement {
                span: null_span!(),
                kind: StatementKind::$kind $($args)*,
            }
        };
    }
    macro_rules! expr {
        ($kind:ident $($args:tt)*) => {
            parser::Expression {
                span: null_span!(),
                kind: ExpressionKind::$kind $($args)*,
            }
        };
    }
    macro_rules! var {
        ($ident:expr) => {
            parser::Variable {
                span: null_span!(),
                identifier: String::from($ident),
            }
        };
    }
    macro_rules! con {
        ($value:expr) => {
            parser::Constant {
                span: null_span!(),
                value: $value,
            }
        };
    }

    macro_rules! binary_op {
        ($op:ident, $lhs:expr, $rhs:expr) => {
            expr!(BinaryOp {
                kind: BinaryOpKind::$op,
                lhs: Rc::new($lhs),
                rhs: Rc::new($rhs),
            })
        };
    }

    macro_rules! assert_output {
        ($expected:expr, $statements:expr$(, $rest:tt)*) => {{
            let tree = Tree {
                statements: $statements,
            };
            let mut buff = Vec::new();
            interpret(tree, &mut buff);
            let out = String::from_utf8(buff).expect("Non UTF-8 parser output!");
            assert_eq!($expected, out$(, $rest)*);
        }};
    }

    #[test]
    fn interpret_declaration() {
        assert_output!(
            "0\n",
            vec![
                stmt!(Declaration {
                    var: var!("x"),
                    initializer: None,
                }),
                stmt!(Print(expr!(Variable(var!("x"))))),
            ]
        );
        assert_output!(
            "10\n",
            vec![
                stmt!(Declaration {
                    var: var!("x"),
                    initializer: Some(expr!(Constant(con!(10)))),
                }),
                stmt!(Print(expr!(Variable(var!("x"))))),
            ]
        );
    }
    #[test]
    fn interpret_binary_op() {
        assert_output!(
            "20\n",
            vec![stmt!(Print(binary_op!(
                Add,
                expr!(Constant(con!(10))),
                expr!(Constant(con!(10)))
            )))]
        );
        assert_output!(
            "20\n",
            vec![stmt!(Print(binary_op!(
                Add,
                expr!(Constant(con!(10))),
                expr!(Constant(con!(10)))
            )))]
        );
    }
}
