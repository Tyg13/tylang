use super::*;
use std::convert::TryFrom;
use std::rc::Rc;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum BuiltinType {
    I8,
    I16,
    I32,
    I64,
}

#[derive(Clone, PartialEq, Debug)]
pub enum TypeKind {
    Builtin(BuiltinType),
    Pointer(Rc<Type>),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Type {
    pub span: Span,
    pub kind: TypeKind,
}

impl TryFrom<&str> for TypeKind {
    type Error = ();
    fn try_from(s: &str) -> std::result::Result<Self, ()> {
        Ok(match s {
            "i8" => TypeKind::Builtin(BuiltinType::I8),
            "i16" => TypeKind::Builtin(BuiltinType::I16),
            "i32" => TypeKind::Builtin(BuiltinType::I32),
            "i64" => TypeKind::Builtin(BuiltinType::I64),
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match &self.kind {
            TypeKind::Builtin(BuiltinType::I8) => String::from("i8"),
            TypeKind::Builtin(BuiltinType::I16) => String::from("i16"),
            TypeKind::Builtin(BuiltinType::I32) => String::from("i32"),
            TypeKind::Builtin(BuiltinType::I64) => String::from("i64"),
            TypeKind::Pointer(type_) => type_.to_string(),
        };
        write!(f, "{}", repr)
    }
}

impl Parser<'_> {
    pub(super) fn parse_type(&mut self) -> Result<Type> {
        if let Some(star) = self.maybe(TokenKind::Star) {
            let inner = self.parse_type()?;
            return Ok(Type {
                span: span!(star, inner),
                kind: TypeKind::Pointer(Rc::new(inner)),
            });
        }
        let token = self.expect(TokenKind::Identifier)?;
        let ident = self.ident(token)?;
        if let Ok(kind) = TypeKind::try_from(ident.as_str()) {
            Ok(Type {
                span: token.span,
                kind,
            })
        } else {
            Err(Error::UnexpectedToken(format!("type")))
        }
    }
}
