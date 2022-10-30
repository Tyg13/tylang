pub(crate) mod block;
pub use block::*;

pub(crate) mod values;
pub use values::*;

pub(crate) mod function;
pub use function::*;

pub(crate) mod module;
pub use module::*;

pub(crate) mod ty;
pub use ty::*;

#[derive(Debug)]
pub struct Inst {
    pub val: ValueRef,
    pub kind: InstKind,
    pub lval: Option<ValueRef>,
    pub rvals: Vec<ValueRef>,
}

impl Inst {
    pub fn ident<'f>(&self, f: &'f Function) -> String {
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
    Subscript,
    Call,
    Add,
    Sub,
    Mul,
    Div,
    Jmp,
    Branch,
    Cmp { kind: CmpKind },
    Return,
    Nop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpKind {
    Eq,
    Gt,
    Lt,
    Gte,
    Lte,
}

impl InstKind {
    pub const fn can_have_lvals(&self) -> bool {
        match self {
            Self::Return | Self::Jmp => false,
            _ => true,
        }
    }

    pub const fn num_rvals(&self) -> std::ops::RangeInclusive<usize> {
        match self {
            InstKind::Var | InstKind::Nop => 0..=0,
            InstKind::Copy
            | InstKind::Cast
            | InstKind::Return
            | InstKind::Load
            | InstKind::Store
            | InstKind::Jmp => 1..=1,
            InstKind::Offset
            | InstKind::Add
            | InstKind::Sub
            | InstKind::Mul
            | InstKind::Div
            | InstKind::Cmp { .. } => 2..=2,
            InstKind::Branch => 3..=3,
            InstKind::Call | InstKind::Subscript => 1..=usize::MAX,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Context<'ctx> {
    pub(crate) m: Option<&'ctx Module>,
    pub(crate) f: Option<&'ctx Function>,
}

impl<'ctx> Context<'ctx> {
    #[inline]
    pub fn full(m: &'ctx Module, f: &'ctx Function) -> Self {
        Self {
            f: Some(f),
            m: Some(m),
        }
    }

    #[inline]
    pub fn fn_(f: &'ctx Function) -> Self {
        Self {
            f: Some(f),
            m: None,
        }
    }

    #[inline]
    pub fn mod_(m: &'ctx Module) -> Self {
        Self {
            m: Some(m),
            f: None,
        }
    }

    #[inline]
    pub fn as_fn(&self) -> &'ctx Function {
        self.f.expect("context has no function!")
    }

    #[inline]
    pub fn as_mod(&self) -> &'ctx Module {
        self.m.expect("context has no module!")
    }

    #[inline]
    pub fn types(&self) -> &'ctx TyContext {
        &self.as_mod().types
    }
}

impl<'ctx> From<&'ctx mut Function> for Context<'ctx> {
    fn from(f: &'ctx mut Function) -> Self {
        Self::fn_(f)
    }
}

impl<'ctx> From<&'ctx mut Module> for Context<'ctx> {
    fn from(m: &'ctx mut Module) -> Self {
        Self::mod_(m)
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

impl<'ctx, 'a: 'ctx> From<&'a ContextMut<'ctx>> for Context<'ctx> {
    fn from(ctx: &'a ContextMut<'ctx>) -> Self {
        Context {
            m: ctx.m.as_deref(),
            f: ctx.f.and_then(|id| ctx.m.as_ref().map(|m| m.fn_(&id))),
        }
    }
}

impl<'ctx, 'a: 'ctx> From<&'a mut ContextMut<'ctx>> for Context<'ctx> {
    fn from(ctx: &'a mut ContextMut<'ctx>) -> Self {
        Context {
            m: ctx.m.as_deref(),
            f: ctx.f.and_then(|id| ctx.m.as_ref().map(|m| m.fn_(&id))),
        }
    }
}

#[derive(Debug)]
pub struct ContextMut<'ctx> {
    pub(crate) m: Option<&'ctx mut Module>,
    pub(crate) f: Option<ValueID>,
}

impl<'ctx> ContextMut<'ctx> {
    #[inline]
    pub fn full(m: &'ctx mut Module, f: ValueID) -> Self {
        Self {
            m: Some(m),
            f: Some(f),
        }
    }

    #[inline]
    pub fn mod_(m: &'ctx mut Module) -> Self {
        Self {
            m: Some(m),
            f: None,
        }
    }

    #[inline]
    pub fn as_fn<'this: 'ctx>(&'this mut self) -> &'ctx mut Function {
        let fn_id = self.f.expect("context has no function!");
        self.as_mod().fn_mut(&fn_id)
    }

    #[inline]
    pub fn as_mod<'this: 'ctx>(&'this mut self) -> &'ctx mut Module {
        self.m.as_mut().expect("context has no module!")
    }

    #[inline]
    pub fn types<'this: 'ctx>(&'this mut self) -> &'ctx mut TyContext {
        &mut self.as_mod().types
    }
}

impl<'ctx> From<&'ctx mut Module> for ContextMut<'ctx> {
    fn from(m: &'ctx mut Module) -> Self {
        Self::mod_(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_value() {
        let local = ValueID::local(123);
        assert_eq!(local.is_global(), false);
        assert_eq!(local.is_local(), true);
        assert_eq!(local.as_idx(), 123);

        let local = ValueID::local(0);
        assert_eq!(local.is_global(), false);
        assert_eq!(local.is_local(), true);
        assert_eq!(local.as_idx(), 0);
    }

    #[test]
    fn global_value() {
        let global = ValueID::global(0);
        assert_eq!(global.is_global(), true);
        assert_eq!(global.is_local(), false);
        assert_eq!(global.as_idx(), 0);

        let global = ValueID::global(234);
        assert_eq!(global.is_global(), true);
        assert_eq!(global.is_local(), false);
        assert_eq!(global.as_idx(), 234);
    }

    #[test]
    fn repr() {
        assert_eq!("G0", format!("{}", ValueID::global(0)));
        assert_eq!("123", format!("{}", ValueID::local(123)));
        assert_eq!("ValueID::Global(0)", format!("{:?}", ValueID::global(0)));
        assert_eq!("ValueID::Local(123)", format!("{:?}", ValueID::local(123)));
    }
}
