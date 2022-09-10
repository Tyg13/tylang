use crate::errors::Error;
use std::{assert_matches::debug_assert_matches, collections::HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ID(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Module,
    Type,
    Function,
    Param,
    Var,
    Block,
    Other,
    Constant,
    Error,

    Tombstone,
}

pub trait HasKind {
    const KIND: Kind;
}

#[derive(Debug, Default)]
pub struct Map {
    nodes: Vec<Kind>,
    tombstones: Vec<ID>,

    assigned_type: HashMap<ID, ID>,
    containing_ns: HashMap<ID, ID>,
    casts: HashMap<ID, ID>,

    types: HashMap<ID, Type>,
    names: HashMap<ID, Name>,
    namespaces: HashMap<ID, Namespace>,
    functions: HashMap<ID, Function>,
    errors: HashMap<ID, Error>,
    birs: HashMap<ID, bir::ID>,
    params: HashMap<ID, Param>,
    vars: HashMap<ID, Var>,
    constants: HashMap<ID, Constant>,

    bir_to_id: HashMap<bir::ID, ID>,
}

impl Map {
    pub fn bir_map(&self) -> impl Iterator<Item = (bir::ID, ID)> + '_ {
        self.bir_to_id.iter().map(|(&bir, &id)| (bir, id))
    }

    pub fn nodes(&self) -> impl Iterator<Item = (ID, Kind)> + '_ {
        self.nodes.iter().enumerate().map(|(idx, &k)| (ID(idx), k))
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

    pub fn errors(&self) -> impl Iterator<Item = &Error> + '_ {
        self.errors.values()
    }

    pub fn containing_ns_id(&self, id: ID) -> Option<ID> {
        self.containing_ns.get(&id).copied()
    }

    pub fn containing_ns(&self, id: ID) -> Option<&Namespace> {
        self.containing_ns_id(id)
            .map(|id| self.ns(id).expect("internal namespace map inconsistency!"))
    }

    pub fn fn_(&self, id: ID) -> Option<&Function> {
        self.functions.get(&id)
    }

    pub fn bir(&self, id: ID) -> Option<bir::ID> {
        self.birs.get(&id).copied()
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

    pub fn bir_to_id(&self, bir: bir::ID) -> Option<ID> {
        self.bir_to_id.get(&bir).copied()
    }

    pub fn is_err(&self, id: ID) -> bool {
        self.err(id).is_some()
    }

    pub fn ty(&self, id: ID) -> Option<&Type> {
        self.ty_id(id).map(|id| self.types.get(&id).unwrap())
    }

    pub fn ty_id(&self, id: ID) -> Option<ID> {
        if self.kind(id) == Kind::Type {
            return Some(id);
        }
        self.assigned_type.get(&id).copied()
    }

    pub(crate) fn new_ty(&mut self, kind: TypeKind) -> ID {
        let id = self.new_node(Kind::Type);
        self.types.insert(id, Type { id, kind });
        id
    }

    pub(crate) fn resolve_marker(&mut self, marker: ID, resolved_ty: ID) {
        debug_assert_eq!(self.kind(marker), Kind::Type);
        debug_assert_eq!(self.kind(resolved_ty), Kind::Type);
        debug_assert_matches!(self.ty(marker).unwrap().kind, TypeKind::Marker);
        for ty in self.assigned_type.values_mut() {
            if *ty == marker {
                *ty = resolved_ty;
            }
        }
        self.mark_tombstone(marker);
        self.types.remove(&marker);
    }

    fn mark_tombstone(&mut self, id: ID) {
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
            Kind::Param | Kind::Var | Kind::Type | Kind::Other | Kind::Error | Kind::Constant => {}
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

    pub fn constant(&self, id: ID) -> Option<&Constant> {
        debug_assert_eq!(self.kind(id), Kind::Constant);
        self.constants.get(&id)
    }

    pub(crate) fn ns_mut(&mut self, id: ID) -> Option<NamespaceHandle<'_>> {
        if !self.namespaces.contains_key(&id) {
            return None;
        }
        Some(NamespaceHandle { id, map: self })
    }

    pub(crate) fn set_ty(&mut self, id: ID, ty: ID) {
        debug_assert_matches!(self.kind(ty), Kind::Type | Kind::Error);
        self.assigned_type.insert(id, ty);
    }

    pub fn cast_ty(&self, id: ID) -> Option<ID> {
        self.casts.get(&id).copied()
    }

    pub(crate) fn set_cast(&mut self, id: ID, to_ty: ID) {
        debug_assert!(self.ty(id).is_some());
        debug_assert!(self.ty(to_ty).is_some());
        dbg!(id, to_ty);
        self.casts.insert(id, to_ty);
    }

    pub(crate) fn set_err(&mut self, id: ID, err: Error) {
        self.errors.insert(id, err);
    }

    fn set_name(&mut self, id: ID, name: Name) {
        self.names.insert(id, name);
    }

    pub(crate) fn set_bir(&mut self, id: ID, bir: bir::ID) {
        self.birs.insert(id, bir);
        self.bir_to_id.insert(bir, id);
    }

    pub fn try_get<T: FromMap>(&self, id: ID) -> Option<&T> {
        <T as FromMap>::try_get(id, self)
    }

    pub fn get<T: FromMap>(&self, id: ID) -> &T {
        <T as FromMap>::get(id, self)
    }

    pub(crate) fn any_markers_or_prototypes(&self) -> bool {
        self.types
            .values()
            .any(|ty| matches!(ty.kind, TypeKind::Prototype | TypeKind::Marker))
    }
}

pub(crate) struct PrototypeTy {
    pub id: ID,
}

pub(crate) struct PrototypeFn {
    pub id: ID,
    pub bir: bir::ID,
    pub return_: ID,
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
        fn_.params = params;
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

impl HasKind for Type {
    const KIND: Kind = Kind::Type;
}

impl FromMap for Type {
    fn try_get(id: ID, map: &Map) -> Option<&Type> {
        map.ty(id)
    }
}

impl Type {
    pub fn as_fn_ty(&self) -> FunctionType {
        if let TypeKind::Function(fn_ty) = &self.kind {
            fn_ty.clone()
        } else {
            panic!("Not a function type!")
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

    pub fn pointee(&self) -> Option<ID> {
        if let TypeKind::Pointer { pointee } = self.kind {
            Some(pointee)
        } else {
            None
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
            TypeKind::Aggregate { members: _ } => ident.unwrap().to_string(),
            TypeKind::Function(fn_ty) => {
                let member_str = fn_ty
                    .param_tys(map)
                    .map(|ty| ty.repr(map))
                    .collect::<Vec<_>>()
                    .join(", ");
                let return_str = fn_ty.return_ty(map).repr(map);
                format!("fn ({member_str}) -> {return_str}")
            }
            kind @ (TypeKind::Prototype | TypeKind::Marker) => unreachable!("{kind:?}"),
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
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Void,
    Never,
    Integer { size: usize },
    Pointer { pointee: ID },
    String,
    Aggregate { members: Vec<ID> },
    Function(FunctionType),

    // Only used during checking
    Prototype,
    Marker,
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub return_ty: ID,
    pub parameters: Vec<ID>,
}

impl FunctionType {
    pub fn return_ty<'map>(&self, map: &'map Map) -> &'map Type {
        map.get(self.return_ty)
    }

    pub fn param_tys<'this, 'map: 'this>(
        &'this self,
        map: &'map Map,
    ) -> impl Iterator<Item = &'map Type> + 'this {
        self.parameters.iter().map(|id| map.get(*id))
    }
}

#[derive(Debug, Clone)]
pub struct Var {
    pub id: ID,
    pub ns: ID,
    pub idx: usize,
}

impl HasKind for Var {
    const KIND: Kind = Kind::Var;
}

#[derive(Debug)]
pub enum Constant {
    Int(usize),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub id: ID,
    pub return_: ID,
    pub params: Vec<ID>,
}

impl HasKind for Function {
    const KIND: Kind = Kind::Function;
}

impl Function {
    pub fn name<'map>(&self, map: &'map Map) -> Option<&'map Name> {
        map.try_get::<Name>(self.id)
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

impl HasKind for Param {
    const KIND: Kind = Kind::Param;
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

    pub fn param<'map>(&self, idx: usize, map: &'map Map) -> Option<&'map Name> {
        self.params.get(idx).and_then(|id| map.try_get(*id))
    }

    pub fn var<'map>(&self, idx: usize, map: &'map Map) -> Option<&'map Name> {
        self.vars.get(idx).and_then(|id| map.name(*id))
    }

    pub fn parent<'map>(&self, map: &'map Map) -> Option<&'map Namespace> {
        map.containing_ns(self.id)
    }

    pub fn lookup_ty<'map>(&self, map: &'map Map, ident: &str) -> Option<&'map Type> {
        self.lookup(map, ident).and_then(|name| name.ty(map))
    }

    pub fn lookup<'map>(&self, map: &'map Map, ident: &str) -> Option<&'map Name> {
        for &id in self.members.iter().rev() {
            let name = map.name(id).unwrap();
            if name.ident == ident {
                return Some(name);
            }
        }
        if let Some(parent) = self.parent(map) {
            return parent.lookup(map, ident);
        }
        None
    }

    pub fn get<'map, T: HasKind + FromMap>(&self, map: &'map Map, ident: &str) -> Option<&'map T> {
        self.lookup(map, ident).map(|name| {
            let id = name.id;
            debug_assert_eq!(<T as HasKind>::KIND, map.kind(id));
            <T as FromMap>::get(id, map)
        })
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

    fn add_name(&mut self, id: ID, ident: &str) {
        self.map.set_name(
            id,
            Name {
                id,
                ident: ident.to_string(),
            },
        );
        self.ns().members.push(id);
    }

    fn add_and_get_idx(v: &mut Vec<ID>, id: ID) -> usize {
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

    fn set_ns(&mut self, id: ID) {
        self.map.containing_ns.insert(id, self.id);
    }

    pub(crate) fn new_ty(&mut self, ident: Option<&str>, kind: TypeKind) -> ID {
        let id = self.map.new_ty(kind);
        if let Some(ident) = ident {
            self.add_name(id, ident);
        }
        self.set_ns(id);
        id
    }

    pub(crate) fn new_ty_proto(&mut self, ident: Option<&str>) -> PrototypeTy {
        PrototypeTy {
            id: self.new_ty(ident, TypeKind::Prototype),
        }
    }

    pub(crate) fn new_fn_proto(&mut self, ident: &str, bir: bir::ID, return_: ID) -> PrototypeFn {
        let id = self.map.new_node(Kind::Function);
        self.add_name(id, ident);
        self.map.functions.insert(
            id,
            Function {
                id,
                return_,
                params: Vec::new(),
            },
        );
        self.set_ns(id);
        PrototypeFn { id, bir, return_ }
    }

    pub(crate) fn new_param(&mut self, ident: &str) -> ID {
        debug_assert_eq!(self.map.kind(self.id), Kind::Function);
        let id = self.map.new_node(Kind::Param);
        let idx = Self::add_and_get_idx(&mut self.ns().params, id);
        self.add_name(id, ident);
        self.set_param(id, idx);
        self.set_ns(id);
        id
    }

    pub(crate) fn new_var(&mut self, ident: &str) -> ID {
        debug_assert_eq!(self.map.kind(self.id), Kind::Block);
        let id = self.map.new_node(Kind::Var);
        let idx = Self::add_and_get_idx(&mut self.ns().vars, id);
        self.add_name(id, ident);
        self.set_var(id, idx);
        self.set_ns(id);
        id
    }

    pub(crate) fn new_block(&mut self) -> ID {
        let id = self.map.new_node(Kind::Block);
        self.set_ns(id);
        id
    }
}
