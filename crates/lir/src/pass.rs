use crate::types::*;

pub enum PassStatus {
    NoChange,
    Changed,
}

pub trait FunctionPass {
    fn name(&self) -> &'static str;
    fn visit_function(
        &mut self,
        f: &mut Function,
        ctx: &PassContext,
    ) -> PassStatus;
    fn should_run_on(&self, _f: &Function) -> bool {
        true
    }
}

pub struct PassContext<'c> {
    pub types: &'c TyContext,
}

pub fn run_pass(m: &mut Module, p: &mut dyn FunctionPass) {
    let ctx = PassContext { types: &m.types };
    for idx in 0..m.functions.len() {
        {
            let f = &m.functions[idx];
            if !p.should_run_on(f) {
                continue;
            }
            eprintln!("Running {} on {}", p.name(), f.ident);
        }
        match p.visit_function(&mut m.functions[idx], &ctx) {
            PassStatus::Changed => {
                let f = &m.functions[idx];
                eprintln!();
                crate::printers::print_fn(m, f).unwrap();
            }
            PassStatus::NoChange => eprintln!("  No change"),
        }
    }
}

pub fn run_passes(m: &mut Module, passes: &mut [&mut dyn FunctionPass]) {
    for pass in passes {
        run_pass(m, *pass);
    }
}
