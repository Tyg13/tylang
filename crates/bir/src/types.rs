use crate::id::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct Node;

#[derive(Default, Clone)]
pub struct Map {
    pub(crate) nodes: Vec<Kind>,
    pub(crate) ast: HashMap<ID, Arc<dyn ast::Node>>,
    pub(crate) typedefs: HashMap<ID, TypeDef>,
    pub(crate) modules: HashMap<ID, Module>,
    pub(crate) typerefs: HashMap<ID, TypeRef>,
    pub(crate) functions: HashMap<ID, Function>,
    pub(crate) literals: HashMap<ID, Literal>,
    pub(crate) scopes: HashMap<ID, Scope>,
    pub(crate) lets: HashMap<ID, Let>,
    pub(crate) exprs: HashMap<ID, Expr>,
    pub(crate) items: HashMap<ID, Item>,
    pub(crate) params: HashMap<ID, Parameter>,

    pub(crate) root_module: ID,
}

impl Map {
    pub fn node(&self, id: &ID) -> Kind {
        self.nodes.get(id.0).cloned().unwrap()
    }

    pub fn ast(&self, id: &ID) -> Option<Arc<dyn ast::Node>> {
        self.ast.get(id).cloned()
    }

    pub fn root_module(&self) -> &Module {
        self.mod_(&self.root_module)
    }
}

macro_rules! impl_map_lookup_fns {
    ($Map:ty {
        $($map_name:ident: $map_type:ident = $fn_name:ident & $fn_mut_name:ident)*$(,)?
    }) => {
        impl $Map {
            $(
                pub fn $fn_name(&self, id: &ID) -> &$map_type {
                    debug_assert_eq!(self.nodes[id.0], Kind::$map_type);
                    self.$map_name.get(id).unwrap()
                }
                pub fn $fn_mut_name(&mut self, id: &ID) -> &mut $map_type {
                    debug_assert_eq!(self.nodes[id.0], Kind::$map_type);
                    self.$map_name.get_mut(id).unwrap()
                }
            )*
        }
        impl std::fmt::Debug for $Map {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_list().entries(self.nodes.iter().enumerate());
                let mut f = f.debug_struct("Map");
                $(f.field(stringify!($map_name), &self.$map_name);)*
                f.finish()
            }
        }
    }
}

#[rustfmt::skip]
impl_map_lookup_fns!(
    Map {
        modules   : Module    = mod_     & mod_mut
        functions : Function  = fn_      & fn_mut
        typerefs  : TypeRef   = typeref  & typeref_mut
        typedefs  : TypeDef   = typedef  & typedef_mut
        literals  : Literal   = lit      & lit_mut
        lets      : Let       = let_     & let_mut
        scopes    : Scope     = scope    & scope_mut
        exprs     : Expr      = expr     & expr_mut
        items     : Item      = item     & item_mut
        params    : Parameter = param    & param_mut
    }
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Module,
    Function,
    Parameter,
    TypeRef,
    TypeDef,
    Scope,
    Item,
    Let,
    Expr,
    Literal,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub id: ID,
    pub functions: Vec<ID>,
    pub typedefs: Vec<ID>,
}

impl Module {
    pub fn new(id: ID) -> Self {
        Self {
            id,
            functions: Vec::default(),
            typedefs: Vec::default(),
        }
    }

    pub fn typedefs<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map TypeDef> + 'this {
        self.typedefs.iter().map(|id| map.typedef(id))
    }

    pub fn functions<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Function> + 'this {
        self.functions.iter().map(|id| map.fn_(id))
    }
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub id: ID,
    pub kind: TypeRefKind,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub id: ID,
    pub identifier: String,
    pub members: Vec<TypeMember>,
}

#[derive(Debug, Clone)]
pub struct TypeMember {
    pub identifier: String,
    pub ty: ID,
}

impl TypeMember {
    pub fn ty<'map>(&self, map: &'map Map) -> &'map TypeRef {
        map.typeref(&self.ty)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeRefKind {
    Void,
    Named { name: String },
    Pointer { pointee: ID },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub id: ID,
    pub module: ID,
    pub identifier: String,
    pub parameters: Vec<ID>,
    pub body: Option<ID>,
    pub return_type: ID,
}

impl Function {
    pub fn new(id: ID, module: ID, identifier: String, return_type: ID) -> Self {
        Self {
            id,
            module,
            identifier,
            parameters: Vec::default(),
            body: None,
            return_type,
        }
    }

    pub fn mod_<'map>(&self, map: &'map Map) -> &'map Module {
        map.mod_(&self.module)
    }

    pub fn parameters<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Parameter> + 'this {
        self.parameters.iter().map(|id| map.param(id))
    }

    pub fn return_type<'map>(&self, map: &'map Map) -> &'map TypeRef {
        map.typeref(&self.return_type)
    }

    pub fn body<'map>(&self, map: &'map Map) -> Option<&'map Scope> {
        self.body.map(|id| map.scope(&id))
    }
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub id: ID,
    pub function: ID,
    pub ty: ID,
    pub identifier: String,
    pub is_var_args: bool,
}

impl Parameter {
    pub fn named(id: ID, function: ID, ty: ID, name: String) -> Self {
        Self {
            id,
            function,
            ty,
            identifier: name,
            is_var_args: false,
        }
    }

    pub fn var_args(id: ID, function: ID) -> Self {
        Self {
            id,
            function,
            ty: NONE,
            identifier: "...".to_string(),
            is_var_args: true,
        }
    }

    pub fn function<'map>(&self, map: &'map Map) -> &'map Function {
        map.fn_(&self.function)
    }

    pub fn ty<'map>(&self, map: &'map Map) -> &'map TypeRef {
        map.typeref(&self.ty)
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub id: ID,
    pub label: String,
    pub kind: ScopeKind,

    pub parent: Option<ID>,
    pub function: ID,

    pub lets: Vec<ID>,
    pub items: Vec<ID>,
    pub return_expr: Option<ID>,
}

impl Scope {
    pub fn function<'map>(&self, map: &'map Map) -> &'map Function {
        map.fn_(&self.function)
    }

    pub fn items<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Item> + 'this {
        self.items.iter().map(|id| map.item(id))
    }

    pub fn lets<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Let> + 'this {
        self.lets.iter().map(|id| map.let_(id))
    }

    pub fn return_expr<'map>(&self, map: &'map Map) -> Option<&'map Expr> {
        self.return_expr.map(|id| map.expr(&id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Function,
    Block,
    Loop,
}

impl Scope {
    pub fn new(id: ID, kind: ScopeKind, parent: Option<ID>, function: ID, label: String) -> Self {
        Self {
            id,
            label,
            parent,
            kind,
            function,
            lets: Vec::default(),
            items: Vec::default(),
            return_expr: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: ID,
    pub kind: ItemKind,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Let(ID),
    Expr(ID),
}

#[derive(Debug, Clone)]
pub struct Let {
    pub id: ID,
    pub name: String,
    pub ty: Option<ID>,
    pub expr: Option<ID>,
}

impl Let {
    pub fn ty<'map>(&self, map: &'map Map) -> Option<&'map TypeRef> {
        self.ty.map(|id| map.typeref(&id))
    }

    pub fn expr<'map>(&self, map: &'map Map) -> Option<&'map Expr> {
        self.expr.map(|id| map.expr(&id))
    }
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub id: ID,
    pub kind: ExprKind,
}

impl Expr {
    pub fn name(&self) -> Option<&str> {
        if let ExprKind::NameRef { name } = &self.kind {
            Some(name.as_str())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(ID),
    NameRef {
        name: String,
    },
    Call {
        receiver: ID,
        operands: Vec<ID>,
    },
    Index {
        receiver: ID,
        index: ID,
    },
    Op(Op),
    Block {
        scope: ID,
    },
    Return {
        expr: ID,
    },
    Break {
        label: String,
    },
    Continue {
        label: String,
    },
    Branch {
        condition: ID,
        kind: BranchKind,
        left: ID,
        right: ID,
    },
    Loop {
        kind: LoopKind,
        body: ID,
    },
}

#[derive(Debug, Clone)]
pub enum BranchKind {
    If,
    IfElse,
}

#[derive(Debug, Clone)]
pub enum LoopKind {
    Loop,
    While,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(usize),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Op {
    pub fixity: OpFixity,
    pub kind: OpKind,
    pub operands: Vec<ID>,
}

impl Op {
    pub fn lhs(&self) -> ID {
        assert!(matches!(self.fixity, OpFixity::Infix));
        self.operands.get(0).copied().unwrap()
    }
    pub fn rhs(&self) -> ID {
        assert!(matches!(self.fixity, OpFixity::Infix));
        self.operands.get(1).copied().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpFixity {
    Prefix,
    Postfix,
    Infix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Plus,
    Minus,
    Multiply,
    Divide,
    FieldAccess,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    Equals,
    Assignment,
}
