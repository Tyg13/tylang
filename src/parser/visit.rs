use super::*;

pub trait Visitor: Sized {
    fn visit_scope(&mut self, scope: &Scope) {
        walk_scope(self, scope);
    }
    fn visit_expression(&mut self, expression: &Expression) {
        walk_expression(self, expression);
    }
    fn visit_statement(&mut self, statement: &Statement) {
        walk_statement(self, statement);
    }
    fn visit_constant(&mut self, _constant: &Constant) {}
    fn visit_variable(&mut self, _variable: &Variable) {}
}

pub fn walk_scope(v: &mut impl Visitor, scope: &Scope) {
    for statement in &scope.statements {
        Visitor::visit_statement(v, statement);
    }
}

pub fn walk_expression(v: &mut impl Visitor, expression: &Expression) {
    use ExpressionKind::*;
    match &expression.kind {
        Variable(var) => Visitor::visit_variable(v, var),
        Constant(cons) => Visitor::visit_constant(v, cons),
        Group(inner) => Visitor::visit_expression(v, &inner),
        BinaryOp { lhs, rhs, .. } => {
            Visitor::visit_expression(v, &lhs);
            Visitor::visit_expression(v, &rhs);
        }
    }
}

pub fn walk_statement(v: &mut impl Visitor, statement: &Statement) {
    use StatementKind::*;
    match &statement.kind {
        Declaration { var, initializer } => {
            Visitor::visit_variable(v, var);
            if let Some(initializer) = initializer {
                Visitor::visit_expression(v, initializer);
            }
        }
        Assignment { dst, src } => {
            Visitor::visit_expression(v, dst);
            Visitor::visit_expression(v, src);
        }
        Return(expr) => {
            Visitor::visit_expression(v, expr);
        }
        Scope(scope) => {
            Visitor::visit_scope(v, scope);
        }
        Expression(expr) => {
            Visitor::visit_expression(v, expr);
        }
        Null => {}
        If { condition, block } => {
            Visitor::visit_expression(v, &condition);
            Visitor::visit_statement(v, &block);
        }
    }
}
