use crate::types::*;

pub trait Visitor<'bir>: Sized {
    fn map(&self) -> &'bir Map;
    fn visit(&mut self);
    fn visit_module(&mut self, mod_: &Module) {
        walk_types(self, mod_);
        walk_functions(self, mod_);
    }
    fn visit_function(&mut self, fn_: &Function) {
        walk_param_list(self, fn_);
    }
    fn visit_param(&mut self, param: &Parameter) {
        walk_param(self, param);
    }
    fn visit_typeref(&mut self, typeref: &TypeRef) {}
    fn visit_typedef(&mut self, typedef: &TypeDef) {}
}

pub fn walk_types<'bir>(v: &mut impl Visitor<'bir>, mod_: &Module) {
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
