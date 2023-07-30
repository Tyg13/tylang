use assert_matches::debug_assert_matches;
use std::collections::HashMap;

use crate::types::*;

#[derive(Debug)]
pub struct Builder<'ctx, 'm> {
    pub(crate) sess: &'ctx mut Session<'ctx, 'ctx>,
    pub(crate) module: &'m mut Module,

    current_function: Option<ValueRef>,
    current_block: Option<Block>,

    int_constants: HashMap<usize, ValueID>,
    str_constants: HashMap<String, ValueID>,

    unresolved_breaks: Vec<BreakPH>,
}

impl<'s, 'm> Builder<'s, 'm> {
    pub fn new(sess: &'s mut Session<'s, 's>, module: &'m mut Module) -> Self {
        Builder {
            sess,
            module,
            current_function: None,
            current_block: None,
            int_constants: Default::default(),
            str_constants: Default::default(),
            unresolved_breaks: Default::default(),
        }
    }

    pub fn new_function<S: ToString>(
        &mut self,
        name: S,
        param_names: impl IntoIterator<Item = S>,
        return_ty: TyID,
        params: Vec<TyID>,
        is_var_args: bool,
        internal: bool,
    ) -> ValueID {
        let ty = self.module.types.get_fn(
            is_var_args,
            &return_ty,
            params.as_slice(),
        );
        self.module.add_fn(
            name.to_string(),
            param_names.into_iter().map(|n| n.to_string()).collect(),
            ty,
            internal,
        )
    }

    pub fn enter_function(&mut self, bir: bir::ID) {
        self.current_function = Some(self.sess.val_from_bir(&bir));
    }

    pub fn exit_function(&mut self, bir: bir::ID) {
        let fn_ = self.current_function.take();
        debug_assert_eq!(fn_, Some(self.sess.val_from_bir(&bir)));
    }

    pub fn void_(&self) -> ValueRef {
        let val = ValueRef::new(self.ctx().as_mod().void);
        debug_assert_eq!(val.kind(self.ctx()), ValueKind::Void);
        val
    }

    pub fn new_undef(&mut self, ty: TyID) -> ValueRef {
        ValueRef::new(self.fn_mut().add_val(ValueKind::Undef, ty, None))
    }

    pub fn void_ty(&self) -> TyID {
        self.ctx().types().void().id
    }

    pub fn ctx(&self) -> Context<'_> {
        Context {
            m: Some(&self.module),
            f: Some(self.fn_()),
        }
    }

    pub fn ctx_mut(&mut self) -> ContextMut<'_> {
        ContextMut::full(&mut self.module, self.current_function.unwrap().id)
    }

    fn fn_(&self) -> &Function {
        let id = self.current_function.unwrap().id;
        self.module.fn_(&id)
    }

    fn fn_mut(&mut self) -> &mut Function {
        let id = self.current_function.unwrap().id;
        self.module.fn_mut(&id)
    }

    pub fn new_int_constant(&mut self, n: usize, ty: TyID) -> ValueRef {
        let id = *self
            .int_constants
            .entry(n)
            .or_insert_with(|| self.module.add_int_constant(n, ty));
        ValueRef::new(id)
    }

    pub fn new_str_constant(&mut self, s: impl ToString) -> ValueRef {
        let s = s.to_string();
        let id = *self
            .str_constants
            .entry(s)
            .or_insert_with_key(|s| self.module.add_str_constant(s.clone()));
        ValueRef::new(id)
    }

    fn new_block_impl<'a, S: Into<String>>(
        &mut self,
        label: Option<S>,
    ) -> Block {
        let void_ty = self.void_ty();
        let block = self.fn_mut().add_block(label.map(|i| i.into()), void_ty);
        self.current_block = Some(block);
        block
    }

    pub fn new_block(&mut self) -> Block {
        self.new_block_impl::<&str>(None)
    }

    pub fn new_labeled_block(&mut self, label: &str) -> Block {
        self.new_block_impl(Some(label))
    }

    pub fn block_from_label(&self, label: &str) -> Block {
        let f = self.fn_();
        f.blocks_by_label[label]
    }

    pub fn current_block(&self) -> Block {
        self.current_block.unwrap()
    }

    fn add_temp<S: Into<String>>(
        &mut self,
        ty: TyID,
        ident: Option<S>,
    ) -> ValueRef {
        ValueRef::new(self.fn_mut().add_val(
            ValueKind::Inst,
            ty,
            ident.map(|i| i.into()),
        ))
    }

    fn add_inst<S: Into<String>>(
        &mut self,
        kind: InstKind,
        ty: TyID,
        lval: Option<ValueRef>,
        rvals: Vec<ValueRef>,
        ident: Option<S>,
    ) -> ValueRef {
        let current_block = self.current_block.expect("inst without block?");
        let inst_val = self.fn_mut().add_inst(
            kind,
            ty,
            current_block,
            lval,
            rvals,
            ident.map(|i| i.into()),
        );
        lval.unwrap_or(inst_val)
    }

    fn assert_lval_expr(&self, val: ValueRef) {
        debug_assert_matches!(
            val.kind(self.ctx()),
            ValueKind::Param | ValueKind::Inst
        );
    }

    fn assert_rval_expr(&self, val: ValueRef) {
        debug_assert_matches!(
            val.kind(self.ctx()),
            ValueKind::Param | ValueKind::Constant(..) | ValueKind::Inst
        );
    }

    fn assert_rval_exprs(&self, vals: &[ValueRef]) {
        for v in vals {
            self.assert_rval_expr(*v);
        }
    }

    pub fn new_inst(&mut self, kind: InstKind) -> InstBuilder<'_, 's, 'm> {
        InstBuilder::new(self, kind)
    }

    pub fn new_return(&mut self, val: ValueRef) -> InstBuilder<'_, 's, 'm> {
        debug_assert_matches!(
            val.kind(self.ctx()),
            ValueKind::Param
                | ValueKind::Constant(..)
                | ValueKind::Inst
                | ValueKind::Void
        );
        self.new_inst(InstKind::Return).with_rval(val.dup())
    }

    pub fn new_copy(&mut self, val: ValueRef) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(val);
        self.new_inst(InstKind::Copy).with_rval(val.dup())
    }

    pub fn new_cast(&mut self, val: ValueRef) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(val);
        self.new_inst(InstKind::Cast).with_rval(val.dup())
    }

    pub fn new_var(&mut self) -> InstBuilder<'_, 's, 'm> {
        self.new_inst(InstKind::Var)
    }

    pub fn new_load(&mut self, base: ValueRef) -> InstBuilder<'_, 's, 'm> {
        self.new_inst(InstKind::Load).with_rval(base.dup())
    }

    pub fn new_store(
        &mut self,
        base: ValueRef,
        val: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_lval_expr(base);
        self.assert_rval_expr(val);
        self.new_inst(InstKind::Store)
            .with_lval(base.dup())
            .with_rval(val.dup())
    }

    pub fn new_subscript(
        &mut self,
        base: ValueRef,
        offsets: &[ValueRef],
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_lval_expr(base);
        self.assert_rval_exprs(offsets);
        self.new_inst(InstKind::Subscript)
            .with_rval(base.dup())
            .add_rvals(offsets.into_iter().map(|op| op.dup()))
    }

    pub fn new_get_field(
        &mut self,
        base: ValueRef,
        offsets: &[ValueRef],
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_lval_expr(base);
        self.assert_rval_exprs(offsets);
        self.new_inst(InstKind::GetField)
            .with_rval(base.dup())
            .add_rvals(offsets.into_iter().map(|op| op.dup()))
    }

    pub fn new_add(
        &mut self,
        lhs: ValueRef,
        rhs: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(lhs);
        self.assert_rval_expr(rhs);
        self.new_inst(InstKind::Add)
            .with_rvals(&[lhs.dup(), rhs.dup()])
    }

    pub fn new_sub(
        &mut self,
        lhs: ValueRef,
        rhs: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(lhs);
        self.assert_rval_expr(rhs);
        self.new_inst(InstKind::Sub)
            .with_rvals(&[lhs.dup(), rhs.dup()])
    }

    pub fn new_mul(
        &mut self,
        lhs: ValueRef,
        rhs: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(lhs);
        self.assert_rval_expr(rhs);
        self.new_inst(InstKind::Mul)
            .with_rvals(&[lhs.dup(), rhs.dup()])
    }

    pub fn new_div(
        &mut self,
        lhs: ValueRef,
        rhs: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(lhs);
        self.assert_rval_expr(rhs);
        self.new_inst(InstKind::Div)
            .with_rvals(&[lhs.dup(), rhs.dup()])
    }

    pub fn new_cmp(
        &mut self,
        kind: CmpKind,
        lhs: ValueRef,
        rhs: ValueRef,
    ) -> InstBuilder<'_, 's, 'm> {
        self.assert_rval_expr(lhs);
        self.assert_rval_expr(rhs);
        self.new_inst(InstKind::Cmp { kind })
            .with_rvals(&[lhs.dup(), rhs.dup()])
    }

    pub fn new_call(
        &mut self,
        fn_: ValueRef,
        ops: Vec<ValueRef>,
    ) -> InstBuilder<'_, 's, 'm> {
        debug_assert_eq!(fn_.kind(self.ctx()), ValueKind::Function);
        self.assert_rval_exprs(&ops);
        self.new_inst(InstKind::Call)
            .with_rval(fn_.dup())
            .add_rvals(ops.into_iter().map(|op| op.dup()))
    }

    pub fn new_jump_marker(&mut self) -> Marker {
        let void_ = self.void_();
        let val = self
            .new_inst(InstKind::Jmp)
            .with_rval(void_)
            .void_ty()
            .build();
        let block = self.current_block();
        Marker {
            block,
            val,
            kind: MarkerKind::Jump,
        }
    }

    pub fn new_jump(&mut self, dst: Block) -> ValueRef {
        let current_block = self.current_block();
        self.fn_mut().add_block_edge(current_block, dst);

        let dst = dst.val(self.fn_()).dup();
        self.new_inst(InstKind::Jmp)
            .with_rval(dst)
            .void_ty()
            .build()
    }

    pub fn new_branch_marker(&mut self) -> Marker {
        let void_ = self.void_();
        let val = self
            .new_inst(InstKind::Branch)
            .with_rvals(&[void_, void_, void_])
            .void_ty()
            .build();
        let block = self.current_block();
        Marker {
            block,
            val,
            kind: MarkerKind::Branch,
        }
    }

    pub fn new_break(&mut self, label: String) -> ValueRef {
        let marker = self.new_jump_marker();
        let val = marker.val;
        self.unresolved_breaks.push(BreakPH { label, marker });
        val
    }
}

#[derive(Debug)]
struct BreakPH {
    label: String,
    marker: Marker,
}

#[derive(Debug)]
pub struct Session<'bir, 'sema> {
    pub(crate) bir: &'bir bir::Map,
    pub(crate) sema: &'sema sema::Map,
    pub(crate) ty_mapping: TyMapping,
    pub(crate) value_mapping: ValueMapping,
}

impl<'bir, 'sema> Session<'bir, 'sema> {
    pub fn new(bir: &'bir bir::Map, sema: &'sema sema::Map) -> Self {
        Self {
            bir,
            sema,
            ty_mapping: TyMapping::default(),
            value_mapping: ValueMapping::default(),
        }
    }
}

impl Session<'_, '_> {
    pub fn bir_to_sema(&self, id: &bir::ID) -> sema::ID {
        self.sema
            .bir_to_id(id)
            .expect(&format!("no sema for bir {id:?}"))
    }

    pub fn sema_to_ty(&self, id: &sema::ID) -> TyID {
        let ty = self.sema.ty_id(*id).unwrap();
        self.ty_mapping.get(&ty)
    }

    pub(crate) fn val_from_sema(&self, sema: &sema::ID) -> ValueRef {
        self.value_mapping.try_get(sema).expect(&format!(
            "no mapped value for {sema:?} ({:?})",
            self.sema.kind(*sema)
        ))
    }

    pub(crate) fn val_from_bir(&self, bir: &bir::ID) -> ValueRef {
        let sema = self.bir_to_sema(bir);
        self.val_from_sema(&sema)
    }

    pub(crate) fn ty_from_bir(&self, bir: &bir::ID) -> TyID {
        let sema = self.bir_to_sema(bir);
        self.sema_to_ty(&sema)
    }
}

#[derive(Default, Debug)]
pub struct TyMapping {
    data: HashMap<sema::ID, TyID>,
}

impl TyMapping {
    pub fn insert(&mut self, sema: sema::ID, id: TyID) {
        self.data.insert(sema, id);
    }

    pub fn try_get(&self, sema: &sema::ID) -> Option<TyID> {
        self.data.get(sema).copied()
    }

    pub fn get(&self, sema: &sema::ID) -> TyID {
        self.try_get(sema)
            .expect(&format!("no mapped type for {sema:?}"))
    }
}

#[derive(Debug, Default)]
pub struct ValueMapping {
    data: HashMap<sema::ID, ValueID>,
}

impl ValueMapping {
    pub fn try_get(&self, id: &sema::ID) -> Option<ValueRef> {
        self.data.get(id).map(|i| ValueRef::new(*i))
    }

    pub fn insert(&mut self, sema: sema::ID, val: ValueID) {
        self.data.insert(sema, val);
    }
}

pub struct InstBuilder<'builder, 'ctx, 'm> {
    builder: &'builder mut Builder<'ctx, 'm>,
    kind: InstKind,
    ty: Option<TyID>,
    name: Option<String>,
    lval: Option<ValueRef>,
    rvals: Vec<ValueRef>,

    should_create_lval: bool,
}

impl<'b, 'ctx, 'm> InstBuilder<'b, 'ctx, 'm> {
    #[must_use]
    fn new(builder: &'b mut Builder<'ctx, 'm>, kind: InstKind) -> Self {
        Self {
            builder,
            kind,
            ty: None,
            name: None,
            lval: None,
            rvals: Vec::new(),
            should_create_lval: false,
        }
    }
}

impl InstBuilder<'_, '_, '_> {
    pub fn of_ty(mut self, v: TyID) -> Self {
        debug_assert!(self.ty.is_none());
        self.ty = Some(v);
        self
    }

    pub fn void_ty(self) -> Self {
        let void_ = self.builder.void_ty();
        self.of_ty(void_)
    }

    pub fn named(mut self, v: impl Into<String>) -> Self {
        debug_assert!(self.name.is_none());
        self.name = Some(v.into());
        self
    }

    pub fn with_lval(mut self, v: ValueRef) -> Self {
        debug_assert!(self.lval.is_none());
        debug_assert!(self.kind.can_have_lvals());
        self.lval = Some(v.dup());
        self
    }

    pub fn with_lval_or_new(self, v: Option<ValueRef>) -> Self {
        if let Some(v) = v {
            self.with_lval(v)
        } else {
            self.with_new_lval()
        }
    }

    pub fn with_new_lval(mut self) -> Self {
        self.should_create_lval = true;
        self
    }

    fn add_rvals(mut self, v: impl Iterator<Item = ValueRef>) -> Self {
        self.rvals.extend(v);
        self
    }

    fn with_rval(mut self, v: ValueRef) -> Self {
        debug_assert!(self.rvals.is_empty());
        self.rvals = vec![v];
        self
    }

    fn with_rvals(mut self, v: &[ValueRef]) -> Self {
        debug_assert!(self.rvals.is_empty());
        self.rvals = v.to_vec();
        self
    }

    pub fn build(self) -> ValueRef {
        if self.kind == InstKind::Call {
            debug_assert!(!self.rvals.is_empty());
        }
        if self.lval.is_some() {
            debug_assert!(self.kind.can_have_lvals());
        }
        if !self.kind.num_rvals().contains(&self.rvals.len()) {
            dbg!(self.kind, self.kind.num_rvals(), self.rvals.len());
            debug_assert!(false);
        }
        let ty = self.ty.unwrap();
        let lval = if self.should_create_lval {
            debug_assert_eq!(self.lval, None);
            Some(self.builder.add_temp(ty, self.name.as_deref()))
        } else {
            self.lval
        };
        self.builder.add_inst(
            self.kind,
            ty,
            lval,
            self.rvals,
            self.name.as_deref(),
        )
    }
}

#[derive(Debug)]
pub struct Marker {
    block: Block,
    val: ValueRef,
    kind: MarkerKind,
}

#[derive(Debug, PartialEq, Eq)]
enum MarkerKind {
    Jump,
    Branch,
}

impl<'s> Builder<'s, '_> {
    pub fn resolve_jump(&mut self, m: Marker, dst: Block) -> ValueRef {
        debug_assert_eq!(m.kind, MarkerKind::Jump);

        self.fn_mut().add_block_edge(m.block, dst);

        let dst = dst.val(self.fn_()).dup().with_parent(m.val.id);

        let inst = m.val.inst_mut(self.fn_mut());
        let jmp = inst.val;
        let old_dst = std::mem::replace(&mut inst.rvals[0], dst);
        debug_assert_eq!(old_dst.kind(self.ctx()), ValueKind::Void);
        jmp
    }

    pub fn resolve_branch(
        &mut self,
        m: Marker,
        cond: ValueRef,
        then: Block,
        alt: Block,
    ) -> ValueRef {
        debug_assert_eq!(m.kind, MarkerKind::Branch);

        self.assert_rval_expr(cond);
        let cond = cond.with_parent(m.val.id);
        self.fn_mut().locals.add_user(cond.id, m.val.id);

        let then = then.val(self.fn_()).dup().with_parent(m.val.id);
        let alt = alt.val(self.fn_()).dup().with_parent(m.val.id);

        let f = self.fn_mut();
        f.add_block_edge(m.block, f.block(&then.id));
        f.add_block_edge(m.block, f.block(&alt.id));

        let inst = m.val.inst_mut(f);
        let br = inst.val;
        let old0 = std::mem::replace(&mut inst.rvals[0], cond);
        let old1 = std::mem::replace(&mut inst.rvals[1], then);
        let old2 = std::mem::replace(&mut inst.rvals[2], alt);
        debug_assert_eq!(old0.kind(self.ctx()), ValueKind::Void);
        debug_assert_eq!(old1.kind(self.ctx()), ValueKind::Void);
        debug_assert_eq!(old2.kind(self.ctx()), ValueKind::Void);
        br
    }

    pub fn resolve_breaks(&mut self, label: &str, dst: Block) {
        while let Some(ph) = self.unresolved_breaks.pop() {
            if ph.label == label {
                self.resolve_jump(ph.marker, dst);
                continue;
            } else {
                self.unresolved_breaks.push(ph);
                return;
            }
        }
    }
}
