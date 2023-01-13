use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use inkwell::{types::BasicType, values::BasicValue};

mod types;
pub use types::*;

struct CG<'ctx> {
    lir: &'ctx lir::Module,
    context: &'ctx llvm::Context,
    block_info: HashMap<lir::ValueID, Block<'ctx>>,
    source_file: String,
    module: llvm::Module<'ctx>,
    builder: llvm::Builder<'ctx>,
    target_machine: llvm::TargetMachine,
    optimize: bool,

    values: HashMap<lir::ValueID, Value<'ctx>>,
}

impl<'ctx> CG<'ctx> {
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
        Self {
            lir,
            source_file: source_file.to_string(),
            context,
            block_info: Default::default(),
            module,
            builder,
            target_machine,
            optimize,
            values: Default::default(),
        }
    }

    fn compile(&mut self) {
        visit_module(self, &self.lir);
        if self.optimize {
            let pass_manager_builder = llvm::PassManagerBuilder::create();
            pass_manager_builder
                .set_optimization_level(llvm::OptimizationLevel::Aggressive);

            let mpm = llvm::PassManager::create(&());
            pass_manager_builder.set_inliner_with_threshold(100);
            pass_manager_builder.populate_module_pass_manager(&mpm);
            mpm.run_on(&self.module);
        }
    }

    fn write_ir(&self, output_path: Option<PathBuf>) {
        let ir_file = output_path.unwrap_or_else(|| {
            PathBuf::from(&self.source_file).with_extension("ll")
        });
        log::debug!("Writing LLVM IR to {}", ir_file.to_str().unwrap());
        self.module
            .print_to_file(&ir_file)
            .expect("Unable to write LLVM IR!");
        if let Err(err) = self.module.verify() {
            println!("LLVM Error: {}", err.to_str().unwrap());
        }
    }

    fn write_object_file(&self, output_path: Option<PathBuf>) {
        let object_file = output_path.unwrap_or_else(|| {
            PathBuf::from(&self.source_file).with_extension("o")
        });
        log::debug!("Writing object code to {}", object_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Object, &object_file)
            .expect("Error writing object file!");
    }

    fn write_executable(&self, output_path: Option<PathBuf>) {
        fn print_if_nonempty(stream_name: &str, bytes: Vec<u8>) {
            if !bytes.is_empty() {
                println!(
                    "{stream_name}:\n{}",
                    String::from_utf8(bytes).unwrap()
                );
            }
        }

        let _main_fn = self
            .module
            .get_function("main")
            .expect("No 'main' function!");

        let source_file = Path::new(&self.source_file);
        let object_file = std::env::temp_dir()
            .join(source_file.file_name().unwrap())
            .with_extension("o");
        let output_path = output_path
            .map(|path| path.as_os_str().to_str().unwrap().to_string())
            .unwrap_or_else(|| "a.out".to_string());
        log::debug!("Writing object code to {}", object_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Object, &object_file)
            .expect("Error writing object file!");

        let output = std::process::Command::new("gcc")
            .args([
                "-no-pie",
                "-lc",
                object_file.to_str().unwrap(),
                "-o",
                &output_path,
            ])
            .output()
            .unwrap();
        print_if_nonempty("stdout", output.stdout);
        print_if_nonempty("stderr", output.stderr);
        if !output.status.success() {
            eprintln!("failed to compile {}", self.source_file);
        }
    }

    fn write_assembly_file(&self, output_path: Option<PathBuf>) {
        let asm_file = output_path.unwrap_or_else(|| {
            PathBuf::from(&self.source_file).with_extension("s")
        });
        log::debug!("Writing assembly code to {}", asm_file.to_str().unwrap());
        self.target_machine
            .write_to_file(&self.module, llvm::FileType::Assembly, &asm_file)
            .expect("Error writing assembly file!");
    }

    fn translate_type(&self, ty: &lir::Ty) -> llvm::BasicTypeEnum<'ctx> {
        use lir::TyKind;
        match &ty.kind {
            TyKind::Integer { size } => match *size {
                1 => self.context.bool_type(),
                8 => self.context.i8_type(),
                16 => self.context.i16_type(),
                32 => self.context.i32_type(),
                64 => self.context.i64_type(),
                _ => unreachable!(),
            }
            .into(),
            TyKind::Pointer => {
                let target_ty = ty.as_ptr_ty().pointee(self.lir);
                self.translate_type(&target_ty)
                    .ptr_type(llvm::AddressSpace::Generic)
                    .into()
            }
            TyKind::Fn { .. } | TyKind::Void => unreachable!("{:?}", ty.kind),
            TyKind::Struct => {
                let struct_ty = ty.as_struct_ty(self.lir);
                let member_tys = struct_ty
                    .members
                    .iter()
                    .map(|id| self.translate_type(self.lir.types.get(id)))
                    .collect::<Vec<_>>();
                self.context.struct_type(&member_tys, false).into()
            }
        }
    }

    fn translate_fn_type(
        &self,
        return_type: &lir::Ty,
        param_types: &[&lir::Ty],
        is_var_args: bool,
    ) -> llvm::FunctionType<'ctx> {
        let param_types = param_types
            .iter()
            .map(|ty| {
                llvm::BasicMetadataTypeEnum::from(self.translate_type(ty))
            })
            .collect::<Vec<_>>();
        if return_type.is_void() {
            self.context.void_type().fn_type(&param_types, is_var_args)
        } else {
            self.translate_type(return_type)
                .fn_type(&param_types, is_var_args)
        }
    }

    fn current_function(&self) -> llvm::FunctionValue<'ctx> {
        self.builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap()
    }
}

fn visit_module<'ctx>(cg: &mut CG<'ctx>, module: &'ctx lir::Module) {
    let mut fn_values = Vec::new();
    for function in module.functions.iter() {
        fn_values.push((function, visit_function_decl(cg, function)));
    }

    for (function, fn_value) in fn_values {
        if function.num_blocks() > 0 {
            let blocks = populate_basic_blocks(cg, fn_value, &function);

            let ctx = lir::Context::full(module, function);

            cg.values.extend(function.params.iter().enumerate().map(
                |(idx, param)| {
                    let llvm_param =
                        fn_value.get_nth_param(idx as u32).unwrap();
                    (param.val, Value::Val(llvm_param))
                },
            ));
            for block in blocks {
                cg.builder.position_at_end(block.bb);
                for inst in block.insts(function) {
                    visit_inst(cg, ctx, inst);
                }
            }
            cg.values.clear();
        }
    }
}

fn visit_function_decl<'ctx>(
    c: &mut CG<'ctx>,
    function: &lir::Function,
) -> llvm::FunctionValue<'ctx> {
    let ctx = lir::Context::full(c.lir, function);
    let fn_ty = function.ty(ctx).as_fn_ty();
    let param_types: Vec<_> = fn_ty.params(ctx).collect();
    let fn_type = c.translate_fn_type(
        fn_ty.return_ty(ctx),
        &param_types,
        fn_ty.is_var_args,
    );
    let linkage = match function.internal {
        true => llvm::Linkage::Internal,
        false => llvm::Linkage::External,
    };
    let fn_ = c
        .module
        .add_function(&function.ident, fn_type, Some(linkage));
    for (idx, param) in fn_.get_param_iter().enumerate() {
        let name = &function.nth_param(idx).val.ident(ctx);
        param.set_name(name);
    }
    fn_
}

fn populate_basic_blocks<'ctx>(
    c: &mut CG<'ctx>,
    fn_: llvm::FunctionValue<'ctx>,
    function: &lir::Function,
) -> Vec<Block<'ctx>> {
    c.block_info.clear();

    let mut cg_blocks = Vec::new();
    function.visit_blocks_in_rpo(|block| {
        let id = block.val(function).id;

        let basic_block = c.context.append_basic_block(
            fn_,
            &format!("{}.{}", function.ident, cg_blocks.len()),
        );
        let block = Block {
            lir: block,
            bb: basic_block,
        };
        c.block_info.insert(id, block);
        cg_blocks.push(block);
    });

    cg_blocks
}

fn visit_inst<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    inst: &lir::Inst,
) {
    use lir::InstKind;
    let result = match &inst.kind {
        InstKind::Copy => {
            let val = visit_rvalue(c, ctx, &inst.rvals[0]);
            Some(Value::Val(val))
        }
        InstKind::Load => {
            let ptr = visit_lvalue(c, ctx, &inst.rvals[0]).into_pointer_value();
            Some(Value::Val(c.builder.build_load(ptr, "load")))
        }
        InstKind::Store => {
            Some(Value::Val(visit_rvalue(c, ctx, &inst.rvals[0])))
        }
        InstKind::Subscript => {
            let base =
                visit_rvalue(c, ctx, &inst.rvals[0]).into_pointer_value();
            let indices: Vec<_> = inst
                .rvals
                .iter()
                .skip(1)
                .map(|v| visit_rvalue(c, ctx, v).into_int_value())
                .collect();
            Some(Value::Addr(unsafe {
                c.builder.build_in_bounds_gep(base, &indices, "subscr")
            }))
        }
        InstKind::Add => {
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(
                c.builder
                    .build_int_nsw_add(lhs, rhs, "add")
                    .as_basic_value_enum(),
            ))
        }
        InstKind::Sub => {
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(
                c.builder
                    .build_int_nsw_sub(lhs, rhs, "sub")
                    .as_basic_value_enum(),
            ))
        }
        InstKind::Mul => {
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(
                c.builder
                    .build_int_nsw_mul(lhs, rhs, "mul")
                    .as_basic_value_enum(),
            ))
        }
        InstKind::Div => {
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            Some(Value::Val(
                c.builder
                    .build_int_signed_div(lhs, rhs, "div")
                    .as_basic_value_enum(),
            ))
        }
        InstKind::Return => {
            let ret_val = &inst.rvals[0];
            if ret_val.ty(ctx).is_void() {
                c.builder.build_return(None);
            } else {
                let val = visit_rvalue(c, ctx, &inst.rvals[0]);
                c.builder.build_return(Some(&val));
            }
            None
        }
        InstKind::Call => {
            let called_fn =
                visit_any_rvalue(c, ctx, &inst.rvals[0]).into_function_value();
            let ops: Vec<_> = inst
                .rvals
                .iter()
                .skip(1)
                .map(|val| to_basic_mdvalue(visit_any_rvalue(c, ctx, val)))
                .collect();
            let call = c.builder.build_call(called_fn, ops.as_slice(), "call");
            called_fn
                .get_type()
                .get_return_type()
                .map(|_| Value::Val(call.try_as_basic_value().left().unwrap()))
        }
        InstKind::Var => {
            let ty = c.translate_type(inst.lval().ty(ctx));
            let alloca = c.builder.build_alloca(ty, &inst.ident(ctx.as_fn()));
            Some(Value::Addr(alloca))
        }
        InstKind::Cmp { kind } => {
            use lir::CmpKind;
            let predicate = match kind {
                CmpKind::Eq => inkwell::IntPredicate::EQ,
                CmpKind::Ne => inkwell::IntPredicate::NE,
                CmpKind::Gt => inkwell::IntPredicate::SGT,
                CmpKind::Lt => inkwell::IntPredicate::SLT,
                CmpKind::Gte => inkwell::IntPredicate::SGE,
                CmpKind::Lte => inkwell::IntPredicate::SLE,
            };
            let lhs = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let rhs = visit_rvalue(c, ctx, &inst.rvals[1]).into_int_value();
            let cmp = c.builder.build_int_compare(predicate, lhs, rhs, "cmp");
            Some(Value::Val(cmp.as_basic_value_enum()))
        }
        InstKind::Cast => {
            let val = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let ty = c.translate_type(inst.val.ty(ctx)).into_int_type();
            let cast = if val.get_type().get_bit_width() > ty.get_bit_width() {
                c.builder.build_int_truncate(val, ty, "trunc")
            } else {
                c.builder.build_int_s_extend(val, ty, "sext")
            };
            Some(Value::Val(cast.as_basic_value_enum()))
        }
        InstKind::Jmp => {
            let dst = visit_block(c, &inst.rvals[0]);
            c.builder.build_unconditional_branch(dst);
            None
        }
        InstKind::Branch => {
            let cond = visit_rvalue(c, ctx, &inst.rvals[0]).into_int_value();
            let then = visit_block(c, &inst.rvals[1]);
            let alt = visit_block(c, &inst.rvals[2]);
            c.builder.build_conditional_branch(cond, then, alt);
            None
        }
        InstKind::GetField => {
            let base =
                visit_rvalue(c, ctx, &inst.rvals[0]).into_pointer_value();
            let index = visit_rvalue(c, ctx, &inst.rvals[1])
                .into_int_value()
                .get_zero_extended_constant()
                .unwrap();
            Some(Value::Val(
                c.builder
                    .build_struct_gep(base, index as u32, "index")
                    .unwrap()
                    .as_basic_value_enum(),
            ))
        }
        InstKind::Nop => None,
    };
    if let Some(lval) = inst.lval {
        let result = result.unwrap();
        match c.values.get(&lval.id) {
            Some(Value::Addr(addr)) => {
                c.builder.build_store(*addr, *result.as_val());
            }
            _ => {
                debug_assert!(!lval.ty(ctx).is_void());
                c.values.insert(lval.id, result);
            }
        }
    } else {
        if result.is_some() {
            c.module.print_to_stderr();
            debug_assert_eq!(result, None);
        }
    }
}

fn visit_any_lvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::AnyValueEnum<'ctx> {
    visit_any_value(c, ctx, value, ValueCategory::LVal)
}

fn visit_any_rvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::AnyValueEnum<'ctx> {
    visit_any_value(c, ctx, value, ValueCategory::RVal)
}

fn visit_any_value<'ctx>(
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
        lir::ValueKind::Inst => match c
            .values
            .get(&value.id)
            .copied()
            .expect(&format!("no key: {:?}", value.id))
        {
            Value::Val(v) => v.into(),
            Value::Addr(a) => match cat {
                ValueCategory::RVal => c.builder.build_load(a, "copy").into(),
                ValueCategory::LVal => a.into(),
            },
        },
        lir::ValueKind::Constant(kind) => {
            let ty = value.ty(ctx);
            match kind {
                lir::ConstantKind::Int => c
                    .translate_type(ty)
                    .into_int_type()
                    .const_int(value.int_constant(ctx) as u64, false)
                    .into(),
                lir::ConstantKind::Str => c
                    .builder
                    .build_global_string_ptr(value.str_constant(ctx), ".str")
                    .as_pointer_value()
                    .into(),
            }
        }
        lir::ValueKind::Function => {
            let ident = &ctx.as_mod().fn_(&value.id).ident;
            c.module.get_function(ident).unwrap().into()
        }
        lir::ValueKind::Undef => {
            let ty = c.translate_type(value.ty(ctx));
            get_undef(ty).into()
        }
        lir::ValueKind::Void => unreachable!("can't have a value of void!"),
        lir::ValueKind::Block => unreachable!("can't have a value of a block!"),
    }
}

fn get_undef(v: llvm::BasicTypeEnum) -> llvm::BasicValueEnum {
    match v {
        inkwell::types::BasicTypeEnum::ArrayType(a) => a.get_undef().into(),
        inkwell::types::BasicTypeEnum::FloatType(f) => f.get_undef().into(),
        inkwell::types::BasicTypeEnum::IntType(i) => i.get_undef().into(),
        inkwell::types::BasicTypeEnum::PointerType(p) => p.get_undef().into(),
        inkwell::types::BasicTypeEnum::StructType(s) => s.get_undef().into(),
        inkwell::types::BasicTypeEnum::VectorType(v) => v.get_undef().into(),
    }
}

fn to_basic_mdvalue(v: llvm::AnyValueEnum) -> llvm::BasicMetadataValueEnum {
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

fn to_basic_value(v: llvm::AnyValueEnum) -> llvm::BasicValueEnum {
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
    to_basic_value(visit_any_lvalue(c, ctx, value))
}

fn visit_rvalue<'ctx>(
    c: &mut CG<'ctx>,
    ctx: lir::Context<'ctx>,
    value: &lir::ValueRef,
) -> llvm::BasicValueEnum<'ctx> {
    to_basic_value(visit_any_rvalue(c, ctx, value))
}

fn visit_block<'ctx>(
    c: &mut CG<'ctx>,
    value: &lir::ValueRef,
) -> llvm::BasicBlock<'ctx> {
    c.block_info[&value.id].bb
}

pub fn compile(
    lir: &lir::Module,
    source_file: &str,
    output_path: Option<&str>,
    action: Action,
    optimize: bool,
) {
    let context = &llvm::Context::create();
    let module = context.create_module(source_file);
    let builder = context.create_builder();
    let _execution_engine = module.create_execution_engine().unwrap();

    let output_path = output_path.map(|path| PathBuf::from(path));

    let mut compiler =
        CG::new(lir, source_file, context, module, builder, optimize);
    compiler.compile();
    match action {
        Action::WriteAssembly => compiler.write_assembly_file(output_path),
        Action::WriteIr => compiler.write_ir(output_path),
        Action::WriteObject => compiler.write_object_file(output_path),
        Action::WriteExecutable => compiler.write_executable(output_path),
    }
}
