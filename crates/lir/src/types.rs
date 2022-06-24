use utils::vec_graph::{NodeRef, VecGraph};

pub type Block = NodeRef<usize>;
pub type BlockGraph = VecGraph<usize>;

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub types: Vec<Type>,
}

impl Module {
    pub fn types(&self) -> impl Iterator<Item = (TypeId, &Type)> {
        self.types
            .iter()
            .enumerate()
            .map(|(idx, ty)| (TypeId(idx), ty))
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    Void,
    Basic {
        name: String,
    },
    Integer {
        size: usize,
    },
    Pointer {
        target: TypeId,
    },
    Struct {
        name: String,
        members: Vec<StructMember>,
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct StructMember {
    pub name: String,
    pub typ_: TypeId,
}

impl Type {
    pub fn int_size(&self) -> usize {
        if let Type::Integer { size } = *self {
            size
        } else {
            panic!("tried to get int size of non-int type")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(pub usize);

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeId,
    pub is_var_args: bool,
    pub instructions: Vec<Instruction>,
    pub blocks: BlockGraph,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_: TypeId,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Nop,
    Declaration {
        name: String,
        type_: TypeId,
        value: Option<ValueOrOperation>,
        promoted: bool,
    },
    Call {
        function: String,
        operands: Vec<Value>,
    },
    Return {
        value: Value,
    },
    Jump {
        target: Block,
    },
    Branch {
        condition: Value,
        left: Block,
        right: Block,
    },
    Choice {
        left_value: Value,
        left: Block,
        right_value: Value,
        right: Block,
    },
    Truncate {
        to_type: TypeId,
        value: Value,
    },
    Extend {
        to_type: TypeId,
        value: Value,
    },
}

#[derive(Debug, Clone)]
pub enum Value {
    Void,
    Literal(Literal),
    VariableRef(usize),
    ParamRef(usize),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(usize),
    Str(String),
}

#[derive(Debug, Clone)]
pub enum ValueOrOperation {
    Value(Value),
    Operation(Operation),
}

impl ValueOrOperation {
    pub fn as_value(self) -> Value {
        match self {
            Self::Value(v) => v,
            _ => panic!("tried to get value from operation!"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub kind: OperationKind,
    pub operands: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Add,
    Subtract,
    Multiply,
    Divide,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    Equals,
    Index,
    Assignment,
}
