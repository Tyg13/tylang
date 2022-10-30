use crate::types::{
    ConstantKind, Function, TyContext, TyID, ValueID, ValueKind, ValueRef,
    Values,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Module {
    pub functions: Vec<Function>,
    pub globals: Values,
    pub types: TyContext,

    pub(crate) void: ValueRef,

    pub(crate) vals_to_fns: HashMap<ValueID, usize>,
    pub(crate) str_constants: HashMap<ValueID, String>,
    pub(crate) int_constants: HashMap<ValueID, usize>,
}

impl Module {
    pub fn new() -> Self {
        let mut globals = Values::default();
        let mut types = TyContext::default();
        let void =
            globals.add_val(ValueKind::Void, types.get_void(), None, true);
        Module {
            globals,
            types,
            void,

            functions: Default::default(),
            vals_to_fns: Default::default(),
            str_constants: Default::default(),
            int_constants: Default::default(),
        }
    }

    pub fn add_global(
        &mut self,
        kind: ValueKind,
        ty: TyID,
        ident: Option<&str>,
    ) -> ValueRef {
        self.globals.add_val(kind, ty, ident, true)
    }

    pub fn add_str_constant(&mut self, s: String) -> ValueRef {
        let ty = self.types.get_str();
        let val =
            self.add_global(ValueKind::Constant(ConstantKind::Str), ty, None);
        self.str_constants.insert(val.id, s);
        val
    }

    pub fn add_int_constant(&mut self, n: usize, ty: TyID) -> ValueRef {
        let val =
            self.add_global(ValueKind::Constant(ConstantKind::Int), ty, None);
        self.int_constants.insert(val.id, n);
        val
    }

    pub fn add_fn(
        &mut self,
        name: &str,
        param_names: &[String],
        ty: TyID,
        is_var_args: bool,
    ) -> ValueRef {
        let val = self.add_global(ValueKind::Function, ty, Some(name));
        let idx = self.functions.len();
        let f = Function::new(
            &self.types,
            name,
            param_names,
            val.id,
            ty,
            is_var_args,
        );

        self.functions.push(f);
        self.vals_to_fns.insert(val.id, idx);

        val
    }

    pub fn fn_(&self, val: &ValueID) -> &Function {
        let idx = self.vals_to_fns[val];
        self.functions.get(idx).unwrap()
    }

    pub(crate) fn fn_mut(&mut self, val: &ValueID) -> &mut Function {
        let idx = self.vals_to_fns[val];
        self.functions.get_mut(idx).unwrap()
    }

    pub(crate) fn str_constant(&self, id: &ValueID) -> &str {
        &self.str_constants[&id]
    }

    pub(crate) fn int_constant(&self, id: &ValueID) -> usize {
        self.int_constants[&id]
    }
}
