#[derive(Debug)]
pub struct Error {
    pub ids: Vec<crate::ID>,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    UnknownType,
    UnknownName,
    DuplicateType,
    DuplicateBinding,
    UnknownCall,
    Unification,
    InvalidIndexType,
    InvalidPointeeType,
    ParamAssignment,
}
