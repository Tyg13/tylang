pub(crate) mod llvm {
    pub use inkwell::basic_block::BasicBlock;
    pub use inkwell::builder::Builder;
    pub use inkwell::context::Context;
    pub use inkwell::module::{Linkage, Module};
    pub use inkwell::passes::{PassManager, PassManagerBuilder};
    pub use inkwell::targets::{
        CodeModel, FileType, RelocMode, Target, TargetMachine, TargetTriple,
    };
    pub use inkwell::types::{
        BasicMetadataTypeEnum, BasicTypeEnum, FunctionType,
    };
    pub use inkwell::values::{
        AnyValueEnum, BasicMetadataValueEnum, BasicValueEnum, FunctionValue,
        PointerValue,
    };
    pub use inkwell::{AddressSpace, OptimizationLevel};
}

pub enum Action {
    WriteIr,
    WriteObject,
    WriteAssembly,
    WriteExecutable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum Value<'ctx> {
    Val(llvm::BasicValueEnum<'ctx>),
    Addr(llvm::PointerValue<'ctx>, llvm::BasicTypeEnum<'ctx>),
}

impl<'ctx> Value<'ctx> {
    pub fn as_addr(&self) -> &llvm::PointerValue<'ctx> {
        if let Self::Addr(addr, _) = self {
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

// TODO? move into lir
#[derive(Debug)]
pub(crate) enum ValueCategory {
    LVal,
    RVal,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Block<'ctx> {
    pub lir: lir::Block,
    pub bb: llvm::BasicBlock<'ctx>,
}

impl<'this, 'ctx: 'this> Block<'ctx> {
    pub fn insts(
        &'this self,
        ctx: impl Into<lir::Context<'ctx>> + 'this,
    ) -> impl Iterator<Item = &'ctx lir::Inst> + 'this {
        self.lir.insts(ctx)
    }
}
