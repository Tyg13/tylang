use std::collections::HashMap;

use inkwell::{types::BasicType, values::BasicValue};

mod llvm {
    pub use inkwell::basic_block::BasicBlock;
    pub use inkwell::builder::Builder;
    pub use inkwell::context::Context;
    pub use inkwell::module::{Linkage, Module};
    pub use inkwell::targets::{
        CodeModel, FileType, RelocMode, Target, TargetMachine, TargetTriple,
    };
    pub use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, FunctionType};
    pub use inkwell::values::{
        BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue,
    };
    pub use inkwell::{AddressSpace, OptimizationLevel};
}

type Variables<'ctx> = HashMap<usize, Variable<'ctx>>;
type Parameters = Vec<Parameter>;

pub enum Action {
    WriteIr,
    WriteObject,
    WriteAssembly,
    WriteExecutable,
}

#[derive(Clone, Debug)]
struct Parameter {
    index: usize,
    type_: lir::TypeId,
}

#[derive(Clone, PartialEq, Debug)]
enum Variable<'ctx> {
    Address(llvm::PointerValue<'ctx>),
    Value(llvm::BasicValueEnum<'ctx>),
}

#[derive(Clone, Debug)]
struct Scope<'ctx> {
    variables: Variables<'ctx>,
    parameters: Parameters,
}

impl Scope<'_> {
    fn new() -> Self {
        Self {
            variables: Default::default(),
            parameters: Parameters::new(),
        }
    }
}

pub enum BlockInfo<'ctx> {
    Reachable(llvm::BasicBlock<'ctx>),
    Unreachable,
}

struct Compiler<'ctx> {
    lir: &'ctx lir::Module,
    context: &'ctx llvm::Context,
    types: HashMap<lir::TypeId, &'ctx lir::Type>,
    block_info: HashMap<usize, BlockInfo<'ctx>>,
    source_file: String,
    module: llvm::Module<'ctx>,
    builder: llvm::Builder<'ctx>,
    scope_stack: Vec<Scope<'ctx>>,
    target_machine: llvm::TargetMachine,
}

impl<'ctx> Compiler<'ctx> {
    fn new(
        lir: &'ctx lir::Module,
        source_file: &str,
        context: &'ctx llvm::Context,
        module: llvm::Module<'ctx>,
        builder: llvm::Builder<'ctx>,
        optimize: bool,
    ) -> Self {
        use llvm::*;

        let opt_level = if optimize {
            OptimizationLevel::Aggressive
        } else {
            OptimizationLevel::None
        };

        let target = Target::from_name("x86-64").unwrap();
        let target_machine = target
            .create_target_machine(
                &TargetTriple::create("x86_64-pc-linux-gnu"),
                "x86-64",
                "+avx2",
                opt_level,
                RelocMode::Default,
                CodeModel::Default,
            )
            .unwrap();
        let types = lir
            .types
            .iter()
            .enumerate()
            .map(|(idx, ty)| (lir::TypeId(idx), ty))
            .collect();
        Self {
            lir,
            source_file: source_file.to_string(),
            context,
            types,
            block_info: Default::default(),
            module,
            builder,
            scope_stack: vec![],
            target_machine,
        }
    }

    fn scope(&mut self) -> &mut Scope<'ctx> {
        self.scope_stack.last_mut().expect("Scope stack empty!")
    }

    fn compile(&mut self) {
        visit_module(self, &self.lir);
    }

    fn write_ir(&self) {
        let ir_file = std::path::Path::new(&self.source_file).with_extension("ll");
        log::debug!("Writing LLVM IR to {}", ir_file.to_str().unwrap());
        self.module
            .print_to_file(&ir_file)
            .expect("Unable to write LLVM IR!");
        if let Err(err) = self.module.verify() {
            println!("LLVM Error: {}", err.to_str().unwrap());
        }
    }

    fn write_object_file(&self) {
        let object_file = std::path::Path::new(&self.source_file).with_extension("o");
        log::debug!("Writing object code to {}", object_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Object, &object_file)
            .expect("Error writing object file!");
    }

    fn write_executable(&self) {
        let _main_fn = self
            .module
            .get_function("main")
            .expect("No 'main' function!");

        let object_file = std::env::temp_dir()
            .join(&self.source_file)
            .with_extension("o");
        log::debug!("Writing object code to {}", object_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Object, &object_file)
            .expect("Error writing object file!");
        let output = std::process::Command::new("gcc")
            .args(["-lc", object_file.to_str().unwrap(), "-o", "a.out"])
            .output()
            .unwrap();
        fn print_if_nonempty(stream_name: &str, bytes: Vec<u8>) {
            if !bytes.is_empty() {
                println!("{stream_name}:\n{}", String::from_utf8(bytes).unwrap());
            }
        }
        print_if_nonempty("stdout", output.stdout);
        print_if_nonempty("stderr", output.stderr);
        if !output.status.success() {
            eprintln!("failed to compile {}", self.source_file);
        }
    }

    fn write_assembly_file(&self) {
        let asm_file = std::path::Path::new(&self.source_file).with_extension("s");
        log::debug!("Writing assembly code to {}", asm_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Assembly, &asm_file)
            .expect("Error writing assembly file!");
    }

    //fn var_address(&mut self, name: &str) -> PointerValue<'ctx> {
    //    if let Some(var) = self.var_in_scope(name) {
    //        var.address
    //    } else if let Some(param) = self.param_in_scope(name) {
    //        let value = self
    //            .module
    //            .get_function(&param.function)
    //            .expect("can't find param function")
    //            .get_nth_param(param.index as u32)
    //            .expect("can't find param")
    //            .into_int_value();
    //        let addr = self.builder.build_alloca(self.type_(&param.type_), "param");
    //        self.builder.build_store(addr, value);
    //        self.scope()
    //            .variables
    //            .insert(String::from(name), Variable::new(addr));
    //        addr
    //    } else {
    //        panic!("Undefined variable {}", name);
    //    }
    //}

    fn var_in_scope(&self, idx: usize) -> Option<Variable<'ctx>> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(var) = scope.variables.get(&idx) {
                return Some(var.clone());
            }
        }
        return None;
    }

    fn param_in_scope(&self, idx: usize) -> Option<Parameter> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(param) = scope.parameters.get(idx) {
                return Some(param.clone());
            }
        }
        return None;
    }

    fn translate_type(&self, type_: &lir::Type) -> llvm::BasicTypeEnum<'ctx> {
        match type_ {
            lir::Type::Basic { name } => match name.as_str() {
                "bool" => self.context.bool_type().as_basic_type_enum(),
                _ => unreachable!(),
            },
            lir::Type::Integer { size } => match size {
                8 => self.context.i8_type().as_basic_type_enum(),
                16 => self.context.i16_type().as_basic_type_enum(),
                32 => self.context.i32_type().as_basic_type_enum(),
                64 => self.context.i64_type().as_basic_type_enum(),
                _ => unreachable!(),
            },
            lir::Type::Pointer { target } => {
                let target_ty = self.types[target];
                self.translate_type(&target_ty)
                    .ptr_type(llvm::AddressSpace::Generic)
                    .as_basic_type_enum()
            }
            _ => unreachable!(),
        }
    }

    fn translate_type_id(&self, id: &lir::TypeId) -> llvm::BasicTypeEnum<'ctx> {
        self.translate_type(self.types[id])
    }

    fn fn_type(
        &self,
        return_type: &lir::Type,
        param_types: &[&lir::Type],
        is_var_args: bool,
    ) -> llvm::FunctionType<'ctx> {
        let param_types = param_types
            .iter()
            .map(|ty| llvm::BasicMetadataTypeEnum::from(self.translate_type(ty)))
            .collect::<Vec<_>>();
        if let lir::Type::Void = return_type {
            self.context.void_type().fn_type(&param_types, is_var_args)
        } else {
            self.translate_type(return_type)
                .fn_type(&param_types, is_var_args)
        }
    }

    fn default(&self, type_: &llvm::BasicTypeEnum<'ctx>) -> llvm::BasicValueEnum<'ctx> {
        use llvm::{BasicTypeEnum::*, BasicValueEnum};
        match type_ {
            IntType(type_) => BasicValueEnum::from(type_.const_zero()),
            PointerType(type_) => BasicValueEnum::from(type_.const_null()),
            _ => unreachable!(),
        }
    }

    fn current_function(&self) -> llvm::FunctionValue<'ctx> {
        self.builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap()
    }

    fn translate_lir_block(
        &self,
        fn_: &lir::Function,
        block: &lir::Block,
    ) -> llvm::BasicBlock<'ctx> {
        let idx = block.data(&fn_.blocks);
        match self.block_info[idx] {
            BlockInfo::Reachable(bb) => bb,
            BlockInfo::Unreachable => unreachable!(),
        }
    }
}

fn visit_module(c: &mut Compiler<'_>, module: &lir::Module) {
    let mut fn_values = Vec::new();
    for function in module.functions.iter() {
        fn_values.push((function, visit_function_decl(c, function)));
    }

    for type_ in module.types.iter() {
        if let lir::Type::Struct { name, members } = type_ {
            todo!();
        }
    }

    for (function, fn_value) in fn_values {
        c.scope_stack.push(Scope::new());
        for (index, param) in function.parameters.iter().enumerate() {
            c.scope().parameters.push(Parameter {
                index,
                type_: param.type_,
            });
        }
        if function.instructions.len() > 0 {
            populate_basic_blocks(c, fn_value, &function);

            let mut in_unreachable_block = false;
            for (idx, instruction) in function.instructions.iter().enumerate() {
                if let Some(info) = c.block_info.get(&idx) {
                    if let BlockInfo::Reachable(block) = info {
                        c.builder.position_at_end(*block);
                        in_unreachable_block = false;
                    } else {
                        in_unreachable_block = true;
                    }
                }
                if in_unreachable_block {
                    continue;
                }
                visit_instruction(c, idx, instruction, &function);
            }
        }
        c.scope_stack.pop();
    }
}

fn visit_function_decl<'ctx>(
    c: &mut Compiler<'ctx>,
    function: &lir::Function,
) -> llvm::FunctionValue<'ctx> {
    let mut param_types = Vec::new();
    for param in function.parameters.iter() {
        param_types.push(c.types[&param.type_]);
    }
    let fn_return_type = c.types[&function.return_type];
    let fn_type = c.fn_type(fn_return_type, &param_types, function.is_var_args);
    let fn_ = c
        .module
        .add_function(&function.name, fn_type, Some(llvm::Linkage::External));
    fn_
}

fn populate_basic_blocks<'ctx>(
    c: &mut Compiler<'ctx>,
    fn_: llvm::FunctionValue<'ctx>,
    function: &lir::Function,
) {
    c.block_info.clear();

    for block in function.blocks.vertices() {
        let idx = *block.data(&function.blocks);

        let is_start_block = if idx == 0 { true } else { false };
        let has_predecessors = block.in_degree(&function.blocks) > 0;

        if is_start_block || has_predecessors {
            let basic_block = c
                .context
                .append_basic_block(fn_, &format!("{}.{idx}", function.name));
            c.block_info.insert(idx, BlockInfo::Reachable(basic_block));
            if is_start_block && has_predecessors {
                let entry = c.context.prepend_basic_block(basic_block, "entry");
                c.builder.position_at_end(entry);
                c.builder.build_unconditional_branch(basic_block);
            }
        } else {
            c.block_info.insert(idx, BlockInfo::Unreachable);
        }
    }
}

fn visit_instruction<'ctx>(
    c: &mut Compiler<'ctx>,
    index: usize,
    instruction: &lir::Instruction,
    function: &lir::Function,
) {
    match instruction {
        lir::Instruction::Declaration {
            ref name,
            type_,
            value,
            promoted,
        } => {
            if *promoted {
                // This was an operation promoted to a value -- don't allocate
                // anything as this is only an intermediate computation.
                let val = visit_value_or_operation(c, value.as_ref().unwrap());
                c.scope().variables.insert(index, Variable::Value(val));
            } else {
                // Otherwise, this is a proper declaration, build an alloca,
                // visit the value or operation, and then store the result.
                let var_type = c.translate_type_id(type_);
                let addr = c.builder.build_alloca(var_type, &name);
                let initial_value = if let Some(val) = value {
                    visit_value_or_operation(c, val)
                } else {
                    c.default(&var_type)
                };
                c.builder.build_store(addr, initial_value);
                c.scope().variables.insert(index, Variable::Address(addr));
            }
        }
        lir::Instruction::Return { value } => {
            if let lir::Value::Void = value {
                c.builder.build_return(None);
            } else {
                let return_val = visit_value(c, value);
                c.builder.build_return(Some(&return_val));
            }
        }
        lir::Instruction::Call { function, operands } => {
            let fn_ = c
                .module
                .get_function(&function)
                .expect(&format!("no such function '{function}'"));
            let operands = operands
                .iter()
                .map(|operand| llvm::BasicMetadataValueEnum::from(visit_value(c, operand)))
                .collect::<Vec<_>>();
            let result = c.builder.build_call(fn_, &operands, "call");
            if let Some(val) = result.try_as_basic_value().left() {
                c.scope().variables.insert(index, Variable::Value(val));
            }
        }
        lir::Instruction::Jump { target } => {
            c.builder
                .build_unconditional_branch(c.translate_lir_block(function, target));
        }
        lir::Instruction::Branch {
            condition,
            left,
            right,
        } => {
            let condition = visit_value(c, condition).into_int_value();
            let condition =
                c.builder
                    .build_int_truncate(condition, c.context.bool_type(), "condition");
            let left_block = c.translate_lir_block(function, left);
            let right_block = c.translate_lir_block(function, right);
            c.builder
                .build_conditional_branch(condition, left_block, right_block);
            c.builder.position_at_end(left_block);
        }
        lir::Instruction::Choice {
            left_value,
            left,
            right_value,
            right,
        } => {
            let left_block = c.translate_lir_block(function, left);
            let right_block = c.translate_lir_block(function, right);

            let left_value = visit_value(c, left_value);
            let right_value = visit_value(c, right_value);

            let choice = c.builder.build_phi(c.context.i32_type(), "phi");
            choice.add_incoming(&[(&left_value, left_block), (&right_value, right_block)]);

            c.scope()
                .variables
                .insert(index, Variable::Value(choice.as_basic_value()));
        }
        lir::Instruction::Truncate { to_type, value } => {
            let value = visit_value(c, value).into_int_value();
            let trunc = c.builder.build_int_truncate(
                value,
                c.translate_type_id(to_type).into_int_type(),
                "trunc",
            );
            c.scope()
                .variables
                .insert(index, Variable::Value(llvm::BasicValueEnum::from(trunc)));
        }
        lir::Instruction::Extend { to_type, value } => {
            let value = visit_value(c, value).into_int_value();
            let trunc = c.builder.build_int_s_extend(
                value,
                c.translate_type_id(to_type).into_int_type(),
                "sext",
            );
            c.scope()
                .variables
                .insert(index, Variable::Value(llvm::BasicValueEnum::from(trunc)));
        }
        inst => eprintln!("not yet implemented: {inst:#?}"),
    }
}

fn visit_value_or_operation<'ctx>(
    c: &mut Compiler<'ctx>,
    value: &lir::ValueOrOperation,
) -> llvm::BasicValueEnum<'ctx> {
    use inkwell::IntPredicate::*;

    match value {
        lir::ValueOrOperation::Value(value) => visit_value(c, value),
        lir::ValueOrOperation::Operation(ref op) => {
            if op.kind == lir::OperationKind::Assignment {
                let lhs = match &op.operands[0] {
                    lir::Value::VariableRef(idx) => {
                        match c.scope().variables[idx] {
                            Variable::Address(addr) => addr,
                            Variable::Value(val) => c.builder.build_alloca(val.get_type(), "tmp"),
                        }
                    }
                    _ => unimplemented!("what's goin on man"),
                };
                let rhs = visit_value(c, &op.operands[1]);
                c.builder.build_store(lhs, rhs);
                return c.context.i32_type().const_zero().as_basic_value_enum();
            }
            let lhs = visit_value(c, &op.operands[0]);
            let rhs = visit_value(c, &op.operands[1]);
            match op.kind {
                lir::OperationKind::Add => llvm::BasicValueEnum::from(c.builder.build_int_add(
                    lhs.into_int_value(),
                    rhs.into_int_value(),
                    "sum",
                )),
                lir::OperationKind::Subtract => llvm::BasicValueEnum::from(
                    c.builder
                        .build_int_sub(lhs.into_int_value(), rhs.into_int_value(), "diff"),
                ),
                lir::OperationKind::Multiply => llvm::BasicValueEnum::from(
                    c.builder
                        .build_int_mul(lhs.into_int_value(), rhs.into_int_value(), "prod"),
                ),
                lir::OperationKind::Divide => {
                    llvm::BasicValueEnum::from(c.builder.build_int_signed_div(
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "quot",
                    ))
                }
                lir::OperationKind::LessThan => {
                    llvm::BasicValueEnum::from(c.builder.build_int_compare(
                        SLT,
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "lt",
                    ))
                }
                lir::OperationKind::LessThanEquals => {
                    llvm::BasicValueEnum::from(c.builder.build_int_compare(
                        SLE,
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "lte",
                    ))
                }
                lir::OperationKind::GreaterThan => {
                    llvm::BasicValueEnum::from(c.builder.build_int_compare(
                        SGT,
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "gt",
                    ))
                }
                lir::OperationKind::GreaterThanEquals => {
                    llvm::BasicValueEnum::from(c.builder.build_int_compare(
                        SGE,
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "gte",
                    ))
                }
                lir::OperationKind::Equals => {
                    llvm::BasicValueEnum::from(c.builder.build_int_compare(
                        EQ,
                        lhs.into_int_value(),
                        rhs.into_int_value(),
                        "eq",
                    ))
                }
                lir::OperationKind::Index => unsafe {
                    let addr = c.builder.build_in_bounds_gep(
                        lhs.into_pointer_value(),
                        &[rhs.into_int_value()],
                        "idx_addr",
                    );
                    llvm::BasicValueEnum::from(c.builder.build_load(addr, "idx_val"))
                },
                _ => unreachable!(),
            }
        }
    }
}

fn visit_value<'ctx>(c: &mut Compiler<'ctx>, value: &lir::Value) -> llvm::BasicValueEnum<'ctx> {
    match value {
        lir::Value::Void => unreachable!(),
        lir::Value::Literal(literal) => match literal {
            lir::Literal::Number(n) => {
                llvm::BasicValueEnum::from(c.context.i32_type().const_int(*n as u64, false))
            }
            lir::Literal::Str(s) => llvm::BasicValueEnum::from(
                c.builder
                    .build_global_string_ptr(s, "str")
                    .as_pointer_value(),
            ),
        },
        lir::Value::VariableRef(idx) => {
            let var = c
                .var_in_scope(*idx)
                .expect(&format!("no var at {idx}: {:#?}", c.scope().variables));
            match var {
                Variable::Address(addr) => {
                    llvm::BasicValueEnum::from(c.builder.build_load(addr, "tmp"))
                }
                Variable::Value(val) => val,
            }
        }
        lir::Value::ParamRef(idx) => {
            let param = c.param_in_scope(*idx).unwrap();
            dbg!(idx, param.index);
            c.current_function()
                .get_nth_param(param.index as u32)
                .expect("can't find param")
        }
    }
}

pub fn compile(lir: &lir::Module, source_file: &str, action: Action, optimize: bool) {
    let context = &llvm::Context::create();
    let module = context.create_module(source_file);
    let builder = context.create_builder();
    let _execution_engine = module.create_execution_engine().unwrap();

    let mut compiler = Compiler::new(lir, source_file, context, module, builder, optimize);
    compiler.compile();
    match action {
        Action::WriteAssembly => compiler.write_assembly_file(),
        Action::WriteIr => compiler.write_ir(),
        Action::WriteObject => compiler.write_object_file(),
        Action::WriteExecutable => compiler.write_executable(),
    }
}
