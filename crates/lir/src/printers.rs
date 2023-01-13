use crate::types::*;
use std::{collections::HashSet, fmt::Write};

const DEBUG_VALUE_NUMS: bool = false;
const DEBUG_VALUE_TYS: bool = false;
const DEBUG_USERS: bool = false;
const DEBUG_UNREACHABLE: bool = false;

pub struct Writer {
    buf: String,
    indent_str: String,
}

impl Writer {
    pub fn new() -> Self {
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
        self.buf.chars().last().map_or(false, |c| c == '\n')
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

pub fn print(lir: &Module) {
    let mod_ = lir;
    println!("{}", to_string(mod_));
    if DEBUG_USERS {
        dump_users(mod_);
    }
}

pub fn dump_users(mod_: &Module) {
    for f in mod_.functions.iter() {
        let mut first_val = true;
        let indent = "   ";
        for val in f.values() {
            if first_val {
                println!("{}:", f.ident);
                first_val = false;
            }
            print!("{indent}{}: ", val.id);
            let user_sep = utils::ListSeparator::comma_space();
            for user in f.locals.users(&val.id) {
                print!("{user_sep}{}", user);
            }
            println!()
        }
    }
}

pub fn to_string(mod_: &Module) -> String {
    let mut writer = Writer::new();
    write_mod(&mut writer, mod_).expect("error writing module!");
    writer.buf
}

pub fn write_mod(w: &mut Writer, mod_: &Module) -> std::fmt::Result {
    for t in mod_.types.iter() {
        if let TyKind::Struct = t.kind {
            let struct_ty = t.as_struct_ty(mod_);
            let member_str = struct_ty
                .members
                .iter()
                .map(|id| mod_.types.get(id).repr(mod_))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(w, "type {} = {};", struct_ty.name, member_str)?;
        }
    }
    let ls = utils::ListSeparator::nl();
    for f in &mod_.functions {
        write!(w, "{ls}")?;
        write_fn(w, Context::full(mod_, f), f)?;
    }
    Ok(())
}

pub fn print_fn(m: &Module, f: &Function) -> std::fmt::Result {
    let mut w = Writer::new();
    write_fn(&mut w, Context::full(m, f), f)?;
    eprintln!("{}", w.buf);
    Ok(())
}

pub fn write_fn(w: &mut Writer, c: Context, f: &Function) -> std::fmt::Result {
    if DEBUG_VALUE_NUMS {
        write!(w, "({}) ", f.id)?;
    }
    write!(w, "fn {}(", f.ident)?;
    let ls = utils::ListSeparator::comma_space();
    for param in &f.params {
        let name = param.val.ident(c);
        let ty = param.val.ty(c).repr(c);
        write!(w, "{ls}{name}: {ty}")?;
        if DEBUG_VALUE_NUMS {
            write!(w, " ({})", param.val)?;
        }
    }
    if f.ty(c).as_fn_ty().is_var_args {
        write!(w, ", ...")?;
    }
    write!(w, ")")?;
    write!(w, " -> {}", f.return_ty(c).repr(c))?;

    if DEBUG_VALUE_TYS {
        write!(w, " (ty: {})", f.ty(c).id.0)?;
    }

    if f.insts.is_empty() {
        write!(w, ";")?;
        return Ok(());
    }

    writeln!(w, " {{")?;

    if DEBUG_UNREACHABLE {
        let mut unreachable: HashSet<_> = f.blocks().collect();
        f.visit_blocks_in_rpo(|block| {
            unreachable.remove(&block);
            write_block(block, f, w, c).unwrap();
        });

        if !unreachable.is_empty() {
            for block in unreachable {
                write!(w, "[[unreachable]] ")?;
                write_block(block, f, w, c)?;
            }
        }
    } else {
        f.visit_blocks_in_rpo(|block| {
            write_block(block, f, w, c).unwrap();
        });
    }

    write!(w, "}}")?;

    Ok(())
}

pub fn write_block(
    block: Block,
    f: &Function,
    w: &mut Writer,
    c: Context,
) -> std::fmt::Result {
    write_val(w, c, &block.val(f))?;
    writeln!(w, ":")?;
    w.indent();
    let mut any_insts = false;
    for inst in block.insts(f) {
        write_inst(w, c, inst)?;
        any_insts = true;
    }
    if any_insts {
        writeln!(w)?;
    }
    w.dedent();
    Ok(())
}

pub fn write_inst(
    w: &mut Writer,
    ctx: Context,
    inst: &Inst,
) -> std::fmt::Result {
    if DEBUG_VALUE_NUMS {
        write!(w, "({}): ", inst.val.id)?;
    }
    match inst.kind {
        InstKind::Store => {
            let dest = &inst.lval.unwrap();
            let f = ctx.as_fn();
            let store_to_var_or_param = match inst.lval.unwrap().kind(f) {
                ValueKind::Param => true,
                ValueKind::Inst => dest.is_var(f),
                kind => unreachable!("{kind:?}"),
            };
            let src = &inst.rvals[0];
            if store_to_var_or_param {
                write_val(w, ctx, dest)?;
                write!(w, " = ")?;
                write_val(w, ctx, src)?;
            } else {
                write_val(w, ctx, dest)?;
                write!(w, "^ = ")?;
                write_val(w, ctx, src)?;
            }
            writeln!(w)?;
            return Ok(());
        }
        InstKind::Var => {
            write!(w, "var ")?;
            write_val(w, ctx, &inst.lval.unwrap())?;
            write!(w, ": {}", inst.lval().ty(ctx).repr(ctx))?;
            writeln!(w)?;
            return Ok(());
        }
        _ => {}
    }

    if let Some(lval) = &inst.lval {
        write_val(w, ctx, lval)?;
        write!(w, " = ")?;
    }

    match inst.kind {
        InstKind::Var | InstKind::Store => unreachable!(), // handled above
        InstKind::Call => {
            let called_fn = &inst.rvals[0];
            write_val(w, ctx, called_fn)?;
            let arg_sep = utils::ListSeparator::comma_space();
            write!(w, "(")?;
            for op in inst.rvals.iter().skip(1) {
                write!(w, "{arg_sep}")?;
                write_val(w, ctx, op)?;
            }
            writeln!(w, ")")?;
            return Ok(());
        }
        InstKind::Load => {
            write_val(w, ctx, &inst.rvals[0])?;
            writeln!(w, "^")?;
            return Ok(());
        }
        InstKind::Cast => {
            let ty = inst.lval().ty(ctx).repr(ctx);
            write!(w, "@cast.{} ", ty)?;
            write_val(w, ctx, &inst.rvals[0])?;
            writeln!(w)?;
            return Ok(());
        }
        InstKind::Add => write!(w, "add")?,
        InstKind::Return => write!(w, "return")?,
        InstKind::Cmp { kind } => {
            write!(w, "(")?;
            write_val(w, ctx, &inst.rvals[0])?;
            match kind {
                CmpKind::Eq => write!(w, " == ")?,
                CmpKind::Ne => write!(w, " != ")?,
                CmpKind::Gt => write!(w, " > ")?,
                CmpKind::Lt => write!(w, " < ")?,
                CmpKind::Gte => write!(w, " >= ")?,
                CmpKind::Lte => write!(w, " <= ")?,
            };
            write_val(w, ctx, &inst.rvals[1])?;
            write!(w, ")")?;
            writeln!(w)?;
            return Ok(());
        }
        InstKind::Copy => write!(w, "copy")?,
        InstKind::Nop => write!(w, "nop")?,
        InstKind::Jmp => write!(w, "jmp")?,
        InstKind::Branch => write!(w, "br")?,
        InstKind::Sub => write!(w, "sub")?,
        InstKind::Mul => write!(w, "mul")?,
        InstKind::Div => write!(w, "div")?,
        InstKind::Subscript => write!(w, "subscript")?,
        InstKind::GetField => write!(w, "field")?,
    };

    write!(w, " ")?;

    let ls = utils::ListSeparator::comma_space();
    for rval in &inst.rvals {
        write!(w, "{ls}")?;
        write_val(w, ctx, rval)?;
    }
    writeln!(w)?;

    Ok(())
}

pub fn write_val(
    w: &mut Writer,
    ctx: Context,
    val: &ValueRef,
) -> std::fmt::Result {
    write!(w, "{}", val.repr(ctx.clone()))?;
    if DEBUG_VALUE_NUMS {
        write!(w, " ({})", val.id)?;
    }
    Ok(())
}
