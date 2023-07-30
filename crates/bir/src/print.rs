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
        match mod_.ident {
            Some(ref name) => {
                w!(self, "mod {name} ");
                if mod_.imported {
                    w!(self, "(imported) ");
                }
                wln!(self, "{{");
                self.indented(|this| {
                    visit::walk_module(this, mod_);
                });
                wln!(self, "}}");
            }
            None => {
                visit::walk_module(self, mod_);
            }
        }
    }

    fn visit_import(&mut self, import: &Import) {
        if DEBUG_IDS {
            w!(self, "{:?} ", import.id);
        }
        wln!(self, "import {};", import.name);
    }

    fn visit_typedef(&mut self, typedef: &TypeDef) {
        if DEBUG_IDS {
            w!(self, "{:?} ", typedef.id);
        }
        w!(self, "type {}", typedef.identifier);
        w!(self, " {{");
        let ls = utils::ListSeparator::comma_space();
        for member in typedef.members.iter() {
            w!(self, "{ls}{}: ", member.ident);
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
        if fn_.is_var_args {
            w!(self, ", ...");
        }
        w!(self, ") -> ");
        self.visit_typeref(fn_.return_type(self.map));
        if fn_.is_extern {
            w!(self, " extern");
        }
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
                self.visit_name(self.map.name(name));
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
        if scope.items.is_empty() && scope.return_expr.is_none() {
            wln!(self, "}}");
        } else {
            self.indented(|this| {
                wln!(this);
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
        w!(self, "let {}", let_.ident);
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
        match &expr.kind {
            ExprKind::Literal(id) => {
                if DEBUG_IDS {
                    w!(self, "{:?} ", id);
                }
                match self.map.lit(id) {
                    Literal::Number(n) => w!(self, "{n}"),
                    Literal::Str(s) => w!(self, "{s:?}"),
                    Literal::Struct(lit) => {
                        self.visit_name(self.map.name(&lit.name));
                        w!(self, "{{}}")
                    }
                };
            }
            ExprKind::NameRef { id: name } => {
                self.visit_name(self.map.name(name));
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
            ExprKind::Op(op) => {
                w!(self, "(");
                self.visit_op(op);
                w!(self, ")");
            }
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
    }

    fn visit_name(&mut self, n: &Name) {
        w!(self, "{}", n.segments.join("::"));
    }
}

impl Printer<'_> {
    fn visit_op(&mut self, op: &Op) {
        match op {
            Op::Prefix { kind, arg } => {
                let prefix = match kind {
                    PrefixOpKind::Plus => "+",
                    PrefixOpKind::Negate => "-",
                    PrefixOpKind::Deref => "*",
                };
                w!(self, "{prefix}");
                self.visit_expr(self.map.expr(arg));
            }
            Op::Postfix { kind, arg } => todo!(),
            Op::Binary { kind, lhs, rhs } => {
                let infix = match kind {
                    BinaryOpKind::Add => " + ",
                    BinaryOpKind::Sub => " - ",
                    BinaryOpKind::Mul => " * ",
                    BinaryOpKind::Div => " / ",
                    BinaryOpKind::Assign => " = ",
                    BinaryOpKind::DotAccess => ".",
                    BinaryOpKind::ArrowAccess => "->",
                    BinaryOpKind::LessThan => " < ",
                    BinaryOpKind::LessThanEquals => " <= ",
                    BinaryOpKind::GreaterThan => " > ",
                    BinaryOpKind::GreaterThanEquals => " >= ",
                    BinaryOpKind::Equals => " == ",
                    BinaryOpKind::NotEquals => " != ",
                };
                self.visit_expr(self.map.expr(lhs));
                w!(self, "{infix}");
                self.visit_expr(self.map.expr(rhs));
            }
        }
    }
}
