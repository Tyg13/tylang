#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ID(usize);

pub struct TypeMap {
    types: Vec<Type>,
}

pub struct Type {
    kind: TypeKind,
}

pub enum TypeKind {
    Void,
    Integer { size: usize },
    Pointer { pointee: ID },
    Struct { name: String, members: Vec<ID> },
}
