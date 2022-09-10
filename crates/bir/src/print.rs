use crate::types::*;
use crate::visit::*;
use crate::Visitor;

pub fn print<'bir>(map: &'bir Map) {
    Printer {
        map,
        out: &mut std::io::stdout(),
    }
    .visit();
}

pub struct Printer<'bir, 'out> {
    map: &'bir Map,
    out: &'out mut dyn std::io::Write,
}

impl Printer<'_, '_> {
    pub fn join<T>(
        &mut self,
        sep: &str,
        ts: impl Iterator<Item = T>,
        each: impl Fn(&mut Self, T) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut first = true;
        for t in ts {
            if !first {
                write!(self.out, "{sep}")?;
            }
            first = false;
            each(self, t)?;
        }
        Ok(())
    }
}

impl<'bir> Visitor<'bir> for Printer<'bir, '_> {
    fn map(&self) -> &'bir Map {
        self.map
    }

    fn visit(&mut self) {
        self.visit_module(self.map().root_module())
    }

    fn visit_typedef(&mut self, typedef: &TypeDef) {
        write!(self.out, "type {}", typedef.identifier);
        write!(self.out, " {{");
        let mut first = true;
        self.join(", ", typedef.members.iter(), |this, member| {
            write!(this.out, "{}: ", member.identifier);
            this.visit_typeref(member.ty(self.map));
            write!(this.out, "")
        });
        writeln!(self.out, "}}");
    }

    fn visit_function(&mut self, fn_: &Function) {
        write!(self.out, "fn {}", fn_.identifier);
        write!(self.out, "(");
        walk_param_list(self, fn_);
        write!(self.out, ") -> ");
        self.visit_typeref(fn_.return_type(self.map));
        writeln!(self.out);
    }

    fn visit_param(&mut self, param: &Parameter) {
        write!(self.out, "{}: ", param.identifier);
        self.visit_typeref(param.ty(self.map))
    }

    fn visit_typeref(&mut self, typeref: &TypeRef) {
        use TypeRefKind::*;
        match &typeref.kind {
            Void => {
                write!(self.out, "void");
            }
            Named { name } => {
                write!(self.out, "{name}");
            }
            Pointer { pointee } => {
                write!(self.out, "*");
                self.visit_typeref(self.map.typeref(&pointee));
            }
        }
    }
}
