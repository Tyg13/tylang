use std::sync::Arc;
use utils::newtype_idx;

newtype_idx!(ID);

type IDMap<V> = fxhash::FxHashMap<ID, V>;

#[derive(Debug, Clone)]
pub(crate) struct Node;

#[derive(Default, Clone)]
pub struct Map {
    pub(crate) nodes: Vec<Kind>,
    pub(crate) ast: IDMap<Arc<dyn ast::Node>>,

    pub(crate) typedefs: IDMap<TypeDef>,
    pub(crate) modules: IDMap<Module>,
    pub(crate) imports: IDMap<Import>,
    pub(crate) names: IDMap<Name>,
    pub(crate) typerefs: IDMap<TypeRef>,
    pub(crate) functions: IDMap<Function>,
    pub(crate) literals: IDMap<Literal>,
    pub(crate) blocks: IDMap<Block>,
    pub(crate) lets: IDMap<Let>,
    pub(crate) exprs: IDMap<Expr>,
    pub(crate) items: IDMap<Item>,
    pub(crate) params: IDMap<Parameter>,

    pub(crate) root_module: Option<ID>,
}

impl Map {
    pub fn nodes(&self) -> impl IntoIterator<Item = (ID, Kind)> + '_ {
        self.nodes
            .iter()
            .enumerate()
            .map(|(idx, kind)| (ID(idx as u64), *kind))
    }

    pub fn kind(&self, id: &ID) -> Kind {
        self.nodes.get(id.as_idx()).cloned().unwrap()
    }

    pub fn ast(&self, id: &ID) -> Option<Arc<dyn ast::Node>> {
        self.ast.get(id).cloned()
    }

    pub fn root_module(&self) -> &Module {
        self.mod_(&self.root_module.unwrap())
    }
}

macro_rules! impl_map_lookup_fns {
    ($($map_name:ident: $map_type:ident = $fn_name:ident | $fn_mut_name:ident)*) => {
        impl Map {
            $(
                pub fn $map_name(&self) -> impl Iterator<Item = &$map_type> + '_ {
                    self.$map_name.values()
                }
                pub fn $fn_name(&self, id: &ID) -> &$map_type {
                    debug_assert_eq!(self.nodes[id.as_idx()], Kind::$map_type);
                    self.$map_name.get(id).unwrap()
                }
                pub fn $fn_mut_name(&mut self, id: &ID) -> &mut $map_type {
                    debug_assert_eq!(self.nodes[id.as_idx()], Kind::$map_type);
                    self.$map_name.get_mut(id).unwrap()
                }
            )*
        }
        impl std::fmt::Debug for Map {
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
    modules   : Module    = mod_     | mod_mut
    functions : Function  = fn_      | fn_mut
    typerefs  : TypeRef   = typeref  | typeref_mut
    typedefs  : TypeDef   = typedef  | typedef_mut
    imports   : Import    = import   | import_mut
    names     : Name      = name     | name_mut
    literals  : Literal   = lit      | lit_mut
    lets      : Let       = let_     | let_mut
    blocks    : Block     = block    | block_mut
    exprs     : Expr      = expr     | expr_mut
    items     : Item      = item     | item_mut
    params    : Parameter = param    | param_mut
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Module,
    Import,
    Function,
    Parameter,
    Name,
    TypeRef,
    TypeDef,
    Block,
    Item,
    Let,
    Expr,
    Literal,
    Tombstone,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub id: ID,
    pub ident: Option<String>,
    pub functions: Vec<ID>,
    pub typedefs: Vec<ID>,
    pub modules: Vec<ID>,
    pub imports: Vec<ID>,
    pub parent: Option<ID>,
    pub imported: bool,
}

impl Module {
    pub fn new(id: ID) -> Self {
        Self {
            id,
            ident: None,
            functions: Vec::default(),
            typedefs: Vec::default(),
            modules: Vec::default(),
            imports: Vec::default(),
            parent: None,
            imported: false,
        }
    }

    pub fn typedefs<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map TypeDef> + 'this {
        self.typedefs.iter().map(|id| map.typedef(id))
    }

    pub fn imports<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Import> + 'this {
        self.imports.iter().map(|id| map.import(id))
    }

    pub fn functions<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Function> + 'this {
        self.functions.iter().map(|id| map.fn_(id))
    }

    pub fn modules<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Module> + 'this {
        self.modules.iter().map(|id| map.mod_(id))
    }
}

#[derive(Debug, Clone)]
pub struct Import {
    pub id: ID,
    pub parent: ID,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TypeRef {
    pub id: ID,
    pub kind: TypeRefKind,
}

#[derive(Debug, Clone)]
pub struct Name {
    pub id: ID,
    pub segments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub id: ID,
    pub identifier: String,
    pub members: Vec<TypeMember>,
    pub mod_: ID,
}

#[derive(Debug, Clone)]
pub struct TypeMember {
    pub ident: String,
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
    Named { name: ID },
    Pointer { pointee: ID },
}

#[derive(Debug, Clone)]
pub struct Function {
    pub id: ID,
    pub mod_: ID,
    pub identifier: String,
    pub parameters: Vec<ID>,
    pub body: Option<ID>,
    pub return_type: ID,
    pub is_var_args: bool,
    pub is_extern: bool,
}

impl Function {
    pub fn new(
        id: ID,
        module: ID,
        identifier: String,
        return_type: ID,
    ) -> Self {
        Self {
            id,
            mod_: module,
            identifier,
            parameters: Vec::default(),
            body: None,
            return_type,
            is_var_args: false,
            is_extern: false,
        }
    }

    pub fn mod_<'map>(&self, map: &'map Map) -> &'map Module {
        map.mod_(&self.mod_)
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

    pub fn body<'map>(&self, map: &'map Map) -> Option<&'map Block> {
        self.body.map(|id| map.block(&id))
    }

    pub fn full_name<'map>(&self, map: &'map Map) -> String {
        if self.is_extern {
            return self.identifier.clone();
        }
        let mut parts = Vec::new();
        let mut parent = Some(self.mod_);
        while let Some(mod_) = parent {
            let mod_ = map.mod_(&mod_);
            if let Some(ref name) = mod_.ident {
                parts.push(name.clone());
            }
            parent = mod_.parent;
        }
        parts.reverse();
        parts.push(self.identifier.clone());
        parts.join(".")
    }
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub id: ID,
    pub function: ID,
    pub ty: ID,
    pub identifier: String,
}

impl Parameter {
    pub fn new(id: ID, function: ID, ty: ID, identifier: String) -> Self {
        Self {
            id,
            function,
            ty,
            identifier,
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
pub struct Block {
    pub id: ID,
    pub label: Option<String>,
    pub kind: BlockKind,

    pub parent: Option<ID>,
    pub function: ID,

    pub lets: Vec<ID>,
    pub items: Vec<ID>,
    pub return_expr: Option<ID>,
}

impl Block {
    pub fn fn_<'map>(&self, map: &'map Map) -> &'map Function {
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
pub enum BlockKind {
    Function,
    Expr,
    Loop,
}

impl Block {
    pub fn new(
        id: ID,
        kind: BlockKind,
        parent: Option<ID>,
        function: ID,
        label: Option<String>,
    ) -> Self {
        Self {
            id,
            label,
            parent,
            function,
            kind,
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
    pub ident: String,
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
    pub fn name<'map>(&self, map: &'map Map) -> Option<&'map Name> {
        if let ExprKind::NameRef { id } = &self.kind {
            Some(map.name(id))
        } else {
            None
        }
    }

    pub fn op(&self) -> Option<&Op> {
        match &self.kind {
            ExprKind::Op(op) => Some(op),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(ID),
    NameRef {
        id: ID,
    },
    Cast {
        val: ID,
        to: ID,
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
        expr: Option<ID>,
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
        right: Option<ID>,
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
    Struct(StructLiteral),
}

#[derive(Debug, Clone)]
pub struct StructLiteral {
    pub name: ID,
    pub members: Vec<ID>,
}

#[derive(Debug, Clone)]
pub enum Op {
    Prefix {
        kind: PrefixOpKind,
        arg: ID,
    },
    Postfix {
        kind: PostfixOpKind,
        arg: ID,
    },
    Binary {
        kind: BinaryOpKind,
        lhs: ID,
        rhs: ID,
    },
}

impl Op {
    pub fn is_prefix(&self) -> bool {
        matches!(self, Op::Prefix { .. })
    }
    pub fn prefix_kind(&self) -> PrefixOpKind {
        match self {
            Op::Prefix { kind, .. } => *kind,
            _ => panic!("not a prefix op!"),
        }
    }
    pub fn prefix_arg(&self) -> ID {
        match self {
            Op::Prefix { arg, .. } => *arg,
            _ => panic!("not a prefix op!"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixOpKind {
    Plus,
    Negate,
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOpKind {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
    DotAccess,
    ArrowAccess,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    NotEquals,
    Equals,
    Assign,
}
