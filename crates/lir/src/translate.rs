use crate::builder::Session;
use crate::types::*;
use crate::Builder;

#[derive(Debug, PartialEq, Eq)]
enum ValueCategory {
    LVal,
    RVal,
}

pub fn translate(bir: &bir::Map, sema: &sema::Map) -> Module {
    let mut sess = Session::new(bir, sema);
    let mut module = Module::new();
    let mut builder = Builder::new(&mut sess, &mut module);

    map_sema_tys_to_lir_tys(&mut builder);

    let get_full_name = |f: &bir::Function| {
        let mut parts = Vec::new();
        let mut parent = Some(f.mod_);
        while let Some(mod_) = parent {
            let mod_ = builder.sess.bir.mod_(&mod_);
            if let Some(ref name) = mod_.name {
                parts.push(name.clone());
            }
            parent = mod_.parent;
        }
        parts.reverse();
        parts.push(f.identifier.clone());
        parts.join("$")
    };

    for mod_ in bir.modules() {
        for bir_f in mod_.functions(builder.sess.bir) {
            let sema = builder.sess.bir_to_sema(&bir_f.id);
            let sema_fn = builder.sess.sema.fn_(sema).unwrap();

            let return_ty = builder.sess.sema_to_ty(&sema_fn.return_ty);
            let mut param_names = Vec::new();
            let mut param_tys = Vec::new();
            for param in bir_f.parameters.iter() {
                let sema = builder.sess.bir_to_sema(param);
                param_tys.push(builder.sess.ty_from_bir(param));
                param_names
                    .push(builder.sess.sema.name(sema).unwrap().ident.clone());
            }
            let is_var_args = sema_fn.is_var_args(builder.sess.sema);
            let val = builder.new_function(
                &get_full_name(bir_f),
                &param_names,
                return_ty,
                param_tys,
                is_var_args,
            );
            builder.sess.value_mapping.insert(sema, val);
            let f = builder.module.fn_(&val.id);
            for (idx, param) in bir_f.parameters.iter().enumerate() {
                let param = builder.sess.bir_to_sema(param);
                builder
                    .sess
                    .value_mapping
                    .insert(param, f.nth_param(idx).val);
            }
        }
    }

    for mod_ in bir.modules() {
        for f in mod_.functions(builder.sess.bir) {
            fn_(&mut builder, f);
        }
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
            sema::TypeKind::Aggregate(struct_ty) => todo!(),
            sema::TypeKind::Function(fn_ty) => {
                let return_ty =
                    map_ty(fn_ty.return_ty(builder.sess.sema), builder);
                let params: Vec<_> = fn_ty
                    .param_tys(builder.sess.sema)
                    .map(|param| map_ty(param, builder))
                    .collect();
                builder.module.types.get_fn(&return_ty, &params)
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

fn fn_(builder: &mut Builder, f: &bir::Function) {
    builder.enter_function(f.id);
    if let Some(body) = f.body(builder.sess.bir) {
        let ty = builder.sess.ty_from_bir(&body.id);
        builder.new_labeled_block(".entry");
        let lval = if ty.get(builder.ctx()).is_void() {
            None
        } else {
            Some(builder.new_var().named(".ret").ty(ty).create_lval().build())
        };
        scope_(builder, lval, body);
        let ret_val = lval.dup().unwrap_or(builder.void_());
        builder.new_return(ret_val).ty(ty).build();
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
            .ty(ty)
            .create_lval()
            .named(let_.name.clone())
            .build();
        builder.sess.value_mapping.insert(sema, var);
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
                return builder.new_copy(val).with_lval(lval).ty(ty).build();
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
                    .ty(ty)
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
            let mut call = builder.new_call(called_fn, ops).ty(ty);
            if call_has_lval {
                call = call.lval_or_create(lval);
            }
            call.build()
        }
        bir::ExprKind::Index { receiver, index } => {
            let offset = rvalue(builder, None, builder.sess.bir.expr(index));
            let base = lvalue(builder, builder.sess.bir.expr(receiver));
            let offset = builder.new_offset(base, offset).ty(ty);
            match cat {
                ValueCategory::LVal => offset.lval_or_create(lval).build(),
                ValueCategory::RVal => {
                    let offset = offset.create_lval().build();
                    builder.new_load(offset).ty(ty).lval_or_create(lval).build()
                }
            }
        }
        bir::ExprKind::Op(op) => match &op.kind {
            bir::OpKind::Assignment => assign_expr(builder, op, ty),
            bir::OpKind::FieldAccess => {
                field_access_expr(builder, cat, lval, op, ty)
            }
            _ => op_expr(builder, ty, lval, op),
        },
        bir::ExprKind::Block { scope } => {
            scope_(builder, lval, builder.sess.bir.block(scope))
        }
        bir::ExprKind::Cast { val, .. } => {
            let val = rvalue(builder, None, builder.sess.bir.expr(val));
            builder.new_cast(val).ty(ty).lval_or_create(lval).build()
        }
        bir::ExprKind::Return { expr } => {
            let ret = if let Some(expr) = expr {
                let ty = builder.sess.ty_from_bir(expr);
                let val = rvalue(builder, None, builder.sess.bir.expr(expr));
                builder.new_return(val).ty(ty).build()
            } else {
                builder.new_return(builder.void_()).ty(ty).build()
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
    builder.sess.value_mapping.insert(sema, val);
    val
}

fn field_access_expr(
    builder: &mut Builder,
    cat: ValueCategory,
    lval: Option<ValueRef>,
    op: &bir::Op,
    ty: TyID,
) -> ValueRef {
    let base = rvalue(builder, None, builder.sess.bir.expr(&op.operands[0]));
    let rhs = {
        let sema = builder.sess.bir_to_sema(&op.operands[1]);
        let offset =
            builder.sess.sema.ty_member(sema).offset(builder.sess.sema);
        let ty = builder.sess.sema_to_ty(&sema);
        builder.new_int_constant(offset, ty)
    };
    let addr = builder.new_subscript(base, &[rhs]).ty(ty).build();
    match cat {
        ValueCategory::LVal => addr,
        ValueCategory::RVal => {
            let ty = {
                let base_ty = base.ty(builder.ctx()).id;
                builder.ctx_mut().types().get_pointer_to(&base_ty)
            };
            builder.new_load(addr).ty(ty).lval_or_create(lval).build()
        }
    }
}

fn assign_expr(builder: &mut Builder, op: &bir::Op, ty: TyID) -> ValueRef {
    let to = lvalue(builder, builder.sess.bir.expr(&op.operands[0]));
    rvalue(builder, Some(to), builder.sess.bir.expr(&op.operands[1]));
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
    op: &bir::Op,
) -> ValueRef {
    let lhs = rvalue(builder, None, builder.sess.bir.expr(&op.operands[0]));
    let rhs = rvalue(builder, None, builder.sess.bir.expr(&op.operands[1]));
    match &op.kind {
        bir::OpKind::Plus => builder.new_add(lhs, rhs),
        bir::OpKind::Minus => builder.new_sub(lhs, rhs),
        bir::OpKind::Multiply => builder.new_mul(lhs, rhs),
        bir::OpKind::Divide => builder.new_div(lhs, rhs),
        bir::OpKind::LessThan => builder.new_cmp(CmpKind::Lt, lhs, rhs),
        bir::OpKind::LessThanEquals => builder.new_cmp(CmpKind::Lte, lhs, rhs),
        bir::OpKind::GreaterThan => builder.new_cmp(CmpKind::Gt, lhs, rhs),
        bir::OpKind::GreaterThanEquals => {
            builder.new_cmp(CmpKind::Gte, lhs, rhs)
        }
        bir::OpKind::Equals => builder.new_cmp(CmpKind::Eq, lhs, rhs),
        bir::OpKind::Assignment | bir::OpKind::FieldAccess => unreachable!(),
        bir::OpKind::ScopeAccess => todo!(),
    }
    .ty(ty)
    .lval_or_create(lval)
    .build()
}
