use std::collections::HashMap;

use crate::types::*;
use crate::utils::lookup_name;

#[derive(Debug)]
pub struct Builder<'ctx> {
    sess: &'ctx Session<'ctx, 'ctx>,
    current_module: Option<Module>,
    current_function: Option<Function>,

    ns_stack: Vec<sema::ID>,
    vars: HashMap<sema::ID, ValueRef>,
    void_val: Option<ValueID>,
}

#[derive(Debug)]
pub struct Session<'bir, 'sema> {
    pub(crate) bir: &'bir bir::Map,
    pub(crate) sema: &'sema sema::Map,
}

impl Session<'_, '_> {
    pub fn bir_to_sema(&self, id: bir::ID) -> sema::ID {
        self.sema.bir_to_id(id).unwrap()
    }
}

impl<'s> Builder<'s> {
    pub fn new(sess: &'s Session) -> Self {
        Builder {
            sess,
            current_module: None,
            current_function: None,
            ns_stack: Vec::new(),
            vars: Default::default(),
            void_val: None,
        }
    }

    pub fn new_module(&mut self, sema: sema::ID) {
        self.current_module = Some(Module::new());
        self.push_ns(sema);
    }

    pub fn finish_module(&mut self) -> Module {
        self.pop_ns();
        self.current_module.take().unwrap()
    }

    pub fn new_function(
        &mut self,
        name: &str,
        id: sema::ID,
        params: Vec<sema::ID>,
        sema: &sema::Map,
    ) {
        self.current_function = Some(Function::new(id, name.to_string(), params, sema));
        self.mod_mut()
            .add_global(ValueKind::Function, Some(id), Some(name.to_string()));
        self.push_ns(id);
    }

    pub fn finish_function(&mut self) {
        self.pop_ns();
        let f = self.current_function.take().unwrap();
        self.current_module.as_mut().unwrap().functions.push(f);
    }

    pub fn current_ns(&self) -> sema::ID {
        self.ns_stack.last().copied().unwrap()
    }

    pub fn push_ns(&mut self, sema: sema::ID) {
        debug_assert!(self.sess.sema.ns(sema).is_some());
        self.ns_stack.push(sema);
    }

    pub fn pop_ns(&mut self) {
        self.ns_stack.pop();
    }

    pub fn void_(&mut self) -> ValueRef {
        ValueRef::local(self.void_val.unwrap())
    }

    pub fn set_void(&mut self, sema: sema::ID) {
        debug_assert_eq!(self.void_val, None);
        self.void_val = Some(
            self.mod_mut()
                .add_global(ValueKind::Void, Some(sema), None)
                .id,
        );
    }

    pub fn ctx(&self) -> Context<'_> {
        Context {
            m: self.current_module.as_ref(),
            f: self.current_function.as_ref(),
        }
    }

    fn mod_(&self) -> &Module {
        self.current_module.as_ref().unwrap()
    }

    fn fn_(&self) -> &Function {
        self.current_function.as_ref().unwrap()
    }

    fn mod_mut(&mut self) -> &mut Module {
        self.current_module.as_mut().unwrap()
    }

    fn fn_mut(&mut self) -> &mut Function {
        self.current_function.as_mut().unwrap()
    }

    pub(crate) fn val_from_sema(&self, sema: &sema::ID, global: bool) -> Option<&ValueRef> {
        match global {
            true => self.mod_().val(sema),
            false => self.fn_().val(sema),
        }
    }

    pub fn new_constant(&mut self, sema: sema::ID) -> ValueRef {
        self.fn_mut().add_val(ValueKind::Constant, Some(sema), None)
    }

    fn add_inst(
        &mut self,
        sema: sema::ID,
        kind: InstKind,
        lval: Option<ValueRef>,
        rvals: Vec<ValueRef>,
        ident: Option<String>,
    ) -> ValueRef {
        let ident = ident.or_else(|| self.sess.sema.name(sema).map(|name| name.ident.clone()));
        let lval = {
            if kind.can_have_lvals() {
                lval.or_else(|| {
                    let val_is_void = self.sess.sema.ty(sema).unwrap().is_void();
                    if !val_is_void {
                        Some(
                            self.fn_mut()
                                .add_val(ValueKind::Inst, Some(sema), ident.clone()),
                        )
                    } else {
                        None
                    }
                })
            } else {
                debug_assert_eq!(lval, None);
                None
            }
        };
        let inst_val = self.fn_mut().add_inst(Some(sema), kind, lval, rvals, ident);
        let val = lval.unwrap_or(inst_val);
        match kind {
            InstKind::Var => {
                self.vars.insert(sema, val);
            }
            _ => {}
        };
        val
    }

    pub fn new_inst(&mut self, kind: InstKind) -> InstBuilder<'_, 's> {
        InstBuilder::new(self, kind)
    }

    pub fn new_return(&mut self, sema: sema::ID, val: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Return).sema(sema).with_rval(val)
    }

    pub fn new_copy(&mut self, val: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Copy).with_rval(val)
    }

    pub fn new_cast(&mut self, val: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Cast).with_rval(val)
    }

    pub fn new_var(&mut self, sema: sema::ID) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Var).sema(sema)
    }

    pub fn new_offset(&mut self, base: ValueRef, offset: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Offset).with_rvals(&[base, offset])
    }

    pub fn new_load(&mut self, base: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Load).with_rval(base)
    }

    pub fn new_store(&mut self, base: ValueRef, val: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Store)
            .with_lval(base)
            .with_rval(val)
    }

    pub fn new_add(&mut self, lhs: ValueRef, rhs: ValueRef) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Add).with_rvals(&[lhs, rhs])
    }

    pub fn new_call(&mut self, fn_: ValueRef, ops: Vec<ValueRef>) -> InstBuilder<'_, 's> {
        self.new_inst(InstKind::Call).with_rval(fn_).add_rvals(ops)
    }
}

pub struct InstBuilder<'builder, 'ctx> {
    builder: &'builder mut Builder<'ctx>,
    kind: InstKind,
    sema: Option<sema::ID>,
    name: Option<String>,
    lval: Option<ValueRef>,
    rvals: Vec<ValueRef>,
}

impl<'b, 'ctx> InstBuilder<'b, 'ctx> {
    #[must_use]
    fn new(builder: &'b mut Builder<'ctx>, kind: InstKind) -> Self {
        Self {
            builder,
            kind,
            sema: None,
            name: None,
            lval: None,
            rvals: Vec::new(),
        }
    }
}

impl InstBuilder<'_, '_> {
    pub fn sema(self, v: sema::ID) -> Self {
        debug_assert!(self.sema.is_none());
        Self {
            sema: Some(v),
            ..self
        }
    }

    pub fn named(self, v: String) -> Self {
        debug_assert!(self.name.is_none());
        Self {
            name: Some(v),
            ..self
        }
    }

    pub fn with_lval(self, v: ValueRef) -> Self {
        debug_assert!(self.lval.is_none());
        debug_assert!(self.kind.can_have_lvals());
        Self {
            lval: Some(v),
            ..self
        }
    }

    pub fn with_maybe_lval(self, v: Option<ValueRef>) -> Self {
        if let Some(v) = v {
            self.with_lval(v)
        } else {
            self
        }
    }

    fn add_rvals(self, mut v: Vec<ValueRef>) -> Self {
        let mut rvals = self.rvals;
        rvals.append(&mut v);
        Self { rvals, ..self }
    }

    fn with_rval(self, v: ValueRef) -> Self {
        debug_assert!(self.rvals.is_empty());
        Self {
            rvals: vec![v],
            ..self
        }
    }

    fn with_rvals(self, v: &[ValueRef]) -> Self {
        debug_assert!(self.rvals.is_empty());
        Self {
            rvals: v.to_vec(),
            ..self
        }
    }

    pub fn build(self) -> ValueRef {
        if self.kind == InstKind::Call {
            debug_assert!(!self.rvals.is_empty());
        } else {
            if self.lval.is_some() {
                debug_assert!(self.kind.can_have_lvals());
            }
            if !self.kind.num_rvals().contains(&self.rvals.len()) {
                dbg!(self.kind, self.kind.num_rvals(), self.rvals.len());
                debug_assert!(false);
            }
        }
        let sema = self.sema.expect("no sema set?");
        self.builder
            .add_inst(sema, self.kind, self.lval, self.rvals, self.name)
    }
}
