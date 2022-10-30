use std::{collections::HashMap, slice::Iter};

use crate::types::{Block, Context, Function, Inst, InstKind, Ty, TyID};

pub struct Users<'v>(Option<Iter<'v, ValueID>>);

impl<'v> Iterator for Users<'v> {
    type Item = ValueID;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.as_mut().and_then(|vals| vals.next().copied())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueID(u32);

impl ValueID {
    #[inline]
    pub fn local(id: usize) -> Self {
        debug_assert!(id < (1 << 31));
        Self(id as u32)
    }

    #[inline]
    pub fn global(id: usize) -> Self {
        debug_assert!(id < (1 << 31));
        Self((id as u32) | (1 << 31))
    }

    #[inline]
    pub fn is_global(&self) -> bool {
        self.0 >= (1 << 31)
    }

    #[inline]
    pub fn is_local(&self) -> bool {
        !self.is_global()
    }

    #[inline]
    pub fn as_idx(&self) -> usize {
        (self.0 & !(1 << 31)) as usize
    }
}

impl std::fmt::Debug for ValueID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(if self.is_global() {
            "ValueID::Global"
        } else {
            "ValueID::Local"
        })
        .field(&self.as_idx())
        .finish()
    }
}

impl std::fmt::Display for ValueID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_global() {
            write!(f, "G")?;
        }
        write!(f, "{}", self.as_idx())
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct ValueRef {
    pub id: ValueID,
    pub parent: Option<ValueID>,
}

impl ValueRef {
    #[inline]
    pub(crate) fn new(id: ValueID) -> Self {
        Self { parent: None, id }
    }

    #[inline]
    pub fn parent(&self) -> ValueID {
        self.parent.unwrap()
    }

    #[inline]
    pub fn is_global(&self) -> bool {
        self.id.is_global()
    }

    #[inline]
    pub fn is_local(&self) -> bool {
        self.id.is_local()
    }

    #[inline]
    pub fn with_parent(self, parent: ValueID) -> Self {
        debug_assert_eq!(self.parent, None);
        Self {
            parent: Some(parent),
            ..self
        }
    }

    #[inline]
    pub(crate) fn inst_mut<'f>(
        &self,
        f: &'f mut Function,
    ) -> Option<&'f mut Inst> {
        f.inst_mut(self.id)
    }

    #[inline]
    pub fn inst<'f>(&self, ctx: impl Into<Context<'f>>) -> Option<&'f Inst> {
        debug_assert!(self.is_local());
        ctx.into().as_fn().inst(self.id)
    }

    #[inline]
    pub fn block<'f>(&self, f: &'f Function) -> Block {
        debug_assert!(self.is_local());
        debug_assert_eq!(self.kind(&*f), ValueKind::Block);
        *f.blocks_by_id.get(&self.id).unwrap()
    }

    pub fn is_var<'ctx>(&self, ctx: impl Into<Context<'ctx>>) -> bool {
        self.inst(ctx.into().as_fn())
            .filter(|inst| inst.kind == InstKind::Var)
            .is_some()
    }

    #[inline]
    fn inner_vals<'f>(&self, ctx: impl Into<Context<'f>>) -> &'f Values {
        let ctx = ctx.into();
        match self.is_global() {
            true => &ctx.as_mod().globals,
            false => &ctx.as_fn().locals,
        }
    }

    #[inline]
    fn inner<'f>(&self, ctx: impl Into<Context<'f>>) -> &'f Value {
        self.inner_vals(ctx).get(&self.id)
    }

    #[inline]
    pub fn users<'f>(&self, ctx: impl Into<Context<'f>>) -> Users<'f> {
        self.inner_vals(ctx).users(&self.id)
    }

    #[inline]
    pub fn ty<'map, 'f: 'map>(&self, ctx: impl Into<Context<'f>>) -> &'map Ty {
        let ctx: Context = ctx.into();
        let ty = self.inner_vals(ctx.clone()).ty(&self.id);
        ctx.as_mod().types.get(&ty)
    }

    #[inline]
    pub fn ident<'f>(&self, ctx: impl Into<Context<'f>>) -> String {
        self.inner_vals(ctx)
            .idents
            .get(&self.id)
            .cloned()
            .unwrap_or(format!(".v{}", self.id.0))
    }

    #[inline]
    pub fn kind<'f>(&self, ctx: impl Into<Context<'f>>) -> ValueKind {
        self.inner(ctx).kind
    }

    pub fn repr<'f>(&self, ctx: impl Into<Context<'f>>) -> String {
        let ctx: Context = ctx.into();
        match self.kind(ctx) {
            ValueKind::Param | ValueKind::Inst | ValueKind::Block => {
                self.ident(ctx).to_string()
            }
            ValueKind::Constant(kind) => match kind {
                ConstantKind::Str => {
                    format!("{:?}", self.str_constant(ctx).to_string())
                }
                ConstantKind::Int => self.int_constant(ctx).to_string(),
            },
            ValueKind::Function => ctx.as_mod().fn_(&self.id).ident.clone(),
            ValueKind::Void => "void".to_string(),
        }
    }

    #[inline]
    pub fn str_constant<'this, 'f: 'this>(
        &'this self,
        ctx: impl Into<Context<'f>>,
    ) -> &'this str {
        ctx.into().as_mod().str_constant(&self.id)
    }

    #[inline]
    pub fn int_constant<'f>(&self, ctx: impl Into<Context<'f>>) -> usize {
        ctx.into().as_mod().int_constant(&self.id)
    }
}

pub trait Duplicate {
    fn dup(self) -> Self;
}

impl Duplicate for ValueRef {
    #[inline]
    fn dup(self) -> Self {
        Self {
            parent: None,
            ..self
        }
    }
}

impl Duplicate for Option<ValueRef> {
    #[inline]
    fn dup(self) -> Self {
        self.map(ValueRef::dup)
    }
}

#[derive(Debug)]
pub struct Value {
    pub id: ValueID,
    pub kind: ValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Function,
    Param,
    Inst,
    Constant(ConstantKind),
    Block,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstantKind {
    Str,
    Int,
}

impl From<ValueRef> for ValueID {
    #[inline]
    fn from(v: ValueRef) -> Self {
        v.id
    }
}

impl From<&ValueRef> for ValueID {
    #[inline]
    fn from(v: &ValueRef) -> Self {
        v.id
    }
}

#[derive(Debug, Default)]
pub struct Values {
    pub(crate) vals: Vec<Value>,
    types: HashMap<ValueID, TyID>,
    users: HashMap<ValueID, Vec<ValueID>>,
    idents: HashMap<ValueID, String>,
}

impl Values {
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        self.vals.iter()
    }

    pub(crate) fn get(&self, val: &ValueID) -> &Value {
        self.vals.get(val.as_idx()).unwrap()
    }

    pub(crate) fn ty(&self, val: &ValueID) -> TyID {
        *self.types.get(val).unwrap()
    }

    pub(crate) fn add_val(
        &mut self,
        kind: ValueKind,
        ty: TyID,
        ident: Option<&str>,
        global: bool,
    ) -> ValueRef {
        let id = {
            let idx = self.vals.len();
            if global {
                ValueID::global(idx)
            } else {
                ValueID::local(idx)
            }
        };
        self.vals.push(Value { id, kind });
        self.types.insert(id, ty);
        if let Some(ident) = ident {
            self.idents.insert(id, ident.to_string());
        }
        ValueRef::new(id)
    }

    #[inline]
    pub(crate) fn add_user(&mut self, val: ValueID, user: ValueID) {
        self.users.entry(val).or_default().push(user);
    }

    #[inline]
    pub fn users(&self, val: &ValueID) -> Users<'_> {
        Users(self.users.get(val).map(|users| users.iter()))
    }
}
