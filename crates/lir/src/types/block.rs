use utils::vec_graph::{VecGraph, Vertex};

use crate::types::{Context, Function, Inst, ValueID, ValueRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Block(pub(crate) Vertex<BlockData>);

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

    pub(crate) fn remove_inst<'f>(&self, f: &mut Function, id: &ValueID) {
        let d = self.data_mut(f);
        d.insts.retain(|i| i.id != *id);
    }

    pub fn val<'f>(&self, ctx: &'f Function) -> ValueRef {
        self.data(ctx).val
    }

    pub fn repr<'f>(&self, ctx: &'f Function) -> String {
        self.val(ctx).repr(ctx)
    }

    pub fn predecessors<'f>(
        &self,
        ctx: impl Into<Context<'f>>,
    ) -> impl Iterator<Item = Block> + 'f {
        let f: &Function = ctx.into().as_fn();
        self.0.predecessors(&f.blocks).iter().map(|&b| Block(b))
    }

    pub fn num_predecessors<'f>(&self, ctx: impl Into<Context<'f>>) -> usize {
        let f: &Function = ctx.into().as_fn();
        self.0.in_degree(&f.blocks)
    }

    pub fn successors<'f>(
        &self,
        ctx: impl Into<Context<'f>>,
    ) -> impl Iterator<Item = Block> + 'f {
        let f: &Function = ctx.into().as_fn();
        self.0.successors(&f.blocks).iter().map(|&b| Block(b))
    }

    pub fn num_successors<'f>(&self, ctx: impl Into<Context<'f>>) -> usize {
        let f: &Function = ctx.into().as_fn();
        self.0.out_degree(&f.blocks)
    }

    pub fn terminator<'f>(&self, f: &'f Function) -> ValueRef {
        let d = self.data(f);
        *d.insts.last().unwrap()
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
