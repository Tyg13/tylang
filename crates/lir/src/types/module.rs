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

    pub(crate) void: ValueID,

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

    fn add_global(
        globals: &mut Values,
        kind: ValueKind,
        ty: TyID,
        ident: Option<String>,
    ) -> ValueID {
        globals.add_val(kind, ty, ident, true)
    }

    pub fn add_str_constant(&mut self, s: String) -> ValueID {
        let id = Self::add_global(
            &mut self.globals,
            ValueKind::Constant(ConstantKind::Str),
            self.types.get_str(),
            None,
        );
        self.str_constants.insert(id, s);
        id
    }

    pub fn add_int_constant(&mut self, n: usize, ty: TyID) -> ValueID {
        let id = Self::add_global(
            &mut self.globals,
            ValueKind::Constant(ConstantKind::Int),
            ty,
            None,
        );
        self.int_constants.insert(id, n);
        id
    }

    pub fn add_fn(
        &mut self,
        name: String,
        param_names: Vec<String>,
        ty: TyID,
        internal: bool,
    ) -> ValueID {
        let id = Self::add_global(
            &mut self.globals,
            ValueKind::Function,
            ty,
            Some(name.clone()),
        );
        let idx = self.functions.len();
        let f = Function::new(&self.types, name, param_names, internal, id, ty);

        self.functions.push(f);
        self.vals_to_fns.insert(id, idx);

        id
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
