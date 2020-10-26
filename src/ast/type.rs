use std::rc::Rc;

use crate::ast::{Parse, Parser};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Type(String),
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

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let repr = match &self.kind {
            TypeKind::Type(name) => name.clone(),
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
            let kind = TypeKind::Type(token.repr());
            Some(Type { kind })
        })
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;
    use std::rc::Rc;

    pub fn ty(ty: &str) -> Type {
        Type {
            kind: TypeKind::Type(ty.to_string()),
        }
    }

    pub fn ptr(ty: Type) -> Type {
        Type {
            kind: TypeKind::Pointer(Rc::new(ty)),
        }
    }

    #[test]
    fn pointer() {
        assert_eq!(ptr(ty("i64")), test::parse_one("*i64"));
        assert_eq!(ptr(ptr(ty("i8"))), test::parse_one("**i8"));
    }

    #[test]
    fn err() {
        assert_eq!(ptr(Type::error()), test::parse_one("*-"));
    }
}
