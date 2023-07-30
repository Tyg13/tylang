use crate::errors::Error;
use assert_matches::debug_assert_matches;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ID(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Module,
    Type,
    TypeMember,
    Function,
    Param,
    Var,
    Block,
    Constant,
    Expr,
    Error,

    Tombstone,
}

#[derive(Debug, Default)]
pub struct Map {
    nodes: Vec<Kind>,
    tombstones: Vec<ID>,

    assigned_type: HashMap<ID, ID>,
    marked_ids: HashMap<ID, Vec<ID>>,
    parents: HashMap<ID, ID>,
    constant_exprs: HashMap<ID, ID>,
    callee_to_callers: HashMap<ID, HashSet<ID>>,

    pub(crate) builtins: Builtins,

    types: HashMap<ID, Type>,
    names: HashMap<ID, Name>,
    namespaces: HashMap<ID, Namespace>,
    functions: HashMap<ID, Function>,
    errors: HashMap<ID, Error>,
    params: HashMap<ID, Param>,
    vars: HashMap<ID, Var>,
    constants: HashMap<ID, Constant>,

    birs: HashMap<ID, bir::ID>,
    associated_bir_ids: HashMap<bir::ID, Vec<ID>>,
}

impl Map {
    pub fn nodes(&self) -> impl Iterator<Item = (ID, Kind)> + '_ {
        self.nodes.iter().enumerate().filter_map(|(idx, &k)| {
            (k != Kind::Tombstone).then_some((ID(idx), k))
        })
    }

    pub fn kind(&self, id: ID) -> Kind {
        self.nodes
            .get(id.0)
            .cloned()
            .expect("Internal map inconsistency")
    }

    pub fn types(&self) -> impl Iterator<Item = &Type> + '_ {
        self.types.values()
    }

    pub fn names(&self) -> impl Iterator<Item = &Name> + '_ {
        self.names.values()
    }

    pub fn any_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn errors(&self) -> impl Iterator<Item = &Error> + '_ {
        self.errors.values()
    }

    pub fn parent(&self, id: ID) -> Option<ID> {
        self.parents.get(&id).copied()
    }

    pub fn bir(&self, id: ID) -> Option<bir::ID> {
        self.birs.get(&id).copied()
    }

    pub fn fn_(&self, id: ID) -> Option<&Function> {
        self.functions.get(&id)
    }

    pub fn name(&self, id: ID) -> Option<&Name> {
        self.names.get(&id)
    }

    pub fn param(&self, id: ID) -> Option<&Param> {
        self.params.get(&id)
    }

    pub fn var(&self, id: ID) -> Option<&Var> {
        self.vars.get(&id)
    }

    pub fn err(&self, id: ID) -> Option<&Error> {
        self.errors.get(&id)
    }

    pub fn ns(&self, id: ID) -> Option<&Namespace> {
        self.namespaces.get(&id)
    }

    pub fn bir_to_id(&self, bir: &bir::ID) -> Option<ID> {
        self.associated_bir_ids
            .get(bir)
            .and_then(|ids| ids.first())
            .copied()
    }

    pub fn is_err(&self, id: ID) -> bool {
        self.err(id).is_some()
    }

    pub fn ty(&self, id: ID) -> Option<&Type> {
        self.ty_id(id).and_then(|id| {
            debug_assert_matches!(self.kind(id), Kind::Type | Kind::Error);
            self.types.get(&id)
        })
    }

    pub fn ty_member(&self, id: ID) -> TypeMember {
        debug_assert_eq!(self.kind(id), Kind::TypeMember);
        TypeMember { id }
    }

    pub fn ty_id(&self, id: ID) -> Option<ID> {
        if self.kind(id) == Kind::Type {
            return Some(id);
        }
        self.assigned_type.get(&id).copied()
    }

    pub fn constant(&self, mut id: ID) -> Option<&Constant> {
        debug_assert_matches!(self.kind(id), Kind::Constant | Kind::Expr);
        if self.kind(id) == Kind::Expr {
            id = self.constant_exprs[&id];
        }
        self.constants.get(&id)
    }

    pub(crate) fn add_caller(&mut self, caller: ID, callee: ID) {
        debug_assert_eq!(self.kind(caller), Kind::Function);
        debug_assert_eq!(self.kind(callee), Kind::Function);
        self.callee_to_callers
            .entry(callee)
            .or_default()
            .insert(caller);
    }

    pub fn num_callers(&self, fn_: ID) -> usize {
        debug_assert_eq!(self.kind(fn_), Kind::Function);
        self.callee_to_callers
            .get(&fn_)
            .map_or(0, |callers| callers.len())
    }

    pub(crate) fn ns_mut(&mut self, id: ID) -> Option<NamespaceHandle<'_>> {
        if !self.namespaces.contains_key(&id) {
            return None;
        }
        Some(NamespaceHandle { id, map: self })
    }

    pub(crate) fn resolve_marker(&mut self, marker: ID, resolved_ty: ID) {
        debug_assert_eq!(self.kind(resolved_ty), Kind::Type);
        debug_assert_matches!(self.ty(marker).unwrap().kind, TypeKind::Marker);
        let mut marked_ids = self.marked_ids.remove(&marker).unwrap();
        for id in marked_ids.iter() {
            *self.assigned_type.get_mut(id).unwrap() = resolved_ty;
        }
        if self.ty(resolved_ty).unwrap().is_marker() {
            // If the type we're resolving to is a marker, add this marker's ids to the
            // newly-resolved marker's ids
            let new_marked_ids = self.marked_ids.get_mut(&resolved_ty).unwrap();
            new_marked_ids.append(&mut marked_ids);
        }
        self.remove_node(marker);
        self.types.remove(&marker);
    }

    fn remove_node(&mut self, id: ID) {
        self.nodes[id.0] = Kind::Tombstone;
        self.tombstones.push(id);
    }

    pub(crate) fn new_node(&mut self, kind: Kind) -> ID {
        let id = match self.tombstones.pop() {
            None => {
                let new = ID(self.nodes.len());
                self.nodes.push(kind);
                new
            }
            Some(id) => {
                self.nodes[id.0] = kind;
                id
            }
        };
        match kind {
            Kind::Module | Kind::Function | Kind::Block => {
                self.namespaces.insert(id, Namespace::empty(id));
            }
            Kind::Param
            | Kind::Var
            | Kind::Type
            | Kind::Constant
            | Kind::Expr
            | Kind::TypeMember
            | Kind::Error => {}
            Kind::Tombstone => unreachable!("why are we making a tombstone?"),
        };
        id
    }

    pub(crate) fn new_constant(&mut self, ty: ID, data: Constant) -> ID {
        let id = self.new_node(Kind::Constant);
        self.set_ty(id, ty);
        self.constants.insert(id, data);
        id
    }

    pub(crate) fn new_ty(&mut self, kind: TypeKind) -> ID {
        let id = self.new_node(Kind::Type);
        match kind {
            TypeKind::Marker => {
                self.marked_ids.insert(id, Vec::new());
            }
            TypeKind::Aggregate(..) | TypeKind::Prototype => {
                self.namespaces.insert(id, Namespace::empty(id));
            }
            _ => {}
        }
        self.types.insert(id, Type { id, kind });
        id
    }

    pub(crate) fn set_ty(&mut self, id: ID, ty: ID) {
        debug_assert_matches!(self.kind(ty), Kind::Type | Kind::Error);
        self.assigned_type.insert(id, ty);
        if self.ty(ty).map_or(false, Type::is_marker) {
            self.marked_ids.get_mut(&ty).unwrap().push(id);
        }
    }

    pub(crate) fn set_err(&mut self, id: ID, err: Error) {
        self.errors.insert(id, err);
    }

    pub(crate) fn set_expr_constant(&mut self, expr: ID, const_: ID) {
        debug_assert_eq!(self.kind(expr), Kind::Expr);
        debug_assert_eq!(self.kind(const_), Kind::Constant);
        self.constant_exprs.insert(expr, const_);
    }

    pub(crate) fn set_parent(&mut self, id: ID, parent: ID) {
        self.parents.insert(id, parent);
    }

    fn set_name(&mut self, id: ID, name: Name) {
        self.names.insert(id, name);
    }

    pub(crate) fn set_bir(&mut self, id: ID, bir: bir::ID) {
        if let Some(old) = self.birs.insert(id, bir) {
            let kind = self.kind(id);
            panic!(
                "overwriting bir::{old:?} with bir::{bir:?} | sema::{id:?} ({kind:?})"
            );
        }
        self.associate_bir_with_id(bir, id);
    }

    pub(crate) fn associate_bir_with_id(&mut self, bir: bir::ID, id: ID) {
        self.associated_bir_ids.entry(bir).or_default().push(id);
    }

    pub fn try_get<T: FromMap>(&self, id: ID) -> Option<&T> {
        <T as FromMap>::try_get(id, self)
    }

    pub fn get<T: FromMap>(&self, id: ID) -> &T {
        <T as FromMap>::get(id, self)
    }
}

#[derive(Debug, Default)]
pub struct Builtins {
    pub(crate) void_type: Option<ID>,
    pub(crate) string_type: Option<ID>,
    pub(crate) bool_type: Option<ID>,
    pub(crate) index_type: Option<ID>,
    pub(crate) never_type: Option<ID>,
}

impl Map {
    pub fn void_type(&self) -> ID {
        self.builtins.void_type.expect("no void type builtin set?")
    }

    pub fn string_type(&self) -> ID {
        self.builtins.string_type.expect("no str type builtin set?")
    }

    pub fn bool_type(&self) -> ID {
        self.builtins.bool_type.expect("no bool type builtin set?")
    }

    pub fn index_type(&self) -> ID {
        self.builtins
            .index_type
            .expect("no index type builtin set?")
    }

    pub fn never_type(&self) -> ID {
        self.builtins
            .never_type
            .expect("no never type builtin set?")
    }
}

pub(crate) struct PrototypeTy {
    pub id: ID,
}

pub(crate) struct PrototypeFn {
    pub id: ID,
    pub bir: bir::ID,
    pub return_ty: ID,
}

impl PrototypeTy {
    pub(crate) fn finish(self, map: &mut Map, kind: TypeKind) -> ID {
        let ty = map.types.get_mut(&self.id).unwrap();
        debug_assert!(matches!(ty.kind, TypeKind::Prototype));
        ty.kind = kind;
        self.id
    }
}

impl PrototypeFn {
    pub(crate) fn finish(self, map: &mut Map, params: Vec<ID>) -> ID {
        let fn_ = map.functions.get_mut(&self.id).unwrap();
        debug_assert_eq!(fn_.prototype, true);
        fn_.params = params;
        fn_.prototype = false;
        self.id
    }
}

pub trait FromMap {
    fn try_get(id: ID, map: &Map) -> Option<&Self>;
    fn get(id: ID, map: &Map) -> &Self {
        Self::try_get(id, map).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct Type {
    pub id: ID,
    pub kind: TypeKind,
}

impl FromMap for Type {
    fn try_get(id: ID, map: &Map) -> Option<&Type> {
        map.ty(id)
    }
}

impl Type {
    pub fn as_fn_ty(&self) -> FunctionType {
        self.into_fn_ty().unwrap()
    }

    pub fn into_fn_ty(&self) -> Option<FunctionType> {
        if let TypeKind::Function(fn_ty) = &self.kind {
            Some(fn_ty.clone())
        } else {
            None
        }
    }

    pub fn as_aggregate_ty(&self) -> AggregateType {
        self.into_aggregate_ty().unwrap()
    }

    pub fn into_aggregate_ty(&self) -> Option<AggregateType> {
        if let TypeKind::Aggregate(aggregate) = &self.kind {
            Some(aggregate.clone())
        } else {
            None
        }
    }

    pub fn name<'map>(&self, map: &'map Map) -> Option<&'map Name> {
        map.try_get(self.id)
    }

    pub fn ident<'map>(&self, map: &'map Map) -> Option<&'map str> {
        self.name(map).map(|n| n.ident.as_str())
    }

    pub fn int_size(&self) -> usize {
        if let TypeKind::Integer { size } = self.kind {
            size
        } else {
            panic!("Not an integer type!")
        }
    }

    pub fn pointee(&self) -> ID {
        if let TypeKind::Pointer { pointee } = self.kind {
            pointee
        } else {
            panic!("Not a pointer type: {:?}!", self.kind)
        }
    }

    pub fn repr(&self, map: &Map) -> String {
        let ident = self.ident(map);
        match &self.kind {
            TypeKind::Void => "void".to_string(),
            TypeKind::Never => "!".to_string(),
            TypeKind::Integer { size } => format!("i{size}"),
            TypeKind::Pointer { pointee } => {
                format!("*{}", map.ty(*pointee).unwrap().repr(map))
            }
            TypeKind::String => "str".to_string(),
            TypeKind::Aggregate(..) => ident.unwrap().to_string(),
            TypeKind::Function(fn_ty) => {
                let mut member_str = fn_ty
                    .param_tys(map)
                    .map(|ty| ty.repr(map))
                    .collect::<Vec<_>>()
                    .join(", ");
                if fn_ty.is_var_args {
                    if member_str.is_empty() {
                        member_str = String::from("...");
                    } else {
                        member_str.push_str(", ...");
                    }
                }
                let return_str = fn_ty
                    .return_ty(map)
                    .map_or("<err>".to_string(), |ty| ty.repr(map));
                format!("fn ({member_str}) -> {return_str}")
            }
            TypeKind::Marker => "{inferred}".to_string(),
            TypeKind::Prototype => unreachable!(),
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self.kind, TypeKind::Integer { .. })
    }

    pub fn is_ptr(&self) -> bool {
        matches!(self.kind, TypeKind::Pointer { .. })
    }

    pub fn is_void(&self) -> bool {
        matches!(self.kind, TypeKind::Void)
    }

    pub fn is_marker(&self) -> bool {
        matches!(self.kind, TypeKind::Marker)
    }

    pub fn is_aggregate(&self) -> bool {
        matches!(self.kind, TypeKind::Aggregate(..))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasedType {
    pub id: ID,
    pub kind: BasedTypeKind,
    pub based_on: ID,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BasedTypeKind {
    Pointer,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Void,
    Never,
    Integer { size: usize },
    Pointer { pointee: ID },
    String,
    Aggregate(AggregateType),
    Function(FunctionType),

    // Only used during checking
    Prototype,
    Marker,
}

#[derive(Debug, Clone)]
pub struct AggregateType {
    pub id: ID,
    pub members: Vec<ID>,
}

impl AggregateType {
    pub fn offset_of(&self, id: ID) -> usize {
        self.members
            .iter()
            .position(|&member| member == id)
            .expect("not a member of this struct!")
    }

    pub fn members<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Type> + 'this {
        self.members.iter().map(|id| map.get(*id))
    }

    pub fn name<'map>(&self, map: &'map Map) -> Option<&'map Name> {
        map.try_get(self.id)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub return_ty: ID,
    pub parameters: Vec<ID>,
    pub is_var_args: bool,
}

impl FunctionType {
    pub fn return_ty<'map>(&self, map: &'map Map) -> Option<&'map Type> {
        map.try_get(self.return_ty)
    }

    pub fn param_tys<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Type> + 'this {
        self.parameters.iter().map(|id| map.get(*id))
    }
}

pub struct TypeMember {
    pub id: ID,
}

impl TypeMember {
    pub fn parent<'map>(&self, map: &'map Map) -> &'map Type {
        map.ty(map.parent(self.id).unwrap()).unwrap()
    }

    pub fn offset<'map>(&self, map: &'map Map) -> usize {
        self.parent(map).as_aggregate_ty().offset_of(self.id)
    }
}

#[derive(Debug, Clone)]
pub struct Var {
    pub id: ID,
    pub ns: ID,
    pub idx: usize,
}

#[derive(Debug)]
pub enum Constant {
    Int(usize),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub id: ID,
    pub return_ty: ID,
    pub params: Vec<ID>,
    pub prototype: bool,
}

impl Function {
    pub fn name<'map>(&self, map: &'map Map) -> Option<&'map Name> {
        map.try_get::<Name>(self.id)
    }

    pub fn is_var_args(&self, map: &Map) -> bool {
        map.ty(self.id).unwrap().as_fn_ty().is_var_args
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub id: ID,
    pub ns: ID,
    pub idx: usize,
}

impl Param {
    pub fn name<'map>(&self, map: &'map Map) -> &'map Name {
        map.name(self.id).unwrap()
    }

    pub fn ident<'map>(&self, map: &'map Map) -> &'map str {
        &self.name(map).ident
    }

    pub fn ty<'map>(&self, map: &'map Map) -> &'map Type {
        map.ty(self.id).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct Name {
    pub id: ID,
    pub ident: String,
}

impl FromMap for Name {
    fn try_get(id: ID, map: &Map) -> Option<&Self> {
        map.name(id)
    }
}

impl Name {
    pub fn kind(&self, map: &Map) -> Kind {
        map.kind(self.id)
    }

    pub fn ty<'map>(&self, map: &'map Map) -> Option<&'map Type> {
        map.try_get(self.id)
    }
}

#[derive(Debug)]
pub struct Namespace {
    pub id: ID,
    members: Vec<ID>,

    params: Vec<ID>,
    vars: Vec<ID>,
}

impl Namespace {
    fn empty(id: ID) -> Self {
        Self {
            id,
            members: Vec::new(),
            params: Vec::default(),
            vars: Vec::default(),
        }
    }

    pub fn kind(&self, map: &Map) -> Kind {
        map.kind(self.id)
    }

    pub fn param<'map>(
        &self,
        idx: usize,
        map: &'map Map,
    ) -> Option<&'map Name> {
        self.params.get(idx).and_then(|id| map.try_get(*id))
    }

    pub fn var<'map>(&self, idx: usize, map: &'map Map) -> Option<&'map Name> {
        self.vars.get(idx).and_then(|id| map.name(*id))
    }

    pub fn parent<'map>(&self, map: &'map Map) -> Option<&'map Namespace> {
        map.parent(self.id).map(|id| map.ns(id).unwrap())
    }

    pub fn lookup<'map>(
        &self,
        map: &'map Map,
        ident: &str,
        check_parents: bool,
    ) -> Option<&'map Name> {
        for id in self.members.iter().rev() {
            let name = map.name(*id).unwrap();
            if name.ident == ident {
                return Some(name);
            }
        }
        if check_parents {
            if let Some(parent) = self.parent(map) {
                return parent.lookup(map, ident, true);
            }
        }
        None
    }
}

#[derive(Debug)]
pub(crate) struct NamespaceHandle<'map> {
    pub id: ID,
    map: &'map mut Map,
}

impl<'map> NamespaceHandle<'map> {
    fn ns(&mut self) -> &mut Namespace {
        self.map.namespaces.get_mut(&self.id).unwrap()
    }

    fn push_and_get_idx(v: &mut Vec<ID>, id: ID) -> usize {
        let idx = v.len();
        v.push(id);
        idx
    }

    fn set_param(&mut self, id: ID, idx: usize) {
        self.map.params.insert(
            id,
            Param {
                id,
                idx,
                ns: self.id,
            },
        );
    }

    fn set_var(&mut self, id: ID, idx: usize) {
        self.map.vars.insert(
            id,
            Var {
                id,
                idx,
                ns: self.id,
            },
        );
    }

    fn set_parent_of(&mut self, id: ID) {
        self.map.set_parent(id, self.id);
    }

    pub(crate) fn add_name(&mut self, id: ID, ident: &str) {
        self.map.set_name(
            id,
            Name {
                id,
                ident: ident.to_string(),
            },
        );
        self.ns().members.push(id);
    }

    pub(crate) fn new_ty(&mut self, ident: Option<&str>, kind: TypeKind) -> ID {
        let id = self.map.new_ty(kind);
        if let Some(ident) = ident {
            self.add_name(id, ident);
        }
        self.set_parent_of(id);
        id
    }

    pub(crate) fn new_node(&mut self, kind: Kind) -> ID {
        let id = self.map.new_node(kind);
        self.set_parent_of(id);
        id
    }

    pub(crate) fn new_ty_member(&mut self, ident: &str, ty: ID) -> ID {
        let id = self.new_node(Kind::TypeMember);
        self.add_name(id, ident);
        self.map.set_ty(id, ty);
        id
    }

    pub(crate) fn new_ty_proto(&mut self, ident: Option<&str>) -> PrototypeTy {
        PrototypeTy {
            id: self.new_ty(ident, TypeKind::Prototype),
        }
    }

    pub(crate) fn new_fn_proto(
        &mut self,
        ident: &str,
        bir: bir::ID,
        return_ty: ID,
    ) -> PrototypeFn {
        let id = self.new_node(Kind::Function);
        self.add_name(id, ident);
        self.map.functions.insert(
            id,
            Function {
                id,
                return_ty,
                params: Vec::new(),
                prototype: true,
            },
        );
        PrototypeFn { id, bir, return_ty }
    }

    pub(crate) fn new_param(&mut self, ident: &str) -> ID {
        debug_assert_eq!(self.map.kind(self.id), Kind::Function);
        let id = self.new_node(Kind::Param);
        let idx = Self::push_and_get_idx(&mut self.ns().params, id);
        self.add_name(id, ident);
        self.set_param(id, idx);
        id
    }

    pub(crate) fn new_var(&mut self, ident: &str) -> ID {
        debug_assert_eq!(self.map.kind(self.id), Kind::Block);
        let id = self.new_node(Kind::Var);
        let idx = Self::push_and_get_idx(&mut self.ns().vars, id);
        self.add_name(id, ident);
        self.set_var(id, idx);
        id
    }

    pub(crate) fn new_block(&mut self) -> ID {
        self.new_node(Kind::Block)
    }

    pub(crate) fn new_module(&mut self, name: Option<&str>) -> ID {
        let id = self.new_node(Kind::Module);
        if let Some(name) = name {
            self.add_name(id, name);
        }
        id
    }
}
