use smallvec::SmallVec;
use std::collections::HashMap;

use utils::folding_set::{FoldID, FoldKey, Foldable, FoldingSet};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyID(pub(crate) FoldID<Ty>);

impl Foldable for TyID {
    fn fold(&self, key: &mut utils::folding_set::FoldKey) {
        self.0.fold(key);
    }
}

impl TyID {
    pub fn get<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> &'ctx Ty {
        ctx.into().get(self)
    }
}

#[derive(Debug)]
pub struct FnTy<'ty> {
    pub id: TyID,
    pub return_ty: TyID,
    pub params: &'ty [TyID],
    pub is_var_args: bool,
}

impl<'ty> FnTy<'ty> {
    pub fn return_ty(&self, ctx: impl Into<&'ty TyContext>) -> &'ty Ty {
        self.return_ty.get(ctx.into())
    }

    pub fn params(
        &self,
        ctx: impl Into<&'ty TyContext>,
    ) -> impl Iterator<Item = &'ty Ty> + '_ {
        self.params.iter().map({
            let ctx = ctx.into();
            move |ty| ty.get(ctx)
        })
    }
}

#[derive(Debug)]
pub struct StructTy<'ty, 'name> {
    pub id: TyID,
    pub members: &'ty [TyID],
    pub name: &'name str,
}

#[derive(Debug)]
pub struct PtrTy {
    pub id: TyID,
    pointee: TyID,
}

impl PtrTy {
    pub fn pointee<'ty>(&self, ctx: impl Into<&'ty TyContext>) -> &'ty Ty {
        self.pointee.get(ctx)
    }
}

#[derive(Debug)]
pub struct Ty {
    pub id: TyID,
    pub kind: TyKind,
    name: Option<String>,
    inner_tys: SmallVec<[TyID; 2]>,
}

impl Foldable for Ty {
    fn fold(&self, key: &mut utils::folding_set::FoldKey) {
        key.add(&self.kind);
        key.add_all(&self.inner_tys);
        key.add_all(&self.name);
    }
    fn fold_key(&self) -> FoldKey {
        Ty::profile(&self.kind, &self.inner_tys, &self.name)
    }
}
impl Ty {
    fn profile(
        kind: &TyKind,
        inner_tys: &[TyID],
        name: &Option<String>,
    ) -> utils::folding_set::FoldKey {
        let mut key = FoldKey::default();
        key.add(kind);
        key.add(&inner_tys);
        if let Some(name) = name {
            key.add(name);
        }
        key
    }
}

impl Ty {
    pub fn as_fn_ty(&self) -> FnTy<'_> {
        let is_var_args = if let TyKind::Fn { is_var_args } = self.kind {
            is_var_args
        } else {
            panic!("not a function type!")
        };
        FnTy {
            id: self.id,
            return_ty: self.inner_tys[0],
            params: &self.inner_tys[1..],
            is_var_args,
        }
    }

    pub fn as_struct_ty<'ctx>(
        &self,
        ctx: impl Into<&'ctx TyContext>,
    ) -> StructTy<'_, 'ctx> {
        if let TyKind::Struct = self.kind {
            let ctx = ctx.into();
            StructTy {
                id: self.id,
                members: &self.inner_tys,
                name: ctx.get(&self.id).name.as_ref().unwrap(),
            }
        } else {
            panic!("not a function type!")
        }
    }

    pub fn as_ptr_ty(&self) -> PtrTy {
        PtrTy {
            id: self.id,
            pointee: self.inner_tys[0],
        }
    }

    pub fn pointer_to<'ctx>(
        &self,
        ctx: impl Into<&'ctx mut TyContext>,
    ) -> &'ctx Ty {
        let ctx: &mut TyContext = ctx.into();
        let pointer = ctx.get_pointer_to(&self.id);
        ctx.get(&pointer)
    }

    pub fn is_void(&self) -> bool {
        self.kind == TyKind::Void
    }

    pub fn is_ptr(&self) -> bool {
        self.kind == TyKind::Void
    }

    pub fn has_lval(&self) -> bool {
        self.kind != TyKind::Void
    }

    pub fn repr<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> String {
        let ctx = ctx.into();
        match self.kind {
            TyKind::Integer { size } => format!("i{size}"),
            TyKind::Pointer => {
                format!("*{}", self.as_ptr_ty().pointee(ctx).repr(ctx))
            }
            TyKind::Void => "void".to_string(),
            TyKind::Fn { is_var_args } => {
                let fn_ty = self.as_fn_ty();
                let mut params = fn_ty
                    .params(ctx)
                    .map(|p| p.repr(ctx))
                    .collect::<Vec<_>>()
                    .join(",");
                if is_var_args {
                    params.push_str(&", ...");
                }
                let ret = fn_ty.return_ty(ctx).repr(ctx);
                format!("fn ({}) -> {}", params, ret)
            }
            TyKind::Struct => self.as_struct_ty(ctx).name.to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TyKind {
    Integer { size: usize },
    Pointer,
    Void,
    Fn { is_var_args: bool },
    Struct,
}
impl Foldable for TyKind {
    fn fold(&self, key: &mut utils::folding_set::FoldKey) {
        match self {
            TyKind::Integer { size } => {
                key.add(&0);
                key.add(size);
            }
            TyKind::Pointer => {
                key.add(&1);
                key.add(&0);
            }
            TyKind::Void => {
                key.add(&2);
                key.add(&0);
            }
            TyKind::Fn { is_var_args } => {
                key.add(&3);
                key.add(is_var_args);
            }
            TyKind::Struct => {
                key.add(&4);
                key.add(&0);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct TyContext {
    types: FoldingSet<Ty>,

    void_ty: Option<TyID>,
    str_ty: Option<TyID>,
    int_tys: HashMap<usize, TyID>,
    structs_by_name: HashMap<String, TyID>,
}

impl<'a> From<&'a crate::Module> for &'a TyContext {
    fn from(m: &'a crate::Module) -> Self {
        &m.types
    }
}

impl<'a> From<crate::Context<'a>> for &'a TyContext {
    fn from(c: crate::Context<'a>) -> Self {
        &c.as_mod().types
    }
}

impl<'ctx, 'a: 'ctx> From<&'a mut crate::ContextMut<'ctx>> for &'ctx TyContext {
    fn from(c: &'a mut crate::ContextMut<'ctx>) -> Self {
        &c.as_mod().types
    }
}

impl<'ctx, 'a: 'ctx> From<&'a mut crate::ContextMut<'ctx>>
    for &'ctx mut TyContext
{
    fn from(c: &'a mut crate::ContextMut<'ctx>) -> Self {
        &mut c.as_mod().types
    }
}

impl TyContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: &TyID) -> &Ty {
        self.types.get(&id.0).expect("inconsistent type map!")
    }

    pub fn iter(&self) -> impl Iterator<Item = &Ty> + '_ {
        self.types.into_iter()
    }

    fn new_ty_with_inner(
        &mut self,
        kind: TyKind,
        inner_tys: &[TyID],
        name: Option<String>,
    ) -> TyID {
        assert!(kind != TyKind::Struct || name.is_some());
        let key = Ty::profile(&kind, inner_tys, &name);
        if let Some(ty) = self.types.try_get_from_key(&key) {
            return ty.id;
        }
        self.types.insert_and_then(
            key,
            Ty {
                id: TyID::default(),
                kind,
                inner_tys: SmallVec::from_slice(inner_tys),
                name,
            },
            |ty, id| {
                ty.id = TyID(id);
                ty.id
            },
        )
    }

    fn new_ty(&mut self, kind: TyKind, name: Option<String>) -> TyID {
        self.new_ty_with_inner(kind, &[], name)
    }

    pub fn void(&self) -> &Ty {
        self.void_ty.unwrap().get(self)
    }

    pub fn get_void(&mut self) -> TyID {
        if let Some(id) = self.void_ty {
            return id;
        }
        let id = self.new_ty(TyKind::Void, None);
        self.void_ty = Some(id);
        id
    }

    pub fn get_str(&mut self) -> TyID {
        if let Some(id) = self.str_ty {
            return id;
        }
        let int8 = self.get_int(8);
        let id = self.get_pointer_to(&int8);
        self.str_ty = Some(id);
        id
    }

    pub fn get_int(&mut self, size: usize) -> TyID {
        if let Some(id) = self.int_tys.get(&size) {
            return *id;
        }
        let id = self.new_ty(TyKind::Integer { size }, None);
        self.int_tys.insert(size, id);
        id
    }

    pub fn get_pointer_to(&mut self, pointee: &TyID) -> TyID {
        self.new_ty_with_inner(TyKind::Pointer, &[*pointee], None)
    }

    pub fn get_fn(
        &mut self,
        is_var_args: bool,
        return_ty: &TyID,
        params: &[TyID],
    ) -> TyID {
        let mut inner_tys = vec![*return_ty];
        inner_tys.extend_from_slice(params);
        self.new_ty_with_inner(TyKind::Fn { is_var_args }, &inner_tys, None)
    }

    pub fn get_struct(&mut self, name: &str, members: &[TyID]) -> TyID {
        if let Some(id) = self.structs_by_name.get(name) {
            return *id;
        }
        let id = self.new_ty_with_inner(
            TyKind::Struct,
            members,
            Some(name.to_string()),
        );
        self.structs_by_name.insert(name.to_string(), id);
        id
    }
}
