use std::collections::HashMap;

use utils::vec_graph::VecGraph;

use crate::types::*;

pub fn translate_module(module: &bir::Module) -> Module {
    let mut type_context = TypeContext::new();
    register_default_types(&mut type_context);
    for typ_ in module.types.iter() {
        register_user_type(&mut type_context, typ_);
    }

    let mut functions = Vec::new();
    for function in module.functions.iter() {
        functions.push(translate_function(function, &mut type_context));
    }
    Module {
        functions,
        types: type_context.types,
    }
}

fn register_default_types(type_context: &mut TypeContext) {
    // Must be type 0 so `void_type()` works correctly
    type_context.register(&bir::TypeRef::Void, Type::Void);

    for basic_type in &["bool"] {
        type_context.register(
            &bir::TypeRef::Basic {
                name: basic_type.to_string(),
            },
            Type::Basic {
                name: basic_type.to_string(),
            },
        );
    }

    for size in [8usize, 16, 32, 64] {
        type_context.register(
            &bir::TypeRef::Basic {
                name: format!("i{size}"),
            },
            Type::Integer { size },
        );
    }
}

fn register_user_type(type_context: &mut TypeContext, type_def: &bir::TypeDefinition) {
    let mut members = Vec::new();
    for member in type_def.members.iter() {
        let name = member.name.to_string();
        let typ_ = type_context.lookup_or_register(&member.typ_);
        members.push(StructMember { name, typ_ });
    }
    let typ_ = Type::Struct {
        name: type_def.name.clone(),
        members,
    };
    type_context.register(
        &bir::TypeRef::Basic {
            name: type_def.name.clone(),
        },
        typ_,
    );
}

fn translate_function(function: &bir::Function, type_context: &mut TypeContext) -> Function {
    let mut is_var_args = false;
    let mut parameters = Vec::new();
    for param in function.parameters.iter() {
        match param {
            bir::Parameter::Named { name, typ_ } => {
                parameters.push(Parameter {
                    name: name.clone(),
                    type_: type_context.lookup_or_register(&typ_),
                });
            }
            bir::Parameter::VariableArgs => is_var_args = true,
        }
    }

    let return_type = if let Some(ref ty) = function.return_type {
        type_context.lookup_or_register(ty)
    } else {
        type_context.void_type()
    };

    let (instructions, blocks) = {
        let mut context = Context::new(type_context, &parameters);

        if let Some(ref body) = function.body {
            let (_, _, mut return_value) = context.add_instructions_from_block(body);
            if let Some(ref ty) = function.return_type {
                let return_type_id = context.type_context.lookup_or_register(ty);
                let return_type = &context.type_context.types[return_type_id.0];
                if let Type::Integer { .. } = return_type {
                    return_value = context.cast_if_necessary(return_value, return_type_id);
                }
            }
            context.add_instruction(Instruction::Return {
                value: return_value,
            });
        }

        utils::vec_graph::write_to_dot_file(
            &context.block_graph,
            &format!("{}.dot", function.identifier),
        )
        .unwrap();

        (context.instructions, context.block_graph)
    };

    Function {
        name: function.identifier.clone(),
        parameters,
        instructions,
        return_type,
        is_var_args,
        blocks,
    }
}

#[derive(Debug, Default)]
struct TypeContext {
    types: Vec<Type>,
    map: HashMap<bir::TypeRef, TypeId>,
}

impl TypeContext {
    fn new() -> Self {
        Default::default()
    }

    fn register(&mut self, type_ref: &bir::TypeRef, type_: Type) -> TypeId {
        let id = TypeId(self.types.len());
        self.map.insert(type_ref.clone(), id);
        self.types.push(type_);
        id
    }

    fn lookup_name(&self, name: &str) -> TypeId {
        if name == "void" {
            return self.void_type();
        }
        self.lookup(&bir::TypeRef::Basic {
            name: name.to_string(),
        })
        .unwrap()
    }

    fn lookup_or_register(&mut self, ty: &bir::TypeRef) -> TypeId {
        if let Some(id) = self.lookup(ty) {
            return id;
        }
        if let bir::TypeRef::Pointer { pointee } = ty {
            let target = self.lookup_or_register(pointee);
            self.register(ty, Type::Pointer { target })
        } else {
            panic!("No such type {ty:#?}");
        }
    }

    fn lookup(&self, ty: &bir::TypeRef) -> Option<TypeId> {
        self.map.get(ty).copied()
    }

    fn void_type(&self) -> TypeId {
        assert_eq!(self.types[0], Type::Void);
        TypeId(0)
    }
}

#[derive(Debug)]
struct Context<'module> {
    type_context: &'module mut TypeContext,
    instructions: Vec<Instruction>,
    block_graph: VecGraph<usize>,
    parameters: &'module Vec<Parameter>,
    definitions: HashMap<String, Def>,
}

#[derive(Debug, Clone)]
struct Def {
    kind: DefKind,
    type_: TypeId,
    index: usize,
}

#[derive(Debug, Clone)]
enum DefKind {
    Variable,
    Param,
}

impl Def {
    fn to_ref(&self) -> Value {
        match self.kind {
            DefKind::Variable => Value::VariableRef(self.index),
            DefKind::Param => Value::ParamRef(self.index),
        }
    }
}

struct Marker {
    basic_block: Block,
    idx: usize,
}

impl Marker {
    #[must_use]
    fn new(context: &mut Context) -> Self {
        Self {
            basic_block: context.block_graph.last().unwrap(),
            idx: context.add_instruction(Instruction::Nop),
        }
    }

    fn containing_block(&self) -> Block {
        self.basic_block
    }

    fn replace(self, context: &mut Context, new: Instruction) -> usize {
        match &mut context.instructions[self.idx] {
            inst @ Instruction::Nop => *inst = new,
            _ => unreachable!(),
        }
        self.idx
    }
}

impl<'module> Context<'module> {
    pub fn new(
        type_context: &'module mut TypeContext,
        parameters: &'module Vec<Parameter>,
    ) -> Self {
        let mut definitions = HashMap::new();
        for (index, param) in parameters.iter().enumerate() {
            definitions.insert(
                param.name.clone(),
                Def {
                    kind: DefKind::Param,
                    type_: param.type_,
                    index,
                },
            );
        }
        Self {
            type_context,
            parameters,
            instructions: Default::default(),
            block_graph: Default::default(),
            definitions,
        }
    }

    pub fn lookup(&self, name: &str) -> Value {
        self.definitions
            .get(name)
            .map(|def| def.to_ref())
            .expect(&format!("No such variable or param {name}"))
    }

    fn type_id_of_value(&self, value: &Value) -> TypeId {
        match value {
            Value::Void => self.type_context.void_type(),
            Value::Literal(..) => self.type_context.lookup_name("i32"),
            Value::VariableRef(idx) => match &self.instructions[*idx] {
                Instruction::Declaration { type_, .. } => *type_,
                Instruction::Call { ref operands, .. } => self.type_id_of_value(&operands[0]),
                Instruction::Choice {
                    left_value: ref left,
                    ..
                } => self.type_id_of_value(left),
                Instruction::Extend { to_type, .. } => *to_type,
                Instruction::Truncate { to_type, .. } => *to_type,
                inst => panic!("{inst:#?} at {idx}"),
            },
            Value::ParamRef(idx) => self.parameters[*idx].type_,
        }
    }

    fn type_id_of_op(&self, op: &Operation) -> TypeId {
        use OperationKind::*;
        match op.kind {
            LessThan | LessThanEquals | Equals | GreaterThan | GreaterThanEquals => {
                self.type_context.lookup_name("bool")
            }
            _ => self.type_id_of_value(&op.operands[0]),
        }
    }

    fn type_id_of(&self, v: &ValueOrOperation) -> TypeId {
        match v {
            ValueOrOperation::Value(v) => self.type_id_of_value(v),
            ValueOrOperation::Operation(op) => self.type_id_of_op(op),
        }
    }

    fn type_of_op(&self, op: &Operation) -> &Type {
        let id = self.type_id_of_op(op);
        &self.type_context.types[id.0]
    }

    fn type_of_value(&self, v: &Value) -> &Type {
        let id = self.type_id_of_value(v);
        &self.type_context.types[id.0]
    }

    fn type_of(&self, v: &ValueOrOperation) -> &Type {
        match v {
            ValueOrOperation::Value(v) => self.type_of_value(v),
            ValueOrOperation::Operation(op) => self.type_of_op(op),
        }
    }

    pub fn promote(&mut self, value_or_operation: ValueOrOperation) -> Value {
        match value_or_operation {
            ValueOrOperation::Value(v) => v,
            ValueOrOperation::Operation(op) => self.promote_op(op),
        }
    }

    fn promote_op(&mut self, operation: Operation) -> Value {
        let type_ = self.type_id_of_op(&operation);
        let definition = self.add_instruction(Instruction::Declaration {
            name: String::new(),
            type_,
            value: Some(ValueOrOperation::Operation(operation)),
            promoted: true,
        });
        Value::VariableRef(definition)
    }

    pub fn new_block(&mut self, idx: usize) -> Block {
        if let Some(block) = self.block_graph.find_vertex(&idx) {
            return block;
        }
        self.block_graph.add_vertex(idx)
    }

    pub fn add_block_edge(&mut self, from: Block, to: Block) {
        self.block_graph.add_edge(from, to);
    }

    #[must_use]
    pub fn marker(&mut self) -> Marker {
        Marker::new(self)
    }

    pub fn resolve_branch_marker(
        &mut self,
        marker: Marker,
        condition: Value,
        left_block: Block,
        right_block: Block,
    ) {
        self.add_block_edge(marker.containing_block(), left_block);
        self.add_block_edge(marker.containing_block(), right_block);
        marker.replace(
            self,
            Instruction::Branch {
                condition,
                left: left_block,
                right: right_block,
            },
        );
    }

    pub fn resolve_jump_marker(&mut self, marker: Marker, target: Block) {
        let _next = self.new_block(marker.idx + 1);

        self.add_block_edge(marker.containing_block(), target);
        marker.replace(self, Instruction::Jump { target });
    }

    fn cast_if_necessary(&mut self, val: Value, ty_: TypeId) -> Value {
        let val_type_id = self.type_id_of_value(&val);
        if val_type_id == ty_ {
            return val;
        }

        let val_type = &self.type_context.types[val_type_id.0];
        let type_ = &self.type_context.types[ty_.0];
        match (val_type, type_) {
            (Type::Integer { size: actual }, Type::Integer { size: expected }) => {
                if actual > expected {
                    Value::VariableRef(self.add_instruction(Instruction::Truncate {
                        to_type: ty_,
                        value: val,
                    }))
                } else if actual < expected {
                    Value::VariableRef(self.add_instruction(Instruction::Extend {
                        to_type: ty_,
                        value: val,
                    }))
                } else {
                    val
                }
            }
            _ => val,
        }
    }

    pub fn add_instruction(&mut self, instruction: Instruction) -> usize {
        let index = self.instructions.len();
        if let Instruction::Declaration {
            ref name, type_, ..
        } = instruction
        {
            self.definitions.insert(
                name.clone(),
                Def {
                    kind: DefKind::Variable,
                    index,
                    type_,
                },
            );
        }
        self.instructions.push(instruction);
        index
    }

    pub fn add_instructions_from_block(&mut self, block: &bir::Scope) -> (Block, Block, Value) {
        let start_block = self.new_block(self.instructions.len());
        for item in block.items.iter() {
            self.add_instructions_from_item(item);
        }
        let value = block.expr.as_ref().map_or(Value::Void, |expr| {
            let inst = self.add_instructions_from_expr(expr);
            self.promote(inst)
        });
        let last_block = self.block_graph.last().unwrap();
        (start_block, last_block, value)
    }

    pub fn add_instructions_from_item(&mut self, item: &bir::Item) {
        match item {
            bir::Item::Let { name, typ_, expr } => {
                let type_ = self.type_context.lookup_or_register(typ_.as_ref().unwrap());
                let mut value = expr
                    .as_ref()
                    .map(|expr| self.add_instructions_from_expr(expr));
                if let Some(ValueOrOperation::Value(v)) = &mut value {
                    *v = self.cast_if_necessary(v.clone(), type_);
                }
                self.add_instruction(Instruction::Declaration {
                    name: name.clone(),
                    type_,
                    value,
                    promoted: false,
                });
            }
            bir::Item::FnDef(..) => unimplemented!(),
            bir::Item::Expr(expr) => {
                self.add_instructions_from_expr(expr);
            }
            bir::Item::Type { .. } => unimplemented!(),
        }
    }

    pub fn add_instructions_from_expr(&mut self, expr: &bir::Expr) -> ValueOrOperation {
        match expr {
            bir::Expr::Literal(literal) => {
                let literal = match literal {
                    bir::Literal::Number(number) => Value::Literal(Literal::Number(*number)),
                    bir::Literal::Str(s) => Value::Literal(Literal::Str(s.clone())),
                };
                ValueOrOperation::Value(literal)
            }
            bir::Expr::NameRef { name } => ValueOrOperation::Value(self.lookup(&name)),
            bir::Expr::Call { receiver, operands } => {
                let operands = operands
                    .iter()
                    .map(|op| {
                        let val = self.add_instructions_from_expr(op);
                        self.promote(val)
                    })
                    .collect();
                let call_inst = self.add_instruction(Instruction::Call {
                    function: receiver.name().unwrap().to_string(),
                    operands,
                });
                ValueOrOperation::Value(Value::VariableRef(call_inst))
            }
            bir::Expr::Op(op) => {
                let operation = self.add_instructions_from_op(op);
                if operation.kind == OperationKind::Assignment {
                    ValueOrOperation::Value(self.promote_op(operation))
                } else {
                    ValueOrOperation::Operation(operation)
                }
            }
            bir::Expr::Branch {
                condition,
                left,
                right,
            } => {
                let condition = {
                    let inst = self.add_instructions_from_expr(&condition);
                    self.promote(inst)
                };
                let branch_left_right = self.marker();

                let (left_start, left_end, left_value) = self.add_instructions_from_block(&left);
                let left_jump_to_end = self.marker();

                let (right_start, right_end, right_value) = self.add_instructions_from_block(&right);
                let right_jump_to_end = self.marker();

                let end = self.new_block(self.instructions.len());

                self.resolve_branch_marker(branch_left_right, condition, left_start, right_start);
                self.resolve_jump_marker(left_jump_to_end, end);
                self.resolve_jump_marker(right_jump_to_end, end);

                if let Value::Void = left_value {
                    ValueOrOperation::Value(Value::Void)
                } else {
                    let choice_inst = self.add_instruction(Instruction::Choice {
                        left_value,
                        left: left_end,
                        right_value,
                        right: right_end,
                    });
                    ValueOrOperation::Value(Value::VariableRef(choice_inst))
                }
            }
            bir::Expr::Index { receiver, index } => {
                let receiver = {
                    let expr = self.add_instructions_from_expr(&receiver);
                    self.promote(expr)
                };
                let index = {
                    let expr = self.add_instructions_from_expr(&index);
                    self.promote(expr)
                };
                ValueOrOperation::Operation(Operation {
                    kind: OperationKind::Index,
                    operands: vec![receiver, index],
                })
            }
            bir::Expr::Loop { kind, body } => {
                let jump_to_start = self.marker();
                let (start_block, last_block, block_value) =
                    self.add_instructions_from_block(&body);
                self.resolve_jump_marker(jump_to_start, start_block);
                match kind {
                    bir::LoopKind::Loop => {
                        self.add_instruction(Instruction::Jump {
                            target: start_block,
                        });

                        self.add_block_edge(last_block, start_block);

                        self.new_block(self.instructions.len());
                    }
                }
                ValueOrOperation::Value(block_value)
            }
            _ => todo!(),
        }
    }

    pub fn add_instructions_from_op(&mut self, op: &bir::Op) -> Operation {
        use bir::{Fixity, OpKind};

        match (op.fixity, op.kind) {
            (Fixity::Infix, kind) => {
                let kind = match kind {
                    OpKind::Plus => OperationKind::Add,
                    OpKind::Minus => OperationKind::Subtract,
                    OpKind::Multiply => OperationKind::Multiply,
                    OpKind::Divide => OperationKind::Divide,
                    OpKind::LessThan => OperationKind::LessThan,
                    OpKind::LessThanEquals => OperationKind::LessThanEquals,
                    OpKind::GreaterThan => OperationKind::GreaterThan,
                    OpKind::GreaterThanEquals => OperationKind::GreaterThanEquals,
                    OpKind::Equals => OperationKind::Equals,
                    OpKind::Assignment => OperationKind::Assignment,
                    _ => unimplemented!("op {kind:?}"),
                };
                let lhs = {
                    let inst = self.add_instructions_from_expr(&op.operands[0]);
                    self.promote(inst)
                };
                let rhs = {
                    let inst = self.add_instructions_from_expr(&op.operands[1]);
                    self.promote(inst)
                };
                let (lhs_type_id, rhs_type_id) =
                    (self.type_id_of_value(&lhs), self.type_id_of_value(&rhs));
                let (lhs_type, rhs_type) = (self.type_of_value(&lhs), self.type_of_value(&rhs));
                let (lhs, rhs) = if lhs_type.int_size() > rhs_type.int_size() {
                    (lhs, self.cast_if_necessary(rhs, lhs_type_id))
                } else if lhs_type.int_size() < rhs_type.int_size() {
                    (self.cast_if_necessary(lhs, rhs_type_id), rhs)
                } else {
                    (lhs, rhs)
                };
                Operation {
                    kind,
                    operands: vec![lhs, rhs],
                }
            }
            _ => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_instructions() {
        let module = bir::parse_module_from_str("fn foo() -> i32 {}").unwrap();
        let lir = translate_module(&module);
        assert_eq!(lir.functions.len(), 1);
        assert_eq!(lir.functions[0].instructions.len(), 1);
    }

    #[test]
    fn just_return() {
        let module = bir::parse_module_from_str("fn foo() -> i32 { 10 }").unwrap();
        let lir = translate_module(&module);
        assert_eq!(lir.functions.len(), 1);
        assert_eq!(lir.functions[0].instructions.len(), 1);
    }

    #[test]
    fn one_item_with_return() {
        let module = bir::parse_module_from_str("fn foo() -> i32 { let i: i32 = 10; i }").unwrap();
        let lir = translate_module(&module);
        assert_eq!(lir.functions.len(), 1);
        assert_eq!(lir.functions[0].instructions.len(), 2);
    }

    #[test]
    fn one_item_with_no_return() {
        let module = bir::parse_module_from_str("fn foo() { let i: i32 = 10; }").unwrap();
        let lir = translate_module(&module);
        assert_eq!(lir.functions.len(), 1);
        assert_eq!(lir.functions[0].instructions.len(), 2);
    }
}
