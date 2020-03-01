use crate::parser::{self, *};
use crate::util::Source;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use inkwell::module::Module;
use inkwell::targets::{CodeModel, FileType, RelocMode, Target};
use inkwell::types::IntType;
use inkwell::values::{FunctionValue, IntValue, PointerValue};
use inkwell::OptimizationLevel;

type Variables<'ctx> = std::collections::HashMap<String, PointerValue<'ctx>>;

struct Interpreter<'ctx> {
    tree: &'ctx Scope,
    source: &'ctx Source,
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
    variables: Variables<'ctx>,
}

impl<'ctx> Interpreter<'ctx> {
    fn new(
        tree: &'ctx Scope,
        source: &'ctx Source,
        context: &'ctx Context,
        module: Module<'ctx>,
        builder: Builder<'ctx>,
        execution_engine: ExecutionEngine<'ctx>,
    ) -> Self {
        Self {
            tree,
            source,
            context,
            module,
            builder,
            execution_engine,
            variables: Variables::new(),
        }
    }

    fn interpret(&mut self) {
        let entry_fn_type = self.int().fn_type(&[], false);
        let entry_fn = self.module.add_function("entry", entry_fn_type, None);
        let body = self.context.append_basic_block(entry_fn, "body");
        self.builder.position_at_end(body);
        self.visit_scope(&self.tree);

        let source_file = self.source.file();
        let source_path = std::path::Path::new(&source_file);

        let ir_path = source_path.with_extension("ir");
        self.module
            .print_to_file(&ir_path)
            .expect("Unable to write LLVM-IR!");

        if let Err(err) = self.module.verify() {
            println!("LLVM Error: {}", err.to_str().unwrap());
            return;
        }

        let target = Target::from_name("x86-64").unwrap();
        let target_machine = target
            .create_target_machine(
                "x86_64-pc-linux-gnu",
                "x86-64",
                "+avx2",
                OptimizationLevel::None,
                RelocMode::Default,
                CodeModel::Default,
            )
            .unwrap();
        let object_file = source_path.with_extension("o");
        target_machine
            .write_to_file(&self.module, FileType::Object, &object_file)
            .expect("Error writing object file!");

        let asm_file = source_path.with_extension("s");
        target_machine
            .write_to_file(&self.module, FileType::Assembly, &asm_file)
            .expect("Error writing assembly file!");

        let result = unsafe { self.execution_engine.run_function_as_main(entry_fn, &[]) };
        println!("{}", result);
    }

    fn expr_value(&self, expr: &Expression) -> IntValue<'ctx> {
        use ExpressionKind::*;
        match &expr.kind {
            BinaryOp { kind, lhs, rhs } => {
                let lhs = self.expr_value(lhs);
                let rhs = self.expr_value(rhs);
                use BinaryOpKind::*;
                match kind {
                    Add => self.builder.build_int_add(lhs, rhs, "sum"),
                    Sub => self.builder.build_int_sub(lhs, rhs, "diff"),
                    Mul => self.builder.build_int_mul(lhs, rhs, "prod"),
                    Div => self.builder.build_int_signed_div(lhs, rhs, "quot"),
                }
            }
            Constant(cons) => self.int().const_int(cons.value as u64, false),
            Variable(var) => {
                let addr = self.var_addr(&var.identifier);
                self.builder.build_load(addr, "tmp").into_int_value()
            }
            Group(inner) => self.expr_value(inner),
        }
    }

    fn var_addr(&self, name: &str) -> PointerValue<'ctx> {
        self.variables
            .get(name)
            .cloned()
            .expect("Undefined variable!")
    }

    fn int(&self) -> IntType<'ctx> {
        self.context.i64_type()
    }

    fn main(&self) -> FunctionValue<'ctx> {
        self.module
            .get_function("entry")
            .expect("No main function!")
    }
}

impl parser::Visitor for Interpreter<'_> {
    fn visit_statement(&mut self, stmt: &Statement) {
        use StatementKind::*;
        match &stmt.kind {
            Declaration { var, initializer } => {
                let addr = self.builder.build_alloca(self.int(), &var.identifier);
                self.variables.insert(var.identifier.clone(), addr);
                let initial_value = if let Some(initializer) = initializer {
                    self.expr_value(initializer)
                } else {
                    self.context.i64_type().const_zero()
                };
                self.builder.build_store(addr, initial_value);
            }
            Assignment { dst, src } => {
                use ExpressionKind::*;
                let addr = match &dst.kind {
                    Variable(var) => self.var_addr(&var.identifier),
                    _ => panic!("LHS of assignment is not a variable!"),
                };
                let value = self.expr_value(src);
                self.builder.build_store(addr, value);
            }
            Scope(scope) => self.visit_scope(scope),
            Return(expr) => {
                let val = self.expr_value(expr);
                self.builder.build_return(Some(&val));
            }
            If { condition, block } => {
                let cond_value = self.expr_value(condition);
                let cond =
                    self.builder
                        .build_int_truncate(cond_value, self.context.bool_type(), "cond");
                let then_block = self.context.append_basic_block(self.main(), "then");
                let next_block = self.context.append_basic_block(self.main(), "next");
                self.builder
                    .build_conditional_branch(cond, then_block, next_block);
                self.builder.position_at_end(then_block);
                self.visit_statement(block);
                if let None = self.builder.get_insert_block().unwrap().get_terminator() {
                    self.builder.build_unconditional_branch(next_block);
                }
                self.builder.position_at_end(next_block);
            }
            _ => {}
        }
    }
}

pub fn interpret(tree: &Scope, source: &Source) {
    let context = &Context::create();
    let module = context.create_module(&source.file());
    let execution_engine = module.create_execution_engine().unwrap();
    let builder = context.create_builder();

    Interpreter::new(tree, source, context, module, builder, execution_engine).interpret();
}
