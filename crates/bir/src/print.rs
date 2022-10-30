use crate::types::*;
use crate::{visit, Visitor};
use std::fmt::Write;

const DEBUG_IDS: bool = false;

pub fn print<'bir>(map: &'bir Map) {
    let mut p = Printer {
        map,
        buf: String::new(),
        indent: 0,
    };
    p.visit_root();
    print!("{}", p.buf);
}

pub struct Printer<'bir> {
    map: &'bir Map,
    buf: String,
    indent: usize,
}

impl Printer<'_> {
    pub fn indent_up(&mut self) -> usize {
        self.indent += 2;
        self.indent
    }

    pub fn indent_down(&mut self) -> usize {
        self.indent -= 2;
        self.indent
    }

    pub fn indented(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent_up();
        f(self);
        self.indent_down();
    }
}

impl std::fmt::Write for Printer<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if self.buf.ends_with("\n") {
            self.buf.push_str(&" ".repeat(self.indent));
        }
        self.buf.push_str(s);
        Ok(())
    }
}

macro_rules! w { ($($args:tt)*) => { write!($($args)*).unwrap() } }
macro_rules! wln { ($($args:tt)*) => { writeln!($($args)*).unwrap() } }

impl<'bir> Visitor<'bir> for Printer<'bir> {
    fn map(&self) -> &'bir Map {
        self.map
    }

    fn visit_module(&mut self, mod_: &Module) {
        if DEBUG_IDS {
            w!(self, "{:?} ", mod_.id);
        }
        let walk_inner = |this: &mut Self, m| {
            visit::walk_modules(this, m);
            visit::walk_typedefs(this, m);
            visit::walk_functions(this, m);
        };
        match mod_.name {
            Some(ref name) => {
                wln!(self, "mod {name} {{");
                self.indented(|this| {
                    walk_inner(this, mod_);
                });
                wln!(self, "}}");
            }
            None => {
                walk_inner(self, mod_);
            }
        }
    }

    fn visit_typedef(&mut self, typedef: &TypeDef) {
        if DEBUG_IDS {
            w!(self, "{:?} ", typedef.id);
        }
        w!(self, "type {}", typedef.identifier);
        w!(self, " {{");
        let ls = utils::ListSeparator::comma_space();
        for member in typedef.members.iter() {
            w!(self, "{ls}{}: ", member.identifier);
            self.visit_typeref(member.ty(self.map));
        }
        wln!(self, "}}");
    }

    fn visit_function(&mut self, fn_: &Function) {
        if DEBUG_IDS {
            w!(self, "{:?} ", fn_.id);
        }
        w!(self, "fn {}", fn_.identifier);
        w!(self, "(");
        let ls = utils::ListSeparator::comma_space();
        for param in fn_.parameters(self.map) {
            w!(self, "{ls}");
            self.visit_param(param);
        }
        w!(self, ") -> ");
        self.visit_typeref(fn_.return_type(self.map));
        if let Some(body) = fn_.body(self.map) {
            w!(self, " ");
            self.visit_block(body);
        } else {
            w!(self, ";");
        }
        wln!(self);
    }

    fn visit_param(&mut self, param: &Parameter) {
        if DEBUG_IDS {
            w!(self, "{:?} ", param.id);
        }
        w!(self, "{}: ", param.identifier);
        self.visit_typeref(param.ty(self.map));
    }

    fn visit_typeref(&mut self, typeref: &TypeRef) {
        if DEBUG_IDS {
            w!(self, "{:?} ", typeref.id);
        }
        use TypeRefKind::*;
        match &typeref.kind {
            Void => {
                w!(self, "void");
            }
            Named { name } => {
                w!(self, "{name}");
            }
            Pointer { pointee } => {
                w!(self, "*");
                self.visit_typeref(self.map.typeref(&pointee));
            }
        };
    }

    fn visit_block(&mut self, scope: &Block) {
        if DEBUG_IDS {
            w!(self, "{:?} ", scope.id);
        }
        if let Some(label) = &scope.label {
            w!(self, "['{label}]: ");
        }
        w!(self, "{{");
        wln!(self);
        self.indented(|this| {
            for item in scope.items(this.map()) {
                this.visit_item(item);
                wln!(this, ";");
            }
            if let Some(expr) = scope.return_expr(this.map()) {
                this.visit_expr(expr);
                wln!(this);
            }
        });
        w!(self, "}}");
    }

    fn visit_item(&mut self, item: &Item) {
        if DEBUG_IDS {
            w!(self, "{:?} ", item.id);
        }
        match &item.kind {
            ItemKind::Let(id) => {
                self.visit_let(self.map.let_(id));
            }
            ItemKind::Expr(id) => {
                self.visit_expr(self.map.expr(id));
            }
        }
    }

    fn visit_let(&mut self, let_: &Let) {
        if DEBUG_IDS {
            w!(self, "{:?} ", let_.id);
        }
        w!(self, "let {}", let_.name);
        if let Some(ty) = let_.ty(self.map) {
            w!(self, ": ");
            self.visit_typeref(ty);
        }
        if let Some(expr) = let_.expr(self.map) {
            w!(self, " = ");
            self.visit_expr(expr);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if DEBUG_IDS {
            w!(self, "{:?} ", expr.id);
        }
        w!(self, "(");
        match &expr.kind {
            ExprKind::Literal(id) => {
                if DEBUG_IDS {
                    w!(self, "{:?} ", id);
                }
                match self.map.lit(id) {
                    Literal::Number(n) => w!(self, "{n}"),
                    Literal::Str(s) => w!(self, "{s:?}"),
                };
            }
            ExprKind::NameRef { name } => {
                w!(self, "{name}");
            }
            ExprKind::Cast { val, to } => {
                let val = self.map.expr(val);
                let ty = self.map.typeref(to);
                self.visit_expr(val);
                w!(self, " as ");
                self.visit_typeref(ty);
            }
            ExprKind::Call { receiver, operands } => {
                let fn_ = self.map.expr(receiver);
                self.visit_expr(fn_);
                w!(self, "(");
                let ls = utils::ListSeparator::comma_space();
                for arg in operands {
                    w!(self, "{ls}");
                    self.visit_expr(self.map.expr(arg));
                }
                w!(self, ")");
            }
            ExprKind::Index { receiver, index } => {
                let val = self.map.expr(receiver);
                let index = self.map.expr(index);
                self.visit_expr(val);
                w!(self, "[");
                self.visit_expr(index);
                w!(self, "]");
            }
            ExprKind::Op(op) => self.visit_op(op),
            ExprKind::Block { scope: id } => {
                self.visit_block(self.map.block(id));
            }
            ExprKind::Return { expr } => {
                w!(self, "return");
                if let Some(expr) = expr {
                    w!(self, " ");
                    self.visit_expr(self.map.expr(expr));
                }
            }
            ExprKind::Break { label } => {
                w!(self, "break '{label}");
            }
            ExprKind::Continue { label } => {
                w!(self, "continue '{label}");
            }
            ExprKind::Branch {
                condition,
                kind,
                left,
                right,
            } => {
                w!(self, "if ");
                self.visit_expr(self.map.expr(condition));
                w!(self, " ");
                match kind {
                    BranchKind::If => {
                        self.visit_block(self.map.block(left));
                    }
                    BranchKind::IfElse => {
                        self.visit_block(self.map.block(left));
                        w!(self, " else ");
                        self.visit_block(self.map.block(&right.unwrap()));
                    }
                }
            }
            ExprKind::Loop { kind: _, body } => {
                w!(self, "loop ");
                self.visit_block(self.map.block(body));
            }
        };
        w!(self, ")");
    }
}

impl Printer<'_> {
    fn visit_op(&mut self, op: &Op) {
        match (&op.fixity, &op.kind) {
            (OpFixity::Infix, OpKind::Plus) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " + ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::Minus) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " - ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::Multiply) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " * ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::Divide) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " / ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::Assignment) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " = ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::FieldAccess) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, ".");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::ScopeAccess) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, "::");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::LessThan) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " < ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::LessThanEquals) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " <= ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::GreaterThan) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, " > ");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::GreaterThanEquals) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, ">=");
                self.visit_expr(rhs);
            }
            (OpFixity::Infix, OpKind::Equals) => {
                let lhs = self.map.expr(&op.operands[0]);
                let rhs = self.map.expr(&op.operands[1]);
                self.visit_expr(lhs);
                w!(self, "==");
                self.visit_expr(rhs);
            }
            _ => unreachable!(),
        }
    }
}
