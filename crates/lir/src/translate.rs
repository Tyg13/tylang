use std::collections::HashMap;

use crate::builder::Session;
use crate::types::*;
use crate::Builder;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ValueCategory {
    LVal,
    RVal,
}

pub fn translate(bir: &bir::Map, sema: &sema::Map) -> Module {
    let mut sess = Session::new(bir, sema);
    let mut module = Module::new();
    let mut builder = Builder::new(&mut sess, &mut module);

    map_sema_tys_to_lir_tys(&mut builder);

    let mut fns_seen: HashMap<String, ValueID> = HashMap::new();
    let mut fns_with_unprocessed_bodies = Vec::new();

    for mod_ in bir.modules() {
        for bir_f in mod_.functions(builder.sess.bir) {
            let sema = builder.sess.bir_to_sema(&bir_f.id);
            let sema_fn = builder.sess.sema.fn_(sema).unwrap();

            let full_name = bir_f.full_name(builder.sess.bir);
            if let Some(val) = fns_seen.get(&full_name) {
                builder.sess.value_mapping.insert(sema, *val);
                continue;
            }

            let fn_ty = sema_fn.ty(builder.sess.sema);
            let return_ty = builder.sess.sema_to_ty(&fn_ty.return_ty);
            let mut param_names = Vec::new();
            let mut param_tys = Vec::new();
            for param in bir_f.parameters.iter() {
                let sema = builder.sess.bir_to_sema(param);
                param_tys.push(builder.sess.ty_from_bir(param));
                param_names
                    .push(builder.sess.sema.name(sema).unwrap().ident.clone());
            }
            let internal =
                builder.sess.sema.num_callers(sema) > 0 && bir_f.body.is_some();
            let val = builder.new_function(
                full_name.clone(),
                param_names,
                return_ty,
                param_tys,
                fn_ty.is_var_args,
                internal,
            );
            builder.sess.value_mapping.insert(sema, val);
            fns_seen.insert(full_name, val);

            let f = builder.module.fn_(&val);
            for (idx, param) in bir_f.parameters.iter().enumerate() {
                let param = builder.sess.bir_to_sema(param);
                builder
                    .sess
                    .value_mapping
                    .insert(param, f.nth_param(idx).val);
            }
            if bir_f.body.is_some() {
                fns_with_unprocessed_bodies.push(bir_f.id);
            }
        }
    }

    for id in fns_with_unprocessed_bodies {
        fn_body(&mut builder, bir.fn_(&id));
    }

    module
}

fn map_sema_tys_to_lir_tys(builder: &mut Builder) {
    fn map_ty(ty: &sema::Type, builder: &mut Builder) -> TyID {
        if let Some(id) = builder.sess.ty_mapping.try_get(&ty.id) {
            return id;
        }
        let id = match &ty.kind {
            sema::TypeKind::Never | sema::TypeKind::Void => {
                builder.module.types.get_void()
            }
            sema::TypeKind::String => builder.module.types.get_str(),
            sema::TypeKind::Integer { size } => {
                builder.module.types.get_int(*size)
            }
            sema::TypeKind::Pointer { pointee } => {
                let pointee =
                    map_ty(builder.sess.sema.ty(*pointee).unwrap(), builder);
                builder.module.types.get_pointer_to(&pointee)
            }
            sema::TypeKind::Aggregate(struct_ty) => {
                let members: Vec<_> = struct_ty
                    .members(builder.sess.sema)
                    .map(|ty| map_ty(ty, builder))
                    .collect();
                builder.module.types.get_struct(
                    &struct_ty.name(builder.sess.sema).unwrap().ident,
                    members.as_slice(),
                )
            }
            sema::TypeKind::Function(fn_ty) => {
                let return_ty = map_ty(
                    fn_ty.return_ty(builder.sess.sema).unwrap(),
                    builder,
                );
                let params: Vec<_> = fn_ty
                    .param_tys(builder.sess.sema)
                    .map(|param| map_ty(param, builder))
                    .collect();
                builder.module.types.get_fn(
                    fn_ty.is_var_args,
                    &return_ty,
                    &params,
                )
            }
            sema::TypeKind::Prototype | sema::TypeKind::Marker => {
                unreachable!()
            }
        };
        builder.sess.ty_mapping.insert(ty.id, id);
        id
    }

    for ty in builder.sess.sema.types() {
        map_ty(ty, builder);
    }
}

fn fn_body(builder: &mut Builder, f: &bir::Function) {
    builder.enter_function(f.id);
    if let Some(body) = f.body(builder.sess.bir) {
        let ty = builder.sess.ty_from_bir(&body.id);
        builder.new_labeled_block(".entry");
        let lval = ty.get(builder.ctx()).has_lval().then(|| {
            builder
                .new_var()
                .named(".ret")
                .of_ty(ty)
                .with_new_lval()
                .build()
        });
        scope_(builder, lval, body);
        let ret_val = lval.dup().unwrap_or(builder.void_());
        builder.new_return(ret_val).of_ty(ty).build();
    }
    builder.exit_function(f.id);
}

// separate block and scope: some scopes should receive a new block
// (e.g. `if`, `else` blocks), whereas others (free-standing blocks,
// expression blocks) should not.
fn block(
    builder: &mut Builder,
    lval: Option<ValueRef>,
    scope: &bir::Block,
) -> (Block, ValueRef) {
    let block = if let Some(label) = &scope.label {
        builder.new_labeled_block(&label)
    } else {
        builder.new_block()
    };
    let expr = scope_(builder, lval, scope);

    (block, expr)
}

fn scope_(
    builder: &mut Builder,
    lval: Option<ValueRef>,
    scope: &bir::Block,
) -> ValueRef {
    for let_ in scope.lets(builder.sess.bir) {
        let sema = builder.sess.bir_to_sema(&let_.id);
        let ty = builder.sess.sema_to_ty(&sema);
        let var = builder
            .new_var()
            .of_ty(ty)
            .with_new_lval()
            .named(&let_.ident)
            .build();
        builder.sess.value_mapping.insert(sema, var.id);
    }

    for it in scope.items(builder.sess.bir) {
        item(builder, it);
    }

    let ret_val = if let Some(e) = scope.return_expr(builder.sess.bir) {
        rvalue(builder, lval, e)
    } else {
        debug_assert_eq!(lval, None);
        debug_assert_eq!(
            builder.sess.ty_from_bir(&scope.id).get(builder.ctx()).kind,
            TyKind::Void
        );
        builder.void_()
    };

    ret_val
}

fn item(builder: &mut Builder, it: &bir::Item) {
    match &it.kind {
        bir::ItemKind::Let(id) => {
            let let_ = builder.sess.bir.let_(id);
            let expr = let_.expr(builder.sess.bir).unwrap();
            let lval = builder.sess.val_from_bir(id);
            rvalue(builder, Some(lval), expr);
        }
        bir::ItemKind::Expr(id) => {
            value(
                builder,
                ValueCategory::RVal,
                None,
                builder.sess.bir.expr(id),
            );
        }
    }
}

fn literal(builder: &mut Builder, id: sema::ID) -> ValueRef {
    match builder.sess.sema.constant(id).unwrap() {
        sema::Constant::Int(v) => {
            builder.new_int_constant(*v, builder.sess.sema_to_ty(&id))
        }
        sema::Constant::Str(s) => builder.new_str_constant(s),
        sema::Constant::Struct => todo!(),
    }
}

fn lvalue(builder: &mut Builder, e: &bir::Expr) -> ValueRef {
    value(builder, ValueCategory::LVal, None, e)
}

fn rvalue(
    builder: &mut Builder,
    lval: Option<ValueRef>,
    e: &bir::Expr,
) -> ValueRef {
    value(builder, ValueCategory::RVal, lval, e)
}

fn value(
    builder: &mut Builder,
    cat: ValueCategory,
    lval: Option<ValueRef>,
    e: &bir::Expr,
) -> ValueRef {
    let sema = builder.sess.bir_to_sema(&e.id);
    let ty = builder.sess.sema_to_ty(&sema);
    let val = match &e.kind {
        bir::ExprKind::Literal(..) => {
            debug_assert_ne!(cat, ValueCategory::LVal);
            let val = literal(builder, sema);
            if let Some(lval) = lval {
                return builder.new_copy(val).with_lval(lval).of_ty(ty).build();
            }
            val
        }
        bir::ExprKind::NameRef { .. } => {
            let named_val = builder.sess.val_from_sema(&sema);
            if let Some(lval) = lval {
                debug_assert_eq!(cat, ValueCategory::RVal);
                return builder
                    .new_copy(named_val)
                    .with_lval(lval)
                    .of_ty(ty)
                    .build();
            }
            named_val
        }
        bir::ExprKind::Call { receiver, operands } => {
            let called_fn = builder.sess.val_from_bir(receiver);
            let ops: Vec<_> = operands
                .iter()
                .map(|op| rvalue(builder, None, builder.sess.bir.expr(op)))
                .collect();
            let call_has_lval = !called_fn
                .ty(builder.ctx())
                .as_fn_ty()
                .return_ty(builder.ctx())
                .is_void();
            let mut call = builder.new_call(called_fn, ops).of_ty(ty);
            if call_has_lval {
                call = call.with_lval_or_new(lval);
            }
            call.build()
        }
        bir::ExprKind::Index { receiver, index } => {
            let base = rvalue(builder, None, builder.sess.bir.expr(receiver));
            let offset = rvalue(builder, None, builder.sess.bir.expr(index));
            let subscr = builder.new_subscript(base, &[offset]).of_ty(ty);
            match cat {
                ValueCategory::LVal => subscr.with_new_lval().build(),
                ValueCategory::RVal => {
                    let subscr = subscr.with_new_lval().build();
                    builder
                        .new_load(subscr)
                        .of_ty(ty)
                        .with_lval_or_new(lval)
                        .build()
                }
            }
        }
        bir::ExprKind::Op(op) => op_expr(builder, ty, lval, cat, op),
        bir::ExprKind::Block { scope } => {
            scope_(builder, lval, builder.sess.bir.block(scope))
        }
        bir::ExprKind::Cast { val, .. } => {
            let val = rvalue(builder, None, builder.sess.bir.expr(val));
            builder
                .new_cast(val)
                .of_ty(ty)
                .with_lval_or_new(lval)
                .build()
        }
        bir::ExprKind::Return { expr } => {
            let ret = if let Some(expr) = expr {
                let ty = builder.sess.ty_from_bir(expr);
                let val = rvalue(builder, None, builder.sess.bir.expr(expr));
                builder.new_return(val).of_ty(ty).build()
            } else {
                builder.new_return(builder.void_()).of_ty(ty).build()
            };
            builder.new_block();
            ret
        }
        bir::ExprKind::Break { label } => {
            let brk = builder.new_break(label.clone());
            builder.new_block();
            brk
        }
        bir::ExprKind::Continue { label } => {
            let latch = builder.block_from_label(label);
            let jmp = builder.new_jump(latch);
            builder.new_block();
            jmp
        }
        bir::ExprKind::Branch {
            condition,
            kind,
            left,
            right,
        } => branch_expr(builder, lval, condition, kind, left, right),
        bir::ExprKind::Loop { body, .. } => {
            let jmp_to_body = builder.new_jump_marker();
            let scope = builder.sess.bir.block(body);
            let (body, _) = block(builder, None, scope);
            builder.resolve_jump(jmp_to_body, body);

            let jmp_to_latch = builder.new_jump(body);
            let after = builder.new_block();
            builder.resolve_breaks(scope.label.as_ref().unwrap(), after);
            jmp_to_latch
        }
    };
    builder.sess.value_mapping.insert(sema, val.id);
    val
}

fn field_access_expr(
    builder: &mut Builder,
    cat: ValueCategory,
    lval: Option<ValueRef>,
    receiver: &bir::Expr,
    field: &bir::Expr,
    ty: TyID,
) -> ValueRef {
    let base = rvalue(builder, None, receiver);
    let rhs = {
        let sema = builder.sess.bir_to_sema(&field.id);
        let offset =
            builder.sess.sema.ty_member(sema).offset(builder.sess.sema);
        let ty = builder.sess.sema_to_ty(&sema);
        builder.new_int_constant(offset, ty)
    };
    let addr = builder
        .new_get_field(base, &[rhs])
        .of_ty(ty)
        .with_new_lval()
        .build();
    match cat {
        ValueCategory::LVal => addr,
        ValueCategory::RVal => {
            let ty = {
                let base_ty = base.ty(builder.ctx()).id;
                builder.ctx_mut().types().get_pointer_to(&base_ty)
            };
            builder
                .new_load(addr)
                .of_ty(ty)
                .with_lval_or_new(lval)
                .build()
        }
    }
}

fn assign_expr(
    builder: &mut Builder,
    dst: &bir::Expr,
    src: &bir::Expr,
) -> ValueRef {
    let to = lvalue(builder, dst);
    rvalue(builder, Some(to), src);
    to.dup()
}

fn branch_expr(
    builder: &mut Builder,
    lval: Option<ValueRef>,
    condition: &bir::ID,
    kind: &bir::BranchKind,
    left: &bir::ID,
    right: &Option<bir::ID>,
) -> ValueRef {
    let cond = rvalue(builder, None, builder.sess.bir.expr(condition));
    let branch_marker = builder.new_branch_marker();
    match kind {
        bir::BranchKind::If => {
            let (left_block, _) =
                block(builder, lval, builder.sess.bir.block(left));
            let jmp_to_exit = builder.new_jump_marker();
            let exit = builder.new_block();
            builder.resolve_jump(jmp_to_exit, exit);
            builder.resolve_branch(branch_marker, cond, left_block, exit);
        }
        bir::BranchKind::IfElse => {
            let (left_block, _) =
                block(builder, lval, builder.sess.bir.block(left));
            let left_jmp_to_exit = builder.new_jump_marker();
            let (right_block, _) =
                block(builder, lval, builder.sess.bir.block(&right.unwrap()));
            let right_jmp_to_exit = builder.new_jump_marker();
            let exit = builder.new_block();
            builder.resolve_jump(left_jmp_to_exit, exit);
            builder.resolve_jump(right_jmp_to_exit, exit);
            builder.resolve_branch(
                branch_marker,
                cond,
                left_block,
                right_block,
            );
        }
    };
    lval.unwrap_or(builder.void_())
}

fn op_expr(
    builder: &mut Builder,
    ty: TyID,
    lval: Option<ValueRef>,
    cat: ValueCategory,
    op: &bir::Op,
) -> ValueRef {
    let bir::Op::Binary { kind, lhs, rhs } = op else {
        todo!()
    };
    use bir::BinaryOpKind as Kind;
    let lhs = &builder.sess.bir.expr(lhs);
    let rhs = &builder.sess.bir.expr(rhs);
    match kind {
        Kind::Assign => {
            assert_ne!(cat, ValueCategory::LVal);
            return assign_expr(builder, lhs, rhs);
        }
        Kind::DotAccess => {
            return field_access_expr(builder, cat, lval, lhs, rhs, ty);
        }
        _ => {}
    };
    let lhs = rvalue(builder, None, lhs);
    let rhs = rvalue(builder, None, rhs);
    match kind {
        Kind::Add => builder.new_add(lhs, rhs),
        Kind::Sub => builder.new_sub(lhs, rhs),
        Kind::Mul => builder.new_mul(lhs, rhs),
        Kind::Div => builder.new_div(lhs, rhs),
        Kind::LessThan => builder.new_cmp(CmpKind::Lt, lhs, rhs),
        Kind::LessThanEquals => builder.new_cmp(CmpKind::Lte, lhs, rhs),
        Kind::GreaterThan => builder.new_cmp(CmpKind::Gt, lhs, rhs),
        Kind::GreaterThanEquals => builder.new_cmp(CmpKind::Gte, lhs, rhs),
        Kind::Equals => builder.new_cmp(CmpKind::Eq, lhs, rhs),
        Kind::NotEquals => builder.new_cmp(CmpKind::Ne, lhs, rhs),
        Kind::DotAccess | Kind::Assign => unreachable!(),
        Kind::ArrowAccess => todo!(),
    }
    .of_ty(ty)
    .with_lval_or_new(lval)
    .build()
}
