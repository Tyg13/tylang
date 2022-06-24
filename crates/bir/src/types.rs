use std::{assert_matches::assert_matches, rc::Rc};

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub types: Vec<TypeDefinition>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub identifier: String,
    pub parameters: Vec<Parameter>,
    pub body: Option<Scope>,
    pub return_type: Option<TypeRef>,
}

#[derive(Debug, Clone)]
pub enum Parameter {
    Named { name: String, typ_: TypeRef },
    VariableArgs,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum TypeRef {
    Void,
    Basic { name: String },
    Pointer { pointee: Rc<TypeRef> },
}

#[derive(Default, Debug, Clone)]
pub struct Scope {
    pub items: Vec<Item>,
    pub expr: Option<Expr>,
}

impl Scope {
    pub fn empty() -> Self {
        Default::default()
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Let {
        name: String,
        typ_: Option<TypeRef>,
        expr: Option<Expr>,
    },
    FnDef(Function),
    Expr(Expr),
    Type(TypeDefinition),
}

#[derive(Debug, Clone)]
pub struct TypeDefinition {
    pub name: String,
    pub members: Vec<TypeMember>,
}

#[derive(Debug, Clone)]
pub struct TypeMember {
    pub name: String,
    pub typ_: TypeRef,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    NameRef {
        name: String,
    },
    Call {
        receiver: Box<Expr>,
        operands: Vec<Expr>,
    },
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    Op(Op),
    Block {
        items: Vec<Item>,
    },
    Return {
        expr: Box<Expr>,
    },
    Branch {
        condition: Box<Expr>,
        left: Box<Scope>,
        right: Box<Scope>,
    },
    Loop {
        kind: LoopKind,
        body: Box<Scope>,
    },
}

impl Expr {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::NameRef { name } => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LoopKind {
    Loop,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(usize),
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Op {
    pub fixity: Fixity,
    pub kind: OpKind,
    pub operands: Vec<Expr>,
}

impl Op {
    pub fn lhs(&self) -> &Expr {
        assert_matches!(self.fixity, Fixity::Infix);
        self.operands.get(0).as_ref().unwrap()
    }
    pub fn rhs(&self) -> &Expr {
        assert_matches!(self.fixity, Fixity::Infix);
        self.operands.get(1).as_ref().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixity {
    Prefix,
    Postfix,
    Infix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Plus,
    Minus,
    Multiply,
    Divide,
    FieldAccess,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    Equals,
    Assignment,
}
