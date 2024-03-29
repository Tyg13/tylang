use crate::types::*;

pub trait Visitor<'bir>: Sized {
    fn map(&self) -> &'bir Map;
    fn visit_root(&mut self) {
        self.visit_module(self.map().root_module())
    }
    fn visit_module(&mut self, mod_: &Module) {
        walk_module(self, mod_);
    }
    fn visit_import(&mut self, _: &Import) {}
    fn visit_function(&mut self, fn_: &Function) {
        walk_param_list(self, fn_);
    }
    fn visit_param(&mut self, param: &Parameter) {
        walk_param(self, param);
    }
    fn visit_block(&mut self, scope: &Block) {
        walk_block(self, scope);
    }
    fn visit_item(&mut self, item: &Item) {
        walk_item(self, item);
    }
    fn visit_let(&mut self, _let: &Let) {}
    fn visit_expr(&mut self, expr: &Expr) {
        walk_expr(self, expr);
    }
    fn visit_typeref(&mut self, _: &TypeRef) {}
    fn visit_typedef(&mut self, _: &TypeDef) {}
    fn visit_name(&mut self, _: &Name) {}
}

pub fn walk_module<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
    walk_imports(v, mod_);
    walk_modules(v, mod_);
    walk_typedefs(v, mod_);
    walk_functions(v, mod_);
}

pub fn walk_imports<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
    for import in mod_.imports(v.map()) {
        v.visit_import(import);
    }
}

pub fn walk_modules<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
    for mod_ in mod_.modules(v.map()) {
        v.visit_module(mod_);
    }
}

pub fn walk_typedefs<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
    for fn_ in mod_.typedefs(v.map()) {
        v.visit_typedef(fn_);
    }
}

pub fn walk_functions<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
    for fn_ in mod_.functions(v.map()) {
        v.visit_function(fn_);
    }
}

pub fn walk_param_list<'bir>(v: &mut impl Visitor<'bir>, fn_: &Function) {
    for param in fn_.parameters(v.map()) {
        v.visit_param(param);
    }
}

pub fn walk_param<'bir>(v: &mut impl Visitor<'bir>, param: &Parameter) {
    v.visit_typeref(param.ty(v.map()));
}

pub fn walk_item<'bir>(v: &mut impl Visitor<'bir>, item: &Item) {
    match item.kind {
        ItemKind::Let(id) => v.visit_let(v.map().let_(&id)),
        ItemKind::Expr(id) => v.visit_expr(v.map().expr(&id)),
    }
}

pub fn walk_expr<'bir>(v: &mut impl Visitor<'bir>, expr: &Expr) {
    match expr.kind {
        ExprKind::NameRef { id: name } => v.visit_name(v.map().name(&name)),
        _ => {}
    }
}

pub fn walk_block<'bir>(v: &mut impl Visitor<'bir>, scope: &Block) {
    for item in scope.items(v.map()) {
        v.visit_item(item);
    }
    if let Some(expr) = scope.return_expr(v.map()) {
        v.visit_expr(expr);
    }
}
