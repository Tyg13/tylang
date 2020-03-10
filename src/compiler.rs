use crate::ast::{self, Visitor};
use crate::util::Source;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{CodeModel, FileType, RelocMode, Target};
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::{AddressSpace, OptimizationLevel};
use std::collections::HashMap;

type Variables<'ctx> = HashMap<String, Variable<'ctx>>;
type Parameters = HashMap<String, Parameter>;

#[derive(Clone, PartialEq, Debug)]
struct Parameter {
    function: String,
    index: usize,
    type_: ast::Type,
}

#[derive(Clone, PartialEq, Debug)]
struct Variable<'ctx> {
    address: PointerValue<'ctx>,
}

impl<'ctx> Variable<'ctx> {
    fn new(address: PointerValue<'ctx>) -> Self {
        Self { address }
    }
}

struct Scope<'ctx> {
    variables: Variables<'ctx>,
    parameters: Parameters,
}

impl Scope<'_> {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parameters: Parameters::new(),
        }
    }
}

struct Compiler<'ctx> {
    tree: &'ctx ast::Tree,
    source: &'ctx Source,
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    scope_stack: Vec<Scope<'ctx>>,
}

impl<'ctx> Compiler<'ctx> {
    fn new(
        tree: &'ctx ast::Tree,
        source: &'ctx Source,
        context: &'ctx Context,
        module: Module<'ctx>,
        builder: Builder<'ctx>,
    ) -> Self {
        Self {
            tree,
            source,
            context,
            module,
            builder,
            scope_stack: vec![],
        }
    }

    fn scope(&mut self) -> &mut Scope<'ctx> {
        self.scope_stack.last_mut().expect("Scope stack empty!")
    }

    fn compile(&mut self) {
        self.visit_tree(&self.tree);

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
    }

    fn expr_value(&self, expr: &ast::Expression) -> BasicValueEnum<'ctx> {
        use ast::ExpressionKind::*;
        use inkwell::IntPredicate::*;
        match &expr.kind {
            BinaryOp { kind, lhs, rhs } => {
                let lhs = self.expr_value(lhs).into_int_value();
                let rhs = self.expr_value(rhs).into_int_value();
                use ast::BinaryOpKind::*;
                BasicValueEnum::from(match kind {
                    Add => self.builder.build_int_add(lhs, rhs, "sum"),
                    Sub => self.builder.build_int_sub(lhs, rhs, "diff"),
                    Mul => self.builder.build_int_mul(lhs, rhs, "prod"),
                    Div => self.builder.build_int_signed_div(lhs, rhs, "quot"),
                    Lt => self.builder.build_int_compare(SLT, lhs, rhs, "lt"),
                    Lte => self.builder.build_int_compare(SLE, lhs, rhs, "lte"),
                    Gt => self.builder.build_int_compare(SGT, lhs, rhs, "gt"),
                    Gte => self.builder.build_int_compare(SGE, lhs, rhs, "gte"),
                    Eq => self.builder.build_int_compare(EQ, lhs, rhs, "eq"),
                })
            }
            Call { name, arguments } => {
                let values: Vec<_> = arguments.iter().map(|arg| self.expr_value(arg)).collect();
                let func = self.module.get_function(&name).expect("No such function");
                self.builder
                    .build_call(func, &values, "call")
                    .try_as_basic_value()
                    .left()
                    .unwrap()
            }
            Constant(cons) => {
                BasicValueEnum::from(self.context.i64_type().const_int(cons.value as u64, false))
            }
            Variable(var) => self.var_value(&var.identifier),
            Group(inner) => self.expr_value(inner),
        }
    }

    fn var_value(&self, name: &str) -> BasicValueEnum<'ctx> {
        if let Some(var) = self.var_in_scope(name) {
            self.builder.build_load(var.address, "tmp")
        } else if let Some(param) = self.param_in_scope(name) {
            self.module
                .get_function(&param.function)
                .expect("can't find param function")
                .get_nth_param(param.index as u32)
                .expect("can't find param")
        } else {
            panic!("Undefined variable {}", name);
        }
    }

    fn var_address(&mut self, name: &str) -> PointerValue<'ctx> {
        if let Some(var) = self.var_in_scope(name) {
            var.address
        } else if let Some(param) = self.param_in_scope(name) {
            let value = self
                .module
                .get_function(&param.function)
                .expect("can't find param function")
                .get_nth_param(param.index as u32)
                .expect("can't find param")
                .into_int_value();
            let addr = self.builder.build_alloca(self.type_(&param.type_), "param");
            self.builder.build_store(addr, value);
            self.scope()
                .variables
                .insert(String::from(name), Variable::new(addr));
            addr
        } else {
            panic!("Undefined variable {}", name);
        }
    }

    fn var_in_scope(&self, name: &str) -> Option<Variable<'ctx>> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(var) = scope.variables.get(name) {
                return Some(var.clone());
            }
        }
        return None;
    }

    fn param_in_scope(&self, name: &str) -> Option<Parameter> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(param) = scope.parameters.get(name) {
                return Some(param.clone());
            }
        }
        return None;
    }

    fn type_(&self, type_: &ast::Type) -> BasicTypeEnum<'ctx> {
        use ast::BuiltinType::*;
        use ast::TypeKind::*;
        match &type_.kind {
            Builtin(kind) => BasicTypeEnum::from(match kind {
                I8 => self.context.i8_type(),
                I16 => self.context.i16_type(),
                I32 => self.context.i32_type(),
                I64 => self.context.i64_type(),
            }),
            Pointer(type_) => BasicTypeEnum::from({
                let pointed_to_type = self.type_(&type_);
                pointed_to_type.ptr_type(AddressSpace::Generic)
            }),
        }
    }

    fn default(&self, type_: &BasicTypeEnum<'ctx>) -> BasicValueEnum<'ctx> {
        use BasicTypeEnum::*;
        match type_ {
            IntType(type_) => BasicValueEnum::from(type_.const_zero()),
            PointerType(type_) => BasicValueEnum::from(type_.const_null()),
            _ => unreachable!(),
        }
    }

    fn current_function(&self) -> FunctionValue<'ctx> {
        self.module
            .get_last_function()
            .expect("No current function!")
    }
}

use ast::visit;
impl ast::Visitor for Compiler<'_> {
    fn visit_function(&mut self, function: &ast::Function) {
        let param_types: Vec<_> = function
            .parameters
            .iter()
            .map(|param| BasicTypeEnum::from(self.type_(&param.type_)))
            .collect();
        let fn_return_type = self.type_(&function.return_type);
        let fn_type = fn_return_type.fn_type(param_types.as_slice(), false);
        let _fn = self
            .module
            .add_function(&function.identifier, fn_type, Some(Linkage::External));

        if let Some(ref body) = function.body {
            let entry_block = self.context.append_basic_block(_fn, "entry");
            self.builder.position_at_end(entry_block);

            self.scope_stack.push(Scope::new());
            for (index, parameter) in function.parameters.iter().enumerate() {
                self.scope().parameters.insert(
                    parameter.variable.identifier.clone(),
                    Parameter {
                        function: function.identifier.clone(),
                        index,
                        type_: parameter.type_.clone(),
                    },
                );
            }
            self.visit_scope(body);
            self.scope_stack.pop();
            let last_block = self.builder.get_insert_block().unwrap();
            if last_block.get_terminator().is_none() {
                self.builder
                    .build_return(Some(&self.default(&fn_return_type)));
            }
        }
    }

    fn visit_scope(&mut self, scope: &ast::Scope) {
        self.scope_stack.push(Scope::new());
        visit::walk_scope(self, scope);
        self.scope_stack.pop();
    }

    fn visit_statement(&mut self, stmt: &ast::Statement) {
        use ast::StatementKind::*;
        match &stmt.kind {
            Declaration {
                var,
                type_,
                initializer,
            } => {
                let var_type = self.type_(type_);
                let addr = self.builder.build_alloca(var_type, &var.identifier);
                let initial_value = match initializer {
                    Some(initializer) => self.expr_value(initializer),
                    None => self.default(&var_type),
                };
                self.builder.build_store(addr, initial_value);
                self.scope()
                    .variables
                    .insert(var.identifier.clone(), Variable::new(addr));
            }
            Assignment { dst, src } => {
                use ast::ExpressionKind::*;
                let addr = match &dst.kind {
                    Variable(var) => self.var_address(&var.identifier),
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
                let cond_value = self.expr_value(condition).into_int_value();
                let cond =
                    self.builder
                        .build_int_truncate(cond_value, self.context.bool_type(), "cond");
                let then_block = self
                    .context
                    .append_basic_block(self.current_function(), "then");
                let next_block = self
                    .context
                    .append_basic_block(self.current_function(), "next");
                self.builder
                    .build_conditional_branch(cond, then_block, next_block);
                self.builder.position_at_end(then_block);
                self.visit_scope(block);
                if let None = self.builder.get_insert_block().unwrap().get_terminator() {
                    self.builder.build_unconditional_branch(next_block);
                }
                self.builder.position_at_end(next_block);
            }
            _ => {}
        }
    }
}

pub fn compile(tree: &ast::Tree, source: &Source) {
    let context = &Context::create();
    let module = context.create_module(&source.file());
    let builder = context.create_builder();
    let _execution_engine = module.create_execution_engine().unwrap();

    Compiler::new(tree, source, context, module, builder).compile();
}
