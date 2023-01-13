use std::collections::HashMap;

use crate::pass::*;
use crate::types::*;

pub struct DCE;
impl FunctionPass for DCE {
    fn name(&self) -> &'static str {
        "DCE"
    }
    fn visit_function(
        &mut self,
        f: &mut Function,
        _: &PassContext,
    ) -> PassStatus {
        let mut status = PassStatus::NoChange;

        let mut dead_insts = Vec::new();
        let mut change = true;
        while change {
            change = false;
            for (id, i) in &f.insts {
                let Some(lval) = i.lval else {
                    continue
                };
                if i.kind == InstKind::Call {
                    continue;
                }
                let num_users = lval.users(&*f).count();
                if num_users != 0 {
                    continue;
                }
                dead_insts.push((*id, lval));
                status = PassStatus::Changed;
            }

            for i in &dead_insts {
                println!("Removing '{}'", i.1.repr(&*f));
                f.remove_inst(&i.0);
                change = true;
            }
            dead_insts.clear();
        }

        status
    }
}

pub struct JumpThreading;
impl FunctionPass for JumpThreading {
    fn name(&self) -> &'static str {
        "Jump Threading"
    }
    fn visit_function(
        &mut self,
        f: &mut Function,
        _: &PassContext,
    ) -> PassStatus {
        #[derive(Debug)]
        struct Thread {
            pred: Block,
            to_remove: Block,
            succ: Block,
        }

        let mut status = PassStatus::NoChange;
        let mut threads = Vec::new();
        f.visit_blocks_in_rpo(&mut |block: Block| {
            let c = Context::fn_(&*f);
            let insts: Vec<_> = block.insts(c).collect();
            if insts.len() != 1 {
                return;
            }
            let i = insts[0];
            if i.kind != InstKind::Jmp {
                return;
            }
            if block.num_predecessors(c) != 1 || block.num_successors(c) != 1 {
                return;
            }

            let pred = block.predecessors(c).next().unwrap();
            let succ = block.successors(c).next().unwrap();
            if pred == succ {
                return;
            }

            threads.push(Thread {
                pred,
                to_remove: block,
                succ,
            });
            status = PassStatus::Changed;
        });

        let mut blocks_to_remove = Vec::with_capacity(threads.len());
        let mut removed_block_preds: HashMap<Block, Block> =
            HashMap::with_capacity(threads.len());

        for t in &threads {
            let succ = t.succ.val(f);
            let to_remove = t.to_remove.val(f);
            let pred = match removed_block_preds.get(&t.pred) {
                Some(pred) => *pred,
                None => t.pred,
            };
            let terminator = pred.terminator(f);
            for rval in terminator.inst_mut(f).rvals.iter_mut() {
                if rval.id == to_remove.id {
                    rval.id = succ.id;
                }
            }
            f.add_block_edge(pred, t.succ);
            eprintln!(
                "Threading {} to {}, removing {}",
                pred.repr(f),
                t.succ.repr(f),
                t.to_remove.repr(f)
            );
            blocks_to_remove.push(to_remove.id);
            removed_block_preds.insert(t.to_remove, pred);
        }

        f.remove_blocks(&blocks_to_remove);

        status
    }

    fn should_run_on(&self, _f: &Function) -> bool {
        true
    }
}
