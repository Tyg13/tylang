use std::collections::HashMap;
use utils::vec_graph::traversal;

use crate::types::{
    Block, BlockData, BlockGraph, Inst, InstKind, Ty, TyContext, TyID, Users,
    Value, ValueID, ValueKind, ValueRef, Values,
};

#[derive(Debug)]
pub struct Function {
    pub id: ValueID,
    pub ty: TyID,
    pub ident: String,
    pub params: Vec<Param>,
    pub internal: bool,

    pub(crate) insts: HashMap<ValueID, Inst>,
    pub(crate) locals: Values,

    pub(crate) blocks: BlockGraph,
    pub(crate) blocks_by_id: HashMap<ValueID, Block>,
    pub(crate) blocks_by_label: HashMap<String, Block>,
}

impl Function {
    pub fn new(
        types: &TyContext,
        ident: String,
        mut param_names: Vec<String>,
        internal: bool,
        id: ValueID,
        fn_ty: TyID,
    ) -> Self {
        debug_assert!(id.is_global());
        let mut this = Self {
            id,
            ty: fn_ty,
            ident,
            params: Default::default(),
            internal,
            insts: Default::default(),
            locals: Default::default(),
            blocks: Default::default(),
            blocks_by_id: Default::default(),
            blocks_by_label: Default::default(),
        };
        this.params = types
            .get(&fn_ty)
            .as_fn_ty()
            .params(types)
            .enumerate()
            .map(|(idx, param)| {
                let val = this.add_val(
                    ValueKind::Param,
                    param.id,
                    Some(std::mem::take(&mut param_names[idx])),
                );
                Param { val }
            })
            .collect();
        this
    }

    pub fn nth_param(&self, n: usize) -> &Param {
        self.params.get(n).expect(&format!("{n} out of bounds!"))
    }

    pub fn param_num(&self, val: impl Into<ValueID>) -> Option<usize> {
        let val = val.into();
        debug_assert!(val.is_local());
        let idx = val.as_idx();
        debug_assert!(idx < self.params.len());
        Some(idx)
    }

    #[inline]
    pub fn blocks(&self) -> impl Iterator<Item = Block> + '_ {
        self.blocks.vertices().map(|b| Block(b))
    }

    #[inline]
    pub fn num_blocks(&self) -> usize {
        self.blocks.num_vertices()
    }

    pub fn visit_blocks_in_po(&self, mut f: impl FnMut(Block)) {
        traversal::post_order(&self.blocks, &mut |node| {
            f(Block(node));
        });
    }

    pub fn visit_blocks_in_rpo(&self, mut f: impl FnMut(Block)) {
        traversal::reverse_post_order(&self.blocks, &mut |node| {
            f(Block(node));
        });
    }

    #[inline]
    pub fn ty<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> &'ctx Ty {
        self.ty.get(ctx.into())
    }

    #[inline]
    pub fn return_ty<'ctx>(&self, ctx: impl Into<&'ctx TyContext>) -> &'ctx Ty {
        let ctx = ctx.into();
        self.ty(ctx).as_fn_ty().return_ty(ctx)
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        self.locals.values()
    }

    #[inline]
    pub(crate) fn add_val(
        &mut self,
        kind: ValueKind,
        ty: TyID,
        ident: Option<String>,
    ) -> ValueID {
        self.locals
            .add_val(kind, ty, ident.map(|i| i.to_string()), false)
    }

    pub fn inst(&self, val: &ValueID) -> Option<&Inst> {
        debug_assert!(val.is_local());
        self.insts.get(val)
    }

    pub(crate) fn inst_mut(&mut self, val: ValueID) -> Option<&mut Inst> {
        debug_assert!(val.is_local());
        self.insts.get_mut(&val)
    }

    pub(crate) fn add_inst(
        &mut self,
        kind: InstKind,
        ty: TyID,
        block: Block,
        lval: Option<ValueRef>,
        rvals: Vec<ValueRef>,
        ident: Option<String>,
    ) -> ValueRef {
        let inst_val = ValueRef::new(self.add_val(ValueKind::Inst, ty, ident))
            .with_parent(block.val(self).id);
        let lval = if kind.can_have_lvals() {
            lval.map(|v| v.with_parent(inst_val.id))
        } else {
            debug_assert_eq!(lval, None);
            None
        };
        let rvals = rvals
            .into_iter()
            .map(|val| {
                self.locals.add_user(val.id, inst_val.id);
                val.with_parent(inst_val.id)
            })
            .collect();
        self.insts.insert(
            inst_val.id,
            Inst {
                val: inst_val,
                kind,
                lval,
                rvals,
            },
        );

        block.add_inst(self, inst_val);

        inst_val
    }

    pub(crate) fn remove_inst(&mut self, id: &ValueID) {
        let (rvals, parent) = {
            let inst = self.inst(id).unwrap();
            let rvals = inst.rvals.clone();
            let parent = inst.block(self);
            (rvals, parent)
        };
        for rval in rvals {
            self.locals.remove_user(rval.id, *id);
        }
        parent.remove_inst(self, id);
        self.insts.remove(id);
    }

    #[inline]
    pub(crate) fn block(&self, id: &ValueID) -> Block {
        debug_assert_eq!(self.locals.get(id).kind, ValueKind::Block);
        self.blocks_by_id[&id]
    }

    pub(crate) fn add_block<'a>(
        &mut self,
        label: Option<String>,
        ty: TyID,
    ) -> Block {
        let val =
            ValueRef::new(self.add_val(ValueKind::Block, ty, label.clone()))
                .with_parent(self.id);
        let block = Block(self.blocks.add_vertex(BlockData {
            insts: Vec::new(),
            val,
        }));
        self.blocks_by_id.insert(val.id, block);
        if let Some(label) = label {
            self.blocks_by_label.insert(label.to_string(), block);
        }
        block
    }

    pub(crate) fn remove_blocks(&mut self, vals: &[ValueID]) {
        let blocks: Vec<_> =
            vals.iter().map(|v| self.blocks_by_id[v].0).collect();
        self.blocks.unlink(&blocks);
        self.blocks_by_id.retain(|k, _| !vals.contains(k));
    }

    #[inline]
    pub(crate) fn add_block_edge(&mut self, from: Block, to: Block) {
        self.blocks.add_edge(from.0, to.0);
    }
}

#[derive(Debug)]
pub struct Param {
    pub val: ValueID,
}
