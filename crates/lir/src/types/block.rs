use utils::vec_graph::{NodeRef, VecGraph};

use crate::types::{Context, Function, Inst, ValueRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Block(pub(crate) NodeRef<BlockData>);

impl Block {
    fn data<'this, 'f: 'this>(
        &'this self,
        ctx: impl Into<Context<'f>>,
    ) -> &'f BlockData {
        self.0.data(&ctx.into().as_fn().blocks)
    }

    fn data_mut<'this, 'f: 'this>(
        &'this self,
        f: &'f mut Function,
    ) -> &'f mut BlockData {
        self.0.data_mut(&mut f.blocks)
    }

    pub fn insts<'this, 'f: 'this>(
        &'this self,
        ctx: impl Into<Context<'f>>,
    ) -> impl Iterator<Item = &'f Inst> + 'this {
        let f = ctx.into().as_fn();
        self.data(f).insts(f)
    }

    pub(crate) fn add_inst<'f>(&self, f: &mut Function, val: ValueRef) {
        self.data_mut(f).insts.push(val);
    }

    pub fn val<'f>(&self, ctx: impl Into<Context<'f>>) -> ValueRef {
        self.data(ctx).val
    }

    pub fn num_predecessors<'f>(&self, ctx: impl Into<Context<'f>>) -> usize {
        let f: &Function = ctx.into().as_fn();
        self.0.in_degree(&f.blocks)
    }
}

pub type BlockGraph = VecGraph<BlockData>;

#[derive(Debug)]
pub struct BlockData {
    pub val: ValueRef,
    pub insts: Vec<ValueRef>,
}

impl BlockData {
    pub fn insts<'this, 'f: 'this>(
        &'this self,
        ctx: impl Into<Context<'f>>,
    ) -> impl Iterator<Item = &'f Inst> + 'this {
        let f = ctx.into().as_fn();
        self.insts.iter().map(move |val| val.inst(f).unwrap())
    }
}
