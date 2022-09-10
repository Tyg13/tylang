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
        AnyValueEnum, BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue,
    };
    pub use inkwell::{AddressSpace, OptimizationLevel};
}

pub enum Action {
    WriteIr,
    WriteObject,
    WriteAssembly,
    WriteExecutable,
}

pub enum BlockInfo<'ctx> {
    Reachable(llvm::BasicBlock<'ctx>),
    Unreachable,
}

struct CG<'ctx> {
    lir: &'ctx lir::Module,
    sema: &'ctx sema::Map,
    context: &'ctx llvm::Context,
    block_info: HashMap<lir::ValueID, BlockInfo<'ctx>>,
    source_file: String,
    module: llvm::Module<'ctx>,
    builder: llvm::Builder<'ctx>,
    target_machine: llvm::TargetMachine,

    inst_values: HashMap<lir::ValueID, Value<'ctx>>,
}

#[derive(Debug, Copy, Clone)]
enum Value<'ctx> {
    Val(llvm::BasicValueEnum<'ctx>),
    Addr(llvm::PointerValue<'ctx>),
}

impl<'ctx> Value<'ctx> {
    pub fn as_addr(&self) -> &llvm::PointerValue<'ctx> {
        if let Self::Addr(addr) = self {
            addr
        } else {
            panic!("not an addr!")
        }
    }

    pub fn as_val(&self) -> &llvm::BasicValueEnum<'ctx> {
        if let Self::Val(val) = self {
            val
        } else {
            panic!("not a val!")
        }
    }
}

impl<'ctx> CG<'ctx> {
    fn new(
        lir: &'ctx lir::Module,
        sema: &'ctx sema::Map,
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
        Self {
            lir,
            sema,
            source_file: source_file.to_string(),
            context,
            block_info: Default::default(),
            module,
            builder,
            target_machine,
            inst_values: Default::default(),
        }
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
        fn print_if_nonempty(stream_name: &str, bytes: Vec<u8>) {
            if !bytes.is_empty() {
                println!("{stream_name}:\n{}", String::from_utf8(bytes).unwrap());
            }
        }

        let _main_fn = self
            .module
            .get_function("main")
            .expect("No 'main' function!");

        let path = std::path::PathBuf::from(&self.source_file);

        let object_file = std::env::temp_dir()
            .join(path.file_name().unwrap())
            .with_extension("o");
        log::debug!("Writing object code to {}", object_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Object, &object_file)
            .expect("Error writing object file!");
        let output = std::process::Command::new("gcc")
            .args(["-lc", object_file.to_str().unwrap(), "-o", "a.out"])
            .output()
            .unwrap();
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

    fn translate_type(&self, ty: &sema::Type) -> llvm::BasicTypeEnum<'ctx> {
        use sema::TypeKind;
        match &ty.kind {
            TypeKind::Integer { size } => match *size {
                1 => self.context.bool_type().as_basic_type_enum(),
                8 => self.context.i8_type().as_basic_type_enum(),
                16 => self.context.i16_type().as_basic_type_enum(),
                32 => self.context.i32_type().as_basic_type_enum(),
                64 => self.context.i64_type().as_basic_type_enum(),
                _ => unreachable!(),
            },
            TypeKind::Pointer { pointee } => {
                let target_ty = self.sema.ty(*pointee).unwrap();
                self.translate_type(&target_ty)
                    .ptr_type(llvm::AddressSpace::Generic)
                    .as_basic_type_enum()
            }
            _ => unreachable!("{:?}", ty.kind),
        }
    }

    fn translate_type_id(&self, id: sema::ID) -> llvm::BasicTypeEnum<'ctx> {
        self.translate_type(self.sema.ty(id).unwrap())
    }

    fn translate_fn_type(
        &self,
        return_type: &sema::Type,
        param_types: &[&sema::Type],
        is_var_args: bool,
    ) -> llvm::FunctionType<'ctx> {
        let param_types = param_types
            .iter()
            .map(|ty| llvm::BasicMetadataTypeEnum::from(self.translate_type(ty)))
            .collect::<Vec<_>>();
        if let sema::TypeKind::Void = return_type.kind {
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
        let idx = block.data(&fn_.blocks());
        match self.block_info[idx] {
            BlockInfo::Reachable(bb) => bb,
            BlockInfo::Unreachable => unreachable!(),
        }
    }
}

fn visit_module<'ctx>(cg: &mut CG<'ctx>, module: &'ctx lir::Module) {
    let mut fn_values = Vec::new();
    for function in module.functions.iter() {
        fn_values.push((function, visit_function_decl(cg, function)));
    }

    for (function, fn_value) in fn_values {
        if function.insts.len() > 0 {
            populate_basic_blocks(cg, fn_value, &function);

            let mut in_unreachable_block = false;
            for inst in function.insts.iter() {
                if let Some(info) = cg.block_info.get(&inst.val.id) {
                    if let BlockInfo::Reachable(block) = info {
                        cg.builder.position_at_end(*block);
                        in_unreachable_block = false;
                    } else {
                        in_unreachable_block = true;
                    }
                }
                if in_unreachable_block {
                    continue;
                }
                visit_inst(cg, lir::Context::full(module, function), inst);
            }
        }
    }
}

fn visit_function_decl<'ctx>(
    c: &mut CG<'ctx>,
    function: &lir::Function,
) -> llvm::FunctionValue<'ctx> {
    let mut param_types = Vec::new();
    for param in function.params.iter() {
        param_types.push(param.ty(function, c.sema));
    }
    let fn_return_type = function.return_ty(c.sema);
    let fn_type = c.translate_fn_type(fn_return_type, &param_types, false);
    let fn_ = c
        .module
        .add_function(&function.ident, fn_type, Some(llvm::Linkage::External));
    for (idx, param) in fn_.get_param_iter().enumerate() {
        let name = function.params[idx].ident(function);
        param.set_name(name);
    }
    fn_
}

fn populate_basic_blocks<'ctx>(
    c: &mut CG<'ctx>,
    fn_: llvm::FunctionValue<'ctx>,
    function: &lir::Function,
) {
    c.block_info.clear();

    let mut is_start_block = true;
    for block in function.blocks().vertices() {
        let id = *block.data(&function.blocks());

        let has_predecessors = block.in_degree(&function.blocks()) > 0;
        let reachable = is_start_block || has_predecessors;

        if is_start_block {
            is_start_block = false;
        }

        if !reachable {
            c.block_info.insert(id, BlockInfo::Unreachable);
            continue;
        }

        let basic_block = c
            .context
            .append_basic_block(fn_, &format!("{}.{}", function.ident, id.to_string()));
        c.block_info.insert(id, BlockInfo::Reachable(basic_block));
        if is_start_block && has_predecessors {
            let entry = c.context.prepend_basic_block(basic_block, "entry");
            c.builder.position_at_end(entry);
            c.builder.build_unconditional_branch(basic_block);
        }
    }
}

fn visit_inst<'ctx>(c: &mut CG<'ctx>, ctx: lir::Context<'ctx>, inst: &lir::Inst) {
    let result = match &inst.kind {
        lir::InstKind::Copy => {
            let val = visit_rvalue(c, ctx, &inst.rvals[0]);
            Some(Value::Val(val))
        }
        lir::InstKind::Offset => {
            let _ty = c.translate_type_id(inst.val.sema(ctx));
            let ptr = visit_rvalue(c, ctx, &inst.rvals[0]).into_pointer_value();
            let index = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(match index.get_zero_extended_constant() {
                // As an optimization, fold away zero offsets
                Some(0) => ptr.as_basic_value_enum(),
                _ => unsafe { c.builder.build_in_bounds_gep(ptr, &[index], "gep") }
                    .as_basic_value_enum(),
            }))
        }
        lir::InstKind::Load => {
            let ptr = visit_lvalue(c, ctx, &inst.rvals[0]).into_pointer_value();
            Some(Value::Val(c.builder.build_load(ptr, "load")))
        }
        lir::InstKind::Store => {
            Some(Value::Val(visit_rvalue(c, ctx, &inst.rvals[0])))
        }
        lir::InstKind::Add => {
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(
                c.builder
                    .build_int_nsw_add(lhs, rhs, "add")
                    .as_basic_value_enum(),
            ))
        }
        lir::InstKind::Return => {
            let val = visit_rvalue(c, ctx, &inst.rvals[0]);
            c.builder.build_return(Some(&val));
            None
        }
        lir::InstKind::Call => {
            let called_fn = visit_any_rvalue(c, ctx, &inst.rvals[0]).into_function_value();
            let ops: Vec<_> = inst
                .rvals
                .iter()
                .skip(1)
                .map(|val| to_basic_mdvalue_enum(visit_any_rvalue(c, ctx, val)))
                .collect();
            let call = c.builder.build_call(called_fn, ops.as_slice(), "call");
            if called_fn.get_type().get_return_type().is_none() {
                None
            } else {
                Some(Value::Val(call.try_as_basic_value().left().unwrap()))
            }
        }
        lir::InstKind::Var => {
            let ty = c.translate_type_id(inst.val.sema(ctx));
            let alloca = c.builder.build_alloca(ty, inst.ident(ctx.as_fn()));
            Some(Value::Addr(alloca))
        }
        lir::InstKind::Cmp { kind } => {
            let predicate = match kind {
                lir::CmpKind::Eq => inkwell::IntPredicate::EQ,
            };
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            let cmp = c.builder.build_int_compare(predicate, lhs, rhs, "cmp");
            Some(Value::Val(cmp.as_basic_value_enum()))
        }
        lir::InstKind::Cast => {
            let val = visit_rvalue(c, ctx, &inst.rvals[0]);
            let ty = c.translate_type_id(inst.val.sema(ctx));
            let cast = c.builder.build_bitcast(val, ty, "cast");
            Some(Value::Val(cast))
        }
        lir::InstKind::Jmp => todo!(),
        //lir::InstKind::Jump { target } => {
        //    c.builder
        //        .build_unconditional_branch(c.translate_lir_block(function, target));
        //}
        //lir::InstKind::Branch {
        //    condition,
        //    left,
        //    right,
        //} => {
        //    let condition = visit_value(c, condition).into_int_value();
        //    let condition =
        //        c.builder
        //            .build_int_truncate(condition, c.context.bool_type(), "condition");
        //    let left_block = c.translate_lir_block(function, left);
        //    let right_block = c.translate_lir_block(function, right);
        //    c.builder
        //        .build_conditional_branch(condition, left_block, right_block);
        //    c.builder.position_at_end(left_block);
        //}
        //lir::InstKind::Truncate { value } => {
        //    let value = visit_value(c, value).into_int_value();
        //    let trunc = c.builder.build_int_truncate(
        //        value,
        //        c.translate_type_id(inst.ty).into_int_type(),
        //        "trunc",
        //    );
        //    c.values
        //        .insert(idx, Value::value(inst.ty, trunc.as_basic_value_enum()));
        //}
        //lir::InstKind::Extend { value } => {
        //    let value = visit_value(c, value).into_int_value();
        //    let ext = c.builder.build_int_s_extend(
        //        value,
        //        c.translate_type_id(inst.ty).into_int_type(),
        //        "sext",
        //    );
        //    c.values
        //        .insert(idx, Value::value(inst.ty, ext.as_basic_value_enum()));
        //}
        //lir::InstKind::Nop => {}
    };
    if inst.kind == lir::InstKind::Var {
        c.inst_values.insert(inst.val.id, result.unwrap());
    }
    else if let Some(lval) = inst.lval {
        dbg!(lval.id, inst.kind);
        if lval.is_var(ctx) {
            let addr = *c.inst_values[&lval.id].as_addr();
            c.builder.build_store(addr, *result.unwrap().as_val());
        } else {
            c.inst_values.insert(lval.id, result.unwrap());
        }
    }
    else if let Some(val) = result {
        eprintln!("inserting for val id: {:?}", inst.val.id);
        c.inst_values.insert(inst.val.id, val);
    }
}

// TODO? move into lir
#[derive(Debug)]
enum ValueCategory {
    LVal,
    RVal,
}

fn visit_any_lvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::AnyValueEnum<'ctx> {
    visit_value(c, ctx, value, ValueCategory::LVal)
}

fn visit_any_rvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::AnyValueEnum<'ctx> {
    visit_value(c, ctx, value, ValueCategory::RVal)
}

fn visit_value<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
    cat: ValueCategory,
) -> llvm::AnyValueEnum<'ctx> {
    match value.kind(ctx) {
        lir::ValueKind::Param => {
            let idx = ctx.as_fn().param_num(value).unwrap();
            c.current_function()
                .get_nth_param(idx as u32)
                .unwrap()
                .into()
        }
        lir::ValueKind::Inst => match c.inst_values[dbg!(&value.id)] {
            Value::Val(v) => v.into(),
            Value::Addr(a) => match cat {
                ValueCategory::RVal => c.builder.build_load(a, "copy").into(),
                ValueCategory::LVal => a.into(),
            },
        },
        lir::ValueKind::Constant => {
            let sema = value.sema(ctx);
            match c.sema.constant(sema).unwrap() {
                sema::Constant::Int(n) => c
                    .translate_type_id(sema)
                    .into_int_type()
                    .const_int(*n as u64, false)
                    .into(),
                sema::Constant::Str(s) => c
                    .builder
                    .build_global_string_ptr(s, "string")
                    .as_pointer_value()
                    .into(),
            }
        }
        lir::ValueKind::Function => {
            let sema = value.sema(ctx);
            let ident = &c.sema.name(sema).unwrap().ident;
            c.module.get_function(ident).unwrap().into()
        }
        lir::ValueKind::Void => {
            unreachable!("can't have a value of void!")
        }
    }
}

fn to_basic_mdvalue_enum(v: llvm::AnyValueEnum) -> llvm::BasicMetadataValueEnum {
    match v {
        llvm::AnyValueEnum::ArrayValue(v) => v.into(),
        llvm::AnyValueEnum::IntValue(v) => v.into(),
        llvm::AnyValueEnum::FloatValue(v) => v.into(),
        llvm::AnyValueEnum::PointerValue(v) => v.into(),
        llvm::AnyValueEnum::StructValue(v) => v.into(),
        llvm::AnyValueEnum::VectorValue(v) => v.into(),
        llvm::AnyValueEnum::MetadataValue(v) => v.into(),
        llvm::AnyValueEnum::PhiValue(_)
        | llvm::AnyValueEnum::FunctionValue(_)
        | llvm::AnyValueEnum::InstructionValue(_) => unreachable!(),
    }
}

fn to_basic_value_enum(v: llvm::AnyValueEnum) -> llvm::BasicValueEnum {
    match v {
        llvm::AnyValueEnum::ArrayValue(v) => v.into(),
        llvm::AnyValueEnum::IntValue(v) => v.into(),
        llvm::AnyValueEnum::FloatValue(v) => v.into(),
        llvm::AnyValueEnum::PointerValue(v) => v.into(),
        llvm::AnyValueEnum::StructValue(v) => v.into(),
        llvm::AnyValueEnum::VectorValue(v) => v.into(),
        llvm::AnyValueEnum::MetadataValue(_)
        | llvm::AnyValueEnum::PhiValue(_)
        | llvm::AnyValueEnum::FunctionValue(_)
        | llvm::AnyValueEnum::InstructionValue(_) => unreachable!(),
    }
}

fn visit_lvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::BasicValueEnum<'ctx> {
    to_basic_value_enum(visit_any_lvalue(c, ctx, value))
}

fn visit_rvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::BasicValueEnum<'ctx> {
    to_basic_value_enum(visit_any_rvalue(c, ctx, value))
}

pub fn compile(
    lir: &lir::Module,
    sema: &sema::Map,
    source_file: &str,
    action: Action,
    optimize: bool,
) {
    let context = &llvm::Context::create();
    let module = context.create_module(source_file);
    let builder = context.create_builder();
    let _execution_engine = module.create_execution_engine().unwrap();

    let mut compiler = CG::new(lir, sema, source_file, context, module, builder, optimize);
    compiler.compile();
    match action {
        Action::WriteAssembly => compiler.write_assembly_file(),
        Action::WriteIr => compiler.write_ir(),
        Action::WriteObject => compiler.write_object_file(),
        Action::WriteExecutable => compiler.write_executable(),
    }
}
