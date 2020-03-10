use super::*;

pub trait Visitor: Sized {
    fn visit_tree(&mut self, tree: &Tree) {
        for function in &tree.functions {
            self.visit_function(function);
        }
    }
    fn visit_function(&mut self, function: &Function) {
        walk_function(self, function);
    }
    fn visit_parameter(&mut self, parameter: &Parameter) {
        walk_parameter(self, parameter);
    }
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
    fn visit_type(&mut self, _type: &Type) {}
}

pub fn walk_function(v: &mut impl Visitor, function: &Function) {
    for parameter in &function.parameters {
        v.visit_parameter(parameter);
    }
}

pub fn walk_parameter(v: &mut impl Visitor, parameter: &Parameter) {
    v.visit_variable(&parameter.variable);
    v.visit_type(&parameter.type_);
}

pub fn walk_scope(v: &mut impl Visitor, scope: &Scope) {
    for statement in &scope.statements {
        v.visit_statement(statement);
    }
}

pub fn walk_expression(v: &mut impl Visitor, expression: &Expression) {
    use ExpressionKind::*;
    match &expression.kind {
        Variable(var) => v.visit_variable(var),
        Constant(cons) => v.visit_constant(cons),
        Group(inner) => v.visit_expression(&inner),
        BinaryOp { lhs, rhs, .. } => {
            v.visit_expression(&lhs);
            v.visit_expression(&rhs);
        }
        Call { arguments, .. } => {
            for arg in arguments {
                v.visit_expression(arg);
            }
        }
    }
}

pub fn walk_statement(v: &mut impl Visitor, statement: &Statement) {
    use StatementKind::*;
    match &statement.kind {
        Declaration {
            var,
            type_,
            initializer,
        } => {
            v.visit_variable(var);
            v.visit_type(type_);
            if let Some(initializer) = initializer {
                v.visit_expression(initializer);
            }
        }
        Assignment { dst, src } => {
            v.visit_expression(dst);
            v.visit_expression(src);
        }
        Return(expr) => {
            v.visit_expression(expr);
        }
        Scope(scope) => {
            v.visit_scope(scope);
        }
        Expression(expr) => {
            v.visit_expression(expr);
        }
        Null => {}
        If { condition, block } => {
            v.visit_expression(&condition);
            v.visit_scope(&block);
        }
    }
}
