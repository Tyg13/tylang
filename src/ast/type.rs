use std::convert::TryFrom;
use std::rc::Rc;

use crate::ast::{Parse, Parser};
use crate::lexer::TokenKind;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    I8,
    I16,
    I32,
    I64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Builtin(Builtin),
    Pointer(Rc<Type>),
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Type {
    pub kind: TypeKind,
}

impl Type {
    pub fn is_error(&self) -> bool {
        self.kind == TypeKind::Error
    }

    pub fn error() -> Self {
        Self {
            kind: TypeKind::Error,
        }
    }
}

impl TryFrom<&str> for TypeKind {
    type Error = ();
    fn try_from(s: &str) -> std::result::Result<Self, <Self as TryFrom<&str>>::Error> {
        match s {
            "i8" => Ok(TypeKind::Builtin(Builtin::I8)),
            "i16" => Ok(TypeKind::Builtin(Builtin::I16)),
            "i32" => Ok(TypeKind::Builtin(Builtin::I32)),
            "i64" => Ok(TypeKind::Builtin(Builtin::I64)),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match &self.kind {
            TypeKind::Builtin(Builtin::I8) => String::from("i8"),
            TypeKind::Builtin(Builtin::I16) => String::from("i16"),
            TypeKind::Builtin(Builtin::I32) => String::from("i32"),
            TypeKind::Builtin(Builtin::I64) => String::from("i64"),
            TypeKind::Pointer(type_) => format!("*{}", type_.to_string()),
            TypeKind::Error => String::from("<err>"),
        };
        write!(f, "{}", repr)
    }
}

impl Parse for Type {
    fn parse(parser: &mut Parser) -> Option<Self> {
        if parser.maybe(TokenKind::Star).is_some() {
            let inner = parser.parse_one().unwrap_or(Type::error());
            return Some(Type {
                kind: TypeKind::Pointer(Rc::new(inner)),
            });
        }
        parser.some_or_backtrack(|parser| {
            let token = parser.expect(TokenKind::Identifier)?;
            let kind = TypeKind::try_from(token.repr().as_str()).ok()?;
            Some(Type { kind })
        })
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;
    use std::convert::TryFrom;
    use std::rc::Rc;

    pub fn ty(ty: &str) -> Type {
        let kind = TypeKind::try_from(ty).unwrap();
        Type { kind }
    }

    pub fn ptr(ty: Type) -> Type {
        Type {
            kind: TypeKind::Pointer(Rc::new(ty)),
        }
    }

    #[test]
    fn builtins() {
        for builtin in &["i8", "i16", "i32", "i64"] {
            assert_eq!(ty(builtin), test::parse_one(builtin));
        }
    }

    #[test]
    fn pointer() {
        assert_eq!(ptr(ty("i64")), test::parse_one("*i64"));
        assert_eq!(ptr(ptr(ty("i8"))), test::parse_one("**i8"));
    }

    #[test]
    fn err() {
        assert_eq!(ptr(Type::error()), test::parse_one("*test"));
    }
}
