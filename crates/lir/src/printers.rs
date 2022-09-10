use crate::types::*;
use std::fmt::Write;

struct Writer {
    buf: String,
    indent_str: String,
}

impl Writer {
    fn new() -> Self {
        Self {
            buf: String::new(),
            indent_str: String::new(),
        }
    }

    fn indent(&mut self) {
        self.indent_str.push_str(&str::repeat(" ", 4));
    }

    fn dedent(&mut self) {
        self.indent_str.truncate(self.indent_str.len() - 4);
    }

    fn last_was_newline(&self) -> bool {
        self.buf.chars().last().map(|c| c == '\n').unwrap_or(false)
    }
}

impl std::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if !self.indent_str.is_empty() && self.last_was_newline() {
            self.buf.write_str(&self.indent_str)?;
        }
        self.buf.write_str(s)
    }
}

pub fn print(sema: &sema::Map, mod_: &Module) {
    println!("{}", to_string(sema, mod_))
}

pub fn dump_users(mod_: &Module) {
    for f in mod_.functions.iter() {
        let mut first_val = true;
        let val_sep = utils::ListSeparator::new("\n");
        for val in f.values() {
            if first_val {
                println!("{}:", f.ident);
                first_val = false;
            }
            let user_sep = utils::ListSeparator::new(", ");
            let mut first_user = true;
            for user in f.users(&val.id) {
                if first_user {
                    print!("  {val_sep}{:?}: ", val.id);
                    first_user = false;
                }
                print!("{user_sep}{:?}", user);
            }
        }
    }
}

pub fn to_string(sema: &sema::Map, mod_: &Module) -> String {
    let mut writer = Writer::new();
    write_mod(&mut writer, sema, mod_);
    writer.buf
}

fn write_mod(w: &mut Writer, sema: &sema::Map, mod_: &Module) {
    let ls = utils::ListSeparator::new("\n");
    for f in &mod_.functions {
        write!(w, "{ls}");
        write_fn(w, sema, &Context::full(mod_, f), f);
    }
}

fn write_fn(w: &mut Writer, sema: &sema::Map, c: &Context, f: &Function) {
    write!(w, "fn {}(", f.ident);
    let ls = utils::ListSeparator::default();
    for param in &f.params {
        let param = sema.param(f.sema(param).unwrap()).unwrap();
        let name = param.ident(sema);
        let ty = param.ty(sema).repr(sema);
        write!(w, "{ls}{name}: {ty}");
    }
    write!(w, ")");

    if f.insts.is_empty() {
        write!(w, ";");
        return;
    }

    writeln!(w, "{{");
    w.indent();
    let ls = utils::ListSeparator::new("\n");
    for inst in &f.insts {
        write!(w, "{ls}");
        write_inst(w, sema, c, inst);
    }
    w.dedent();
    write!(w, "\n}}");
}

fn write_inst(w: &mut Writer, sema: &sema::Map, ctx: &Context, inst: &Inst) {
    match inst.kind {
        InstKind::Store => {
            let dest = &inst.lval.unwrap();
            let f = ctx.as_fn();
            let store_to_var_or_param = match inst.val.kind(f) {
                ValueKind::Param => true,
                ValueKind::Inst => dest.is_var(f),
                _ => false,
            };
            let src = &inst.rvals[0];
            if store_to_var_or_param {
                write_val(w, sema, ctx, dest);
                write!(w, " = ");
                write_val(w, sema, ctx, src);
            } else {
                write_val(w, sema, ctx, dest);
                write!(w, "^ = ");
                write_val(w, sema, ctx, src);
            }
            return;
        }
        InstKind::Var => {
            write!(w, "var ");
            write_val(w, sema, ctx, &inst.val);
            return;
        }
        _ => {}
    }

    if let Some(lval) = &inst.lval {
        write_val(w, sema, ctx, lval);
        write!(w, " = ");
    }

    match inst.kind {
        InstKind::Call => {
            let called_fn = &inst.rvals[0];
            write_val(w, sema, ctx, called_fn);
            let arg_sep = utils::ListSeparator::new(", ");
            write!(w, "(");
            for op in inst.rvals.iter().skip(1) {
                write!(w, "{arg_sep}");
                write_val(w, sema, ctx, op)
            }
            write!(w, ")");
            return;
        }
        InstKind::Offset => {
            write!(w, "&");
            write_val(w, sema, ctx, &inst.rvals[0]);
            write!(w, "[");
            write_val(w, sema, ctx, &inst.rvals[1]);
            write!(w, "]");
            return;
        }
        InstKind::Load => {
            write_val(w, sema, ctx, &inst.rvals[0]);
            write!(w, "^");
            return;
        }
        InstKind::Add => write!(w, "add "),
        InstKind::Return => write!(w, "return "),
        InstKind::Cmp { kind } => match kind {
            CmpKind::Eq => write!(w, "eq"),
        },
        InstKind::Copy => write!(w, "copy "),
        _ => unreachable!(),
    };

    let ls = utils::ListSeparator::default();
    for rval in &inst.rvals {
        write!(w, "{ls}");
        write_val(w, sema, ctx, rval);
    }
}

const DEBUG_VALUE_NUMS: bool = false;

fn write_val(w: &mut Writer, sema: &sema::Map, ctx: &Context, val: &ValueRef) {
    write!(w, "{}", val.repr(ctx.clone(), sema));
    if DEBUG_VALUE_NUMS {
        write!(w, " ({})", val.id.0);
    }
}
