use super::*;
use crate::lexer::TokenKind;
use std::convert::TryFrom;
use std::rc::Rc;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Builtin {
    I8,
    I16,
    I32,
    I64,
}

#[derive(Clone, PartialEq, Debug)]
pub enum TypeKind {
    Builtin(Builtin),
    Pointer(Rc<Type>),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Type {
    pub kind: TypeKind,
}

impl TryFrom<&str> for TypeKind {
    type Error = ();
    fn try_from(s: &str) -> std::result::Result<Self, ()> {
        Ok(match s {
            "i8" => TypeKind::Builtin(Builtin::I8),
            "i16" => TypeKind::Builtin(Builtin::I16),
            "i32" => TypeKind::Builtin(Builtin::I32),
            "i64" => TypeKind::Builtin(Builtin::I64),
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match &self.kind {
            TypeKind::Builtin(Builtin::I8) => String::from("i8"),
            TypeKind::Builtin(Builtin::I16) => String::from("i16"),
            TypeKind::Builtin(Builtin::I32) => String::from("i32"),
            TypeKind::Builtin(Builtin::I64) => String::from("i64"),
            TypeKind::Pointer(type_) => type_.to_string(),
        };
        write!(f, "{}", repr)
    }
}

impl Parser {
    pub(super) fn parse_type(&self, cursor: &mut Cursor) -> Option<Type> {
        if let Some(star) = cursor.maybe_token(TokenKind::Star) {
            let inner = self.parse_type(cursor)?;
            return Some(Type {
                kind: TypeKind::Pointer(Rc::new(inner)),
            });
        }
        let token = cursor.expect_token(TokenKind::Identifier);
        let ident = self.map.ident(&token)?;
        if let Ok(kind) = TypeKind::try_from(ident.as_str()) {
            Some(Type { kind })
        } else {
            panic!("Unrecognized type {}", ident);
        }
    }
}
