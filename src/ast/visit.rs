use super::*;

pub trait Visitor: Sized {
    fn visit_tree(&mut self, tree: &Tree) {
        walk_tree(self, tree);
    }
    fn visit_item(&mut self, item: &Item) {
        walk_item(self, item);
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

pub fn walk_tree(v: &mut impl Visitor, tree: &Tree) {
    for item in &tree.items {
        v.visit_item(item);
    }
}

pub fn walk_item(v: &mut impl Visitor, item: &Item) {
    use ItemKind::*;
    match &item.kind {
        Function(function) => v.visit_function(function),
        Error => {}
    }
}

pub fn walk_function(v: &mut impl Visitor, function: &Function) {
    for parameter in &function.parameters {
        v.visit_parameter(parameter);
    }
    if let Some(scope) = &function.body {
        v.visit_scope(scope);
    }
    v.visit_type(&function.return_type);
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
        Error => {}
    }
}

pub fn walk_statement(v: &mut impl Visitor, statement: &Statement) {
    match &statement.kind {
        StatementKind::Declaration(Declaration {
            var,
            type_,
            initializer,
        }) => {
            v.visit_variable(var);
            if let Some(type_) = type_ {
                v.visit_type(type_);
            }
            if let Some(initializer) = initializer {
                v.visit_expression(initializer);
            }
        }
        StatementKind::Assignment { dst, src } => {
            v.visit_expression(dst);
            v.visit_expression(src);
        }
        StatementKind::Return(expr) => {
            v.visit_expression(expr);
        }
        StatementKind::Scope(scope) => {
            v.visit_scope(scope);
        }
        StatementKind::Expression(expr) => {
            v.visit_expression(expr);
        }
        StatementKind::Null => {}
        StatementKind::If { condition, block } => {
            v.visit_expression(&condition);
            v.visit_scope(&block);
        }
        StatementKind::Error => {}
    }
}
