use crate::builder::Session;
use crate::types::*;
use crate::utils::lookup_name;
use crate::Builder;

#[derive(Debug, PartialEq, Eq)]
enum ValueCategory {
    LVal,
    RVal,
}

pub fn translate(bir: &bir::Map, sema: &sema::Map) -> Module {
    let sess = Session { bir, sema };
    let mut builder = Builder::new(&sess);
    root_module(&sess, &mut builder, bir.root_module())
}

fn root_module(sess: &Session, builder: &mut Builder, mod_: &bir::Module) -> Module {
    let sema = sess.bir_to_sema(mod_.id);
    builder.new_module(sema);

    builder.set_void({
        let ns = sess.sema.ns(sema).unwrap();
        ns.lookup_ty(sess.sema, "void").unwrap().id
    });

    for f in mod_.functions(sess.bir) {
        fn_(sess, builder, f);
    }

    builder.finish_module()
}

fn module(sess: &Session, builder: &mut Builder, mod_: &bir::Module) -> Module {
    builder.new_module(sess.bir_to_sema(mod_.id));

    for f in mod_.functions(sess.bir) {
        fn_(sess, builder, f);
    }

    builder.finish_module()
}

fn fn_(sess: &Session, builder: &mut Builder, f: &bir::Function) {
    let params = f
        .parameters
        .iter()
        .map(|&id| sess.bir_to_sema(id))
        .collect();
    let fn_sema = sess.bir_to_sema(f.id);
    let fn_name = sess.sema.name(fn_sema).unwrap();
    builder.new_function(&fn_name.ident, fn_sema, params, sess.sema);
    if let Some(body) = f.body(sess.bir) {
        let val = scope_(sess, builder, body);
        builder.new_return(sess.bir_to_sema(body.id), val).build();
    }
    builder.finish_function()
}

fn scope_(sess: &Session, builder: &mut Builder, scope: &bir::Scope) -> ValueRef {
    builder.push_ns(sess.bir_to_sema(scope.id));

    for let_ in scope.lets(sess.bir) {
        let sema = sess.bir_to_sema(let_.id);
        builder.new_var(sema).named(let_.name.clone()).build();
    }

    for it in scope.items(sess.bir) {
        item(sess, builder, it);
    }

    let ret_val = if let Some(e) = scope.return_expr(sess.bir) {
        rvalue(sess, builder, None, e)
    } else {
        builder.void_()
    };

    builder.pop_ns();

    ret_val
}

fn item(sess: &Session, builder: &mut Builder, it: &bir::Item) {
    match &it.kind {
        bir::ItemKind::Let(id) => {
            let let_ = &sess.bir.let_(id);
            let expr = let_.expr(sess.bir).unwrap();
            let lval = lookup_name(sess, builder, &let_.name);
            rvalue(sess, builder, Some(lval), expr);
        }
        bir::ItemKind::Expr(id) => {
            value(sess, builder, ValueCategory::RVal, None, sess.bir.expr(id));
        }
    }
}

fn lvalue(sess: &Session, builder: &mut Builder, e: &bir::Expr) -> ValueRef {
    value(sess, builder, ValueCategory::LVal, None, e)
}

fn rvalue(
    sess: &Session,
    builder: &mut Builder,
    lval: Option<ValueRef>,
    e: &bir::Expr,
) -> ValueRef {
    value(sess, builder, ValueCategory::RVal, lval, e)
}

fn value(
    sess: &Session,
    builder: &mut Builder,
    cat: ValueCategory,
    lval: Option<ValueRef>,
    e: &bir::Expr,
) -> ValueRef {
    let sema = sess.bir_to_sema(e.id);
    let val = match &e.kind {
        bir::ExprKind::Literal(..) => {
            debug_assert_ne!(cat, ValueCategory::LVal);
            let val = builder.new_constant(sema);
            if let Some(lval) = lval {
                builder.new_copy(val).with_lval(lval).sema(sema).build()
            } else {
                val
            }
        }
        bir::ExprKind::NameRef { name } => {
            let named_val = lookup_name(sess, builder, &name);
            if let Some(lval) = lval {
                if lval.is_var(builder.ctx()) {
                    return builder.new_copy(named_val).with_lval(lval).sema(sema).build();
                }
            }
            named_val
        }
        bir::ExprKind::Call { receiver, operands } => {
            let fn_name = &sess.sema.name(sess.bir_to_sema(*receiver)).unwrap().ident;
            let called_fn = lookup_name(sess, builder, fn_name);
            let ops: Vec<_> = operands
                .iter()
                .map(|op| rvalue(sess, builder, None, sess.bir.expr(op)))
                .collect();
            builder
                .new_call(called_fn, ops)
                .with_maybe_lval(lval)
                .sema(sema)
                .build()
        }
        bir::ExprKind::Index { receiver, index } => {
            let offset = rvalue(sess, builder, lval, sess.bir.expr(index));
            let base = lvalue(sess, builder, sess.bir.expr(receiver));
            let val = builder.new_offset(base, offset).sema(sema).build();
            match cat {
                ValueCategory::LVal => val,
                ValueCategory::RVal => builder.new_load(val).sema(sema).build(),
            }
        }
        bir::ExprKind::Op(op) => op_expr(sess, builder, sema, lval, op),
        bir::ExprKind::Block { scope } => scope_(sess, builder, sess.bir.scope(scope)),
        bir::ExprKind::Return { expr } => todo!(),
        bir::ExprKind::Break { label } => todo!(),
        bir::ExprKind::Continue { label } => todo!(),
        bir::ExprKind::Branch {
            condition,
            kind,
            left,
            right,
        } => todo!(),
        bir::ExprKind::Loop { kind, body } => todo!(),
    };
    if let Some(cast_ty) = dbg!(sess.sema.cast_ty(sema)) {
        builder.new_cast(val).sema(cast_ty).build()
    } else {
        val
    }
}

fn op_expr(
    sess: &Session,
    builder: &mut Builder,
    sema: sema::ID,
    lval: Option<ValueRef>,
    op: &bir::Op,
) -> ValueRef {
    match &op.kind {
        bir::OpKind::Plus => {
            let lhs = rvalue(sess, builder, None, sess.bir.expr(&op.operands[0]));
            let rhs = rvalue(sess, builder, None, sess.bir.expr(&op.operands[1]));
            builder.new_add(lhs, rhs).with_maybe_lval(lval)
        }
        bir::OpKind::Assignment => {
            let to = lvalue(sess, builder, sess.bir.expr(&op.operands[0]));
            let val = rvalue(sess, builder, Some(to), sess.bir.expr(&op.operands[1]));
            if to.is_var(builder.ctx()){
                return val;
            }
            builder.new_store(to, val)
        }
        bir::OpKind::Minus => todo!(),
        bir::OpKind::Multiply => todo!(),
        bir::OpKind::Divide => todo!(),
        bir::OpKind::FieldAccess => todo!(),
        bir::OpKind::LessThan => todo!(),
        bir::OpKind::LessThanEquals => todo!(),
        bir::OpKind::GreaterThan => todo!(),
        bir::OpKind::GreaterThanEquals => todo!(),
        bir::OpKind::Equals => todo!(),
    }
    .sema(sema)
    .build()
}
