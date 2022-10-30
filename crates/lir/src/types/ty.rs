use std::collections::HashMap;
use utils::folding_set::FoldingSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyID(usize);

impl TyID {
    pub fn get<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> &'ctx Ty {
        ctx.into().get(self)
    }
}

#[derive(Debug)]
pub struct FnTy<'ty> {
    pub id: TyID,
    return_ty: TyID,
    params: &'ty [TyID],
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
    inner_tys: Vec<TyID>,
}

impl Ty {
    pub fn as_fn_ty(&self) -> FnTy<'_> {
        FnTy {
            id: self.id,
            return_ty: self.inner_tys[0],
            params: &self.inner_tys[1..],
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

    pub fn repr<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> String {
        let ctx = ctx.into();
        match self.kind {
            TyKind::Integer { size } => format!("i{size}"),
            TyKind::Pointer => {
                format!("*{}", self.as_ptr_ty().pointee(ctx).repr(ctx))
            }
            TyKind::Void => "void".to_string(),
            TyKind::Fn => {
                let fn_ty = self.as_fn_ty();
                let params = fn_ty
                    .params(ctx)
                    .map(|p| p.repr(ctx))
                    .collect::<Vec<_>>()
                    .join(",");
                let ret = fn_ty.return_ty(ctx).repr(ctx);
                format!("fn ({}) -> {}", params, ret)
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TyKind {
    Integer { size: usize },
    Pointer,
    Void,
    Fn,
}

#[derive(Debug, Default)]
pub struct TyContext {
    types: Vec<Ty>,

    void_ty: Option<TyID>,
    str_ty: Option<TyID>,
    int_tys: HashMap<usize, TyID>,
    pointer_tys: HashMap<TyID, TyID>,
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
        self.types.get(id.0).expect("inconsistent type map!")
    }

    fn new_ty_with_inner(
        &mut self,
        kind: TyKind,
        inner_tys: Vec<TyID>,
    ) -> TyID {
        let id = TyID(self.types.len());
        self.types.push(Ty {
            id,
            kind,
            inner_tys,
        });
        id
    }

    fn new_ty(&mut self, kind: TyKind) -> TyID {
        self.new_ty_with_inner(kind, Vec::new())
    }

    pub fn void(&self) -> &Ty {
        self.void_ty.unwrap().get(self)
    }

    pub fn get_void(&mut self) -> TyID {
        if let Some(id) = self.void_ty {
            return id;
        }
        let id = self.new_ty(TyKind::Void);
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
        let id = self.new_ty(TyKind::Integer { size });
        self.int_tys.insert(size, id);
        id
    }

    pub fn get_pointer_to(&mut self, pointee: &TyID) -> TyID {
        if let Some(id) = self.pointer_tys.get(pointee) {
            return *id;
        }
        let id = self.new_ty_with_inner(TyKind::Pointer, vec![*pointee]);
        self.pointer_tys.insert(*pointee, id);
        id
    }

    pub fn get_fn(&mut self, return_ty: &TyID, params: &[TyID]) -> TyID {
        let mut inner_tys = vec![*return_ty];
        inner_tys.extend_from_slice(params);
        self.new_ty_with_inner(TyKind::Fn, inner_tys)
    }

    pub fn get_struct(&mut self, params: &[TyID]) -> TyID {
        self.new_ty_with_inner(TyKind::Fn, params.to_vec())
    }
}
