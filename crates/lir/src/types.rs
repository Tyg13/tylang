use std::{collections::HashMap, fmt::format, ops::Index};
use utils::vec_graph::{NodeRef, VecGraph};

pub type Block = NodeRef<ValueID>;
pub type BlockGraph = VecGraph<ValueID>;

#[derive(Debug, Default)]
pub struct Values {
    pub(crate) vals: Vec<Value>,
    sema_to_val: HashMap<sema::ID, ValueRef>,
    val_to_sema: HashMap<ValueID, sema::ID>,
    users: HashMap<ValueID, Vec<ValueID>>,
}

impl std::ops::Index<ValueID> for Values {
    type Output = Value;
    fn index(&self, index: ValueID) -> &Self::Output {
        self.vals.index(index.0)
    }
}

impl Values {
    pub fn val(&self, sema: &sema::ID) -> Option<&ValueRef> {
        self.sema_to_val.get(sema)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        self.vals.iter()
    }

    pub fn sema(&self, val: impl Into<ValueID>) -> Option<sema::ID> {
        self.val_to_sema.get(&val.into()).copied()
    }

    pub(crate) fn add_val(
        &mut self,
        kind: ValueKind,
        sema: Option<sema::ID>,
        ident: Option<String>,
        global: bool,
    ) -> ValueRef {
        let id = ValueID(self.vals.len());
        let ident = ident.unwrap_or(format!(".v{}", id.0));
        self.vals.push(Value {
            id,
            sema,
            kind,
            ident,
        });
        let val = match global {
            true => ValueRef::global(id),
            false => ValueRef::local(id),
        };
        let ret = val;
        if let Some(sema) = sema {
            self.val_to_sema.insert(val.id, sema);
            self.sema_to_val.insert(sema, val);
        }
        ret
    }

    fn add_user(&mut self, val: ValueID, user: ValueID) {
        self.users.entry(val).or_default().push(user);
    }

    pub fn users(&self, val: &ValueID) -> impl Iterator<Item = ValueID> + '_ {
        let mut it = self.users.get(val).unwrap().iter();
        std::iter::from_fn(move || it.next().copied())
    }
}

#[derive(Debug)]
pub struct Module {
    pub functions: Vec<Function>,
    pub globals: Values,
}

impl Module {
    pub fn new() -> Self {
        Self {
            functions: Default::default(),
            globals: Default::default(),
        }
    }
    pub fn add_global(
        &mut self,
        kind: ValueKind,
        sema: Option<sema::ID>,
        ident: Option<String>,
    ) -> ValueRef {
        self.globals.add_val(kind, sema, ident, true)
    }

    pub fn val(&self, sema: &sema::ID) -> Option<&ValueRef> {
        self.globals.val(sema)
    }
}

#[derive(Debug)]
pub struct Function {
    pub sema: sema::ID,
    pub ident: String,
    pub params: Vec<ValueRef>,
    pub insts: Vec<Inst>,

    pub(crate) vals_to_insts: HashMap<ValueID, usize>,
    pub(crate) locals: Values,

    blocks: BlockGraph,
}

impl Function {
    pub fn new(id: sema::ID, ident: String, params: Vec<sema::ID>, sema: &sema::Map) -> Self {
        let mut this = Self {
            sema: id,
            ident,
            params: Vec::new(),
            insts: Default::default(),
            vals_to_insts: Default::default(),
            locals: Default::default(),
            blocks: BlockGraph::new(),
        };
        this.params = params
            .into_iter()
            .map(|param| {
                let ident = sema.param(param).unwrap().ident(sema).to_string();
                this.add_val(ValueKind::Param, Some(param), Some(ident))
            })
            .collect();
        this
    }

    pub fn nth_param(&self, n: usize) -> Option<&ValueRef> {
        self.params.get(n)
    }

    pub fn param_num(&self, val: impl Into<ValueID>) -> Option<usize> {
        let idx = val.into().0;
        debug_assert!(self.nth_param(idx).is_some());
        Some(idx)
    }

    pub fn blocks(&self) -> &BlockGraph {
        &self.blocks
    }

    pub fn ty<'map>(&self, sema: &'map sema::Map) -> &'map sema::Type {
        sema.ty(self.sema).unwrap()
    }

    pub fn return_ty<'map>(&self, sema: &'map sema::Map) -> &'map sema::Type {
        self.ty(sema).as_fn_ty().return_ty(sema)
    }

    pub fn val(&self, sema: &sema::ID) -> Option<&ValueRef> {
        self.locals.val(sema)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        self.locals.values()
    }

    fn inst_at(&self, idx: usize) -> Option<&Inst> {
        self.insts.get(idx)
    }

    fn inst(&self, val: &ValueRef) -> Option<&Inst> {
        self.vals_to_insts
            .get(&val.id)
            .and_then(|&idx| self.inst_at(idx))
    }

    pub fn sema(&self, val: impl Into<ValueID>) -> Option<sema::ID> {
        self.locals.sema(val)
    }

    pub(crate) fn add_block(&mut self, val: ValueID) -> Block {
        self.blocks.add_vertex(val)
    }

    pub(crate) fn add_val(
        &mut self,
        kind: ValueKind,
        sema: Option<sema::ID>,
        ident: Option<String>,
    ) -> ValueRef {
        self.locals.add_val(kind, sema, ident, false)
    }

    fn add_user(&mut self, val: ValueID, user: ValueID) {
        self.locals.add_user(val, user)
    }

    pub fn users(&self, val: &ValueID) -> impl Iterator<Item = ValueID> + '_ {
        self.locals.users(val)
    }

    pub(crate) fn add_inst(
        &mut self,
        sema: Option<sema::ID>,
        kind: InstKind,
        lval: Option<ValueRef>,
        rvals: Vec<ValueRef>,
        ident: Option<String>,
    ) -> ValueRef {
        let inst_val = self.add_val(ValueKind::Inst, sema, ident.clone());
        let lval = if kind.can_have_lvals() {
            lval.map(|v| v.with_parent(&inst_val))
        } else {
            debug_assert_eq!(lval, None);
            None
        };
        let rvals = rvals
            .into_iter()
            .map(|val| {
                self.add_user(val.id, inst_val.id);
                val.with_parent(&inst_val)
            })
            .collect();
        if self.insts.is_empty() {
            self.add_block(inst_val.id);
        }
        let idx = self.insts.len();
        self.insts.push(Inst {
            val: inst_val,
            kind,
            lval,
            rvals,
        });
        self.vals_to_insts.insert(inst_val.id, idx);
        inst_val
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueID(pub(crate) usize);

impl ToString for ValueID {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug)]
pub struct Inst {
    pub val: ValueRef,
    pub kind: InstKind,
    pub lval: Option<ValueRef>,
    pub rvals: Vec<ValueRef>,
}

impl Inst {
    pub fn ident<'f>(&self, f: &'f Function) -> &'f str {
        self.val.ident(f)
    }

    pub fn lval(&self) -> ValueRef {
        self.lval.unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstKind {
    Var,
    Copy,
    Cast,
    Offset,
    Load,
    Store,
    Call,
    Add,
    Jmp,
    Cmp { kind: CmpKind },
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpKind {
    Eq,
}

impl InstKind {
    pub const fn can_have_lvals(&self) -> bool {
        match self {
            Self::Return | Self::Var => false,
            _ => true,
        }
    }
    pub const fn num_rvals(&self) -> std::ops::RangeInclusive<usize> {
        match self {
            InstKind::Var => 0..=0,
            InstKind::Copy
            | InstKind::Cast
            | InstKind::Return
            | InstKind::Load
            | InstKind::Store
            | InstKind::Jmp => 1..=1,
            InstKind::Offset | InstKind::Add | InstKind::Cmp { .. } => 2..=2,
            InstKind::Call => 1..=usize::MAX,
        }
    }
}

#[derive(Debug)]
pub struct Value {
    pub id: ValueID,
    pub sema: Option<sema::ID>,
    pub kind: ValueKind,
    pub ident: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Function,
    Param,
    Inst,
    Constant,
    Void,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ValueRef {
    pub parent: Option<ValueID>,
    pub id: ValueID,
    pub global: bool,
}

impl From<ValueRef> for ValueID {
    fn from(v: ValueRef) -> Self {
        v.id
    }
}

impl From<&ValueRef> for ValueID {
    fn from(v: &ValueRef) -> Self {
        v.id
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Context<'ctx> {
    pub(crate) m: Option<&'ctx Module>,
    pub(crate) f: Option<&'ctx Function>,
}

impl<'ctx> Context<'ctx> {
    pub fn full(m: &'ctx Module, f: &'ctx Function) -> Self {
        Self {
            f: Some(f),
            m: Some(m),
        }
    }

    pub fn fn_(f: &'ctx Function) -> Self {
        Self {
            f: Some(f),
            m: None,
        }
    }

    pub fn mod_(m: &'ctx Module) -> Self {
        Self {
            m: Some(m),
            f: None,
        }
    }

    pub fn as_fn(&self) -> &'ctx Function {
        self.f.expect("context has no function!")
    }

    pub fn as_mod(&self) -> &'ctx Module {
        self.m.expect("context has no module!")
    }
}

impl<'ctx> From<&'ctx Function> for Context<'ctx> {
    fn from(f: &'ctx Function) -> Self {
        Self::fn_(f)
    }
}

impl<'ctx> From<&'ctx Module> for Context<'ctx> {
    fn from(m: &'ctx Module) -> Self {
        Self::mod_(m)
    }
}

impl ValueRef {
    pub(crate) fn local(id: ValueID) -> Self {
        Self {
            parent: None,
            id,
            global: false,
        }
    }

    pub(crate) fn global(id: ValueID) -> Self {
        Self {
            parent: None,
            id,
            global: true,
        }
    }

    fn with_parent(self, parent: &ValueRef) -> Self {
        Self {
            parent: Some(parent.id),
            ..self
        }
    }

    pub fn dup(&self) -> Self {
        Self {
            parent: None,
            ..*self
        }
    }

    pub fn is_var<'ctx>(&self, ctx: impl Into<Context<'ctx>>) -> bool {
        self.into_inst(ctx.into().as_fn())
            .filter(|inst| inst.kind == InstKind::Var)
            .is_some()
    }

    pub fn into_inst<'ctx>(&self, f: &'ctx Function) -> Option<&'ctx Inst> {
        if self.global {
            return None;
        }
        if self.inner(f).kind != ValueKind::Inst {
            return None;
        }
        f.inst(self)
    }

    fn inner_vals<'f>(&self, ctx: impl Into<Context<'f>>) -> &'f Values {
        let ctx = ctx.into();
        match self.global {
            true => &ctx.as_mod().globals,
            false => &ctx.as_fn().locals,
        }
    }

    fn inner<'f>(&self, ctx: impl Into<Context<'f>>) -> &'f Value {
        self.inner_vals(ctx).index(self.id)
    }

    pub fn users<'f>(&self, ctx: impl Into<Context<'f>>) -> impl Iterator<Item = ValueID> + 'f {
        self.inner_vals(ctx).users(&self.id)
    }

    pub fn ty<'f, 'map>(
        &self,
        ctx: impl Into<Context<'f>>,
        sema: &'map sema::Map,
    ) -> &'map sema::Type {
        self.inner_vals(ctx)
            .sema(self)
            .and_then(|id| sema.ty(id))
            .unwrap()
    }

    pub fn ident<'f>(&self, ctx: impl Into<Context<'f>>) -> &'f str {
        self.inner(ctx).ident.as_ref()
    }

    pub fn kind<'f>(&self, ctx: impl Into<Context<'f>>) -> ValueKind {
        self.inner(ctx).kind
    }

    pub fn sema<'f>(&self, ctx: impl Into<Context<'f>>) -> sema::ID {
        self.inner_vals(ctx).sema(self.id).unwrap()
    }

    pub fn repr<'f>(&self, ctx: impl Into<Context<'f>>, sema: &sema::Map) -> String {
        let ctx = ctx.into();
        match self.kind(ctx) {
            ValueKind::Param | ValueKind::Inst => format!("{}", self.ident(ctx).clone()),
            ValueKind::Constant => match sema.constant(self.sema(ctx)).unwrap() {
                sema::Constant::Int(n) => n.to_string(),
                sema::Constant::Str(s) => format!("{s:?}"),
            },
            ValueKind::Function => sema.name(self.sema(ctx)).unwrap().ident.clone(),
            ValueKind::Void => "void".to_string(),
        }
    }
}
