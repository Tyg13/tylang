use crate::parser;
use crate::util::{ArmPosition, Source, Span};
use std::collections::HashMap;
use std::rc::Rc;

struct Variable {
    value: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum ErrorKind {
    UndefinedVariable(String),
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ErrorKind::*;
        match self {
            UndefinedVariable(var) => write!(f, "undefined variable `{}`", var),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Error {
    span: Span,
    kind: ErrorKind,
}

type Result<T> = std::result::Result<T, Error>;

impl Interpreter<'_> {
    fn report_err(&mut self, err: Error) {
        || -> std::io::Result<()> {
            writeln!(self.out, "InterpretError: {kind}", kind = err.kind)?;
            if let Some(context) = self.source.give_context(err.span, ArmPosition::Begin) {
                writeln!(self.out, "{}", context)?;
            }
            Ok(())
        }()
        .expect("Couldn't write to interpreter out!")
    }
}

struct Interpreter<'a> {
    variables: HashMap<String, Variable>,
    tree: parser::Tree,
    source: &'a Source,
    out: &'a mut dyn std::io::Write,
}

impl<'a> Interpreter<'a> {
    fn new(tree: parser::Tree, source: &'a Source, out: &'a mut dyn std::io::Write) -> Self {
        Self {
            tree,
            source,
            out,
            variables: HashMap::new(),
        }
    }

    fn interpret(&mut self) {
        for index in 0..self.tree.statements.len() {
            let statement = self.tree.statements[index].clone();
            match self.interpret_statement(statement) {
                Err(err) => {
                    self.report_err(err);
                    break;
                }
                _ => {}
            }
        }
    }

    fn lookup(&mut self, var: &parser::Variable) -> Result<usize> {
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
    ) -> Result<usize> {
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

    fn expr_value(&mut self, expr: &parser::Expression) -> Result<usize> {
        use parser::ExpressionKind::*;
        match &expr.kind {
            Variable(var) => self.lookup(&var),
            Constant(cons) => Ok(cons.value),
            Group(expr) => self.expr_value(expr),
            BinaryOp { kind, lhs, rhs } => self.binary_op(*kind, lhs, rhs),
        }
    }

    fn interpret_statement(&mut self, statement: parser::Statement) -> Result<()> {
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
                        return Err(Error {
                            span: statement.span,
                            kind: ErrorKind::UndefinedVariable(identifier.clone()),
                        });
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

pub fn interpret(tree: parser::Tree, source: &Source, out: &mut dyn std::io::Write) {
    Interpreter::new(tree, source, out).interpret()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::SourceBuilder;
    use crate::{binary_op, expr_con, expr_var, span, stmt, var};

    macro_rules! null_span { () => (span!(0:0, 0:0)); }
    macro_rules! stmt {
        ($kind:ident $($args:tt),*) => {
            $crate::stmt!(null_span!(), $kind $($args),*);
        };
    }
    macro_rules! var {
        ($ident:expr) => {
            $crate::var!(null_span!(), $ident);
        };
    }
    macro_rules! expr_var {
        ($ident:expr) => {
            $crate::expr_var!(null_span!(), $ident);
        };
    }
    macro_rules! expr_con {
        ($value:expr) => {
            $crate::expr_con!(null_span!(), $value);
        };
    }
    macro_rules! binary_op {
        ($op:ident, $lhs:expr, $rhs:expr) => {
            $crate::binary_op!(null_span!(), $op, $lhs, $rhs);
        };
    }

    macro_rules! assert_output {
        ($expected:expr, $statements:expr$(, $rest:tt)*) => {{
            let tree = parser::Tree {
                statements: $statements,
            };
            let mut buff = Vec::new();
            interpret(tree, &SourceBuilder::new().build(), &mut buff);
            let out = String::from_utf8(buff).expect("Non UTF-8 interpreter output!");
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
                stmt!(Print(expr_var!("x"))),
            ]
        );
        assert_output!(
            "10\n",
            vec![
                stmt!(Declaration {
                    var: var!("x"),
                    initializer: Some(expr_con!(10)),
                }),
                stmt!(Print(expr_var!("x"))),
            ]
        );
    }
    #[test]
    fn interpret_binary_op() {
        assert_output!(
            "20\n",
            vec![stmt!(Print(binary_op!(Add, expr_con!(10), expr_con!(10))))]
        );
        assert_output!(
            "20\n",
            vec![stmt!(Print(binary_op!(Add, expr_con!(10), expr_con!(10))))]
        );
    }
}
