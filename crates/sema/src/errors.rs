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
    CallToNonFnType,
    Unification,
    InvalidIndexType,
    InvalidPointeeType,
    ParamAssignment,
    InvalidField,
    InvalidCallReceiver,
    InvalidFieldReceiver,
}

impl Error {
    pub fn render(&self, replacements: &[String]) -> String {
        match self.kind {
            ErrorKind::DuplicateBinding => {
                format!("duplicate binding: {}", replacements[0])
            }
            ErrorKind::UnknownType => {
                format!("unknown type: `{}`", replacements[0])
            }
            ErrorKind::UnknownName => {
                format!("unknown name: `{}`", replacements[0])
            }
            ErrorKind::DuplicateType => {
                format!("redefined type: `{}`", replacements[0])
            }
            ErrorKind::UnknownCall => {
                format!("unknown call to `{}`", replacements[0])
            }
            ErrorKind::Unification => {
                format!(
                    "Can't unify:\n{}\n\n{}",
                    replacements[0], replacements[1]
                )
            }
            ErrorKind::InvalidIndexType => {
                format!(
                    "Can't index pointer with non-integer type!\n{}\n\n{}",
                    replacements[0], replacements[1]
                )
            }
            ErrorKind::InvalidPointeeType => {
                format!("Can't dereference non-pointer!\n{}", replacements[0])
            }
            ErrorKind::ParamAssignment => {
                format!("Can't assign to param!\n{}", replacements[0])
            }
            ErrorKind::CallToNonFnType => {
                format!("Not a function type!\n{}", replacements[0])
            }
            ErrorKind::InvalidField => {
                format!("Invalid field: `{}`", replacements[0])
            }
            ErrorKind::InvalidCallReceiver => {
                format!("Cannot call `{}`", replacements[0])
            }
            ErrorKind::InvalidFieldReceiver => {
                format!(
                    "Cannot index into `{}` as a struct:\n{}",
                    replacements[0], replacements[1]
                )
            }
        }
    }
}
