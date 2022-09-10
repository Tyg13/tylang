use ast::{Node, Token};
use std::sync::Arc;

use crate::build::*;
use crate::id::*;
use crate::types::*;

pub fn ast(module: &Arc<ast::Module>) -> crate::Map {
    let mut builder = Builder::new();

    let module_ = builder.new_module();
    builder.set_ast(module_, module.clone());
    builder.set_current_module(module_);

    for typedef in module.items().filter_map(|item| item.type_()) {
        typedef_(&mut builder, typedef);
    }

    for fn_ in module.items().filter_map(|item| item.fn_()) {
        let identifier = fn_.name().unwrap().text();

        let return_type = if let Some(ty) = fn_.type_() {
            typeref_(&mut builder, &ty)
        } else {
            builder.new_typeref(TypeRefKind::Void)
        };

        let fn_id = builder.new_function(&identifier, return_type);
        builder.set_ast(fn_id, fn_.clone());
        builder.set_current_function(fn_id);

        for param in fn_.param_list().unwrap().params() {
            match param.as_ref() {
                ast::Param::NamedParam(param) => {
                    let name = param.name().unwrap().text();
                    let ty = typeref_(&mut builder, &param.type_().unwrap());
                    builder.new_param(name, ty);
                }
                ast::Param::VaParam(_) => {
                    builder.new_va_param();
                }
            }
        }

        if let Some(body) = fn_.block() {
            let id = block_(
                &mut builder,
                ScopeKind::Function,
                &format!("{identifier}.body"),
                &body,
            );
            builder.current_function().body = Some(id);
        }
    }

    builder.finish()
}

fn typedef_(builder: &mut Builder, typedef: Arc<ast::TypeItem>) -> ID {
    let identifier = typedef.ident().unwrap();
    let members = typedef
        .members()
        .map(|member| TypeMember {
            identifier: member.ident().unwrap().text().to_string(),
            ty: typeref_(builder, &member.type_().unwrap()),
        })
        .collect();
    let id = builder.new_typedef(identifier.text(), members);
    builder.set_ast(id, typedef.clone());
    id
}

fn typeref_(builder: &mut Builder, ty: &Arc<ast::Type>) -> ID {
    let kind = match ty.as_ref() {
        ast::Type::BasicType(ty) => match ty.ident().unwrap().text() {
            "void" => TypeRefKind::Void,
            name => TypeRefKind::Named {
                name: name.to_string(),
            },
        },
        ast::Type::PointerType(ty) => TypeRefKind::Pointer {
            pointee: typeref_(builder, &ty.pointee().unwrap()),
        },
    };
    let id = builder.new_typeref(kind);
    builder.set_ast(id, ty.clone());
    id
}

fn block_(builder: &mut Builder, kind: ScopeKind, label: &str, block: &Arc<ast::Block>) -> ID {
    let id = builder.in_new_scope(label.to_string(), kind, |builder| {
        for item in block.items() {
            item_(builder, &item);
        }
        if let Some(expr) = block.expr() {
            builder.current_scope().return_expr = Some(expr_(builder, &expr));
        }
    });
    builder.set_ast(id, block.clone());
    id
}

fn item_(builder: &mut Builder, item: &Arc<ast::Item>) {
    let id = match item.as_ref() {
        ast::Item::Let(item) => let_(builder, item),
        ast::Item::ExprItem(expr) => expr_item(builder, expr),
        _ => todo!(),
    };
    builder.set_ast(id, item.clone());
}

fn let_(builder: &mut Builder, item: &Arc<ast::Let>) -> ID {
    let name = item.name().unwrap().text().to_string();
    let ty = item.type_().map(|ty| typeref_(builder, &ty));
    let expr = item.expr().map(|ex| expr_(builder, &ex));
    let id = builder.new_let_item(name, ty, expr);
    builder.set_ast(id, item.clone());
    id
}

fn expr_item(builder: &mut Builder, expr: &Arc<ast::ExprItem>) -> ID {
    let expr_id = expr_(builder, &expr.expr().unwrap());
    let item_id = builder.new_item(ItemKind::Expr(expr_id));
    builder.set_ast(item_id, expr.clone());
    item_id
}

fn expr_(builder: &mut Builder, expr: &Arc<ast::Expr>) -> ID {
    let kind = match expr.as_ref() {
        ast::Expr::Group(expr) => return group_expr(builder, &expr),
        ast::Expr::Literal(lit) => literal_expr(builder, &lit),
        ast::Expr::NameRef(name) => name_ref(builder, &name),
        ast::Expr::PrefixExpr(expr) => prefix_expr(builder, &expr),
        ast::Expr::BinExpr(expr) => binary_expr(builder, &expr),
        ast::Expr::Block(expr) => block_expr(builder, &expr),
        ast::Expr::CallExpr(expr) => call_expr(builder, &expr),
        ast::Expr::Return(expr) => return_expr(builder, &expr),
        ast::Expr::IndexExpr(expr) => index_expr(builder, &expr),
        ast::Expr::IfExpr(expr) => if_expr(builder, &expr),
        ast::Expr::LoopExpr(expr) => loop_expr(builder, &expr),
        ast::Expr::WhileExpr(expr) => while_expr(builder, &expr),
        ast::Expr::Break(expr) => break_expr(builder, &expr),
        ast::Expr::Continue(expr) => continue_expr(builder, &expr),
    };
    let id = builder.new_expr(kind);
    builder.set_ast(id, expr.clone());
    id
}

fn literal_expr(builder: &mut Builder, lit: &Arc<ast::Literal>) -> ExprKind {
    let id = literal(builder, lit);
    builder.set_ast(id, lit.clone());
    ExprKind::Literal(id)
}

fn name_ref(_: &mut Builder, ref_: &Arc<ast::NameRef>) -> ExprKind {
    ExprKind::NameRef {
        name: ref_.name().unwrap().text().to_string(),
    }
}

fn prefix_expr(builder: &mut Builder, expr: &Arc<ast::PrefixExpr>) -> ExprKind {
    let kind = match expr.op().unwrap().text() {
        "+" => OpKind::Plus,
        "-" => OpKind::Minus,
        _ => unreachable!(),
    };
    let operand = expr_(builder, &expr.operand().unwrap());
    ExprKind::Op(Op {
        fixity: OpFixity::Prefix,
        kind,
        operands: vec![operand],
    })
}

fn binary_expr(builder: &mut Builder, expr: &Arc<ast::BinExpr>) -> ExprKind {
    let kind = match expr.op().unwrap().text() {
        "+" => OpKind::Plus,
        "-" => OpKind::Minus,
        "*" => OpKind::Multiply,
        "/" => OpKind::Divide,
        "." => OpKind::FieldAccess,
        "==" => OpKind::Equals,
        ">" => OpKind::GreaterThan,
        ">=" => OpKind::GreaterThanEquals,
        "<" => OpKind::LessThan,
        "<=" => OpKind::LessThanEquals,
        "=" => OpKind::Assignment,
        kind => panic!("unrecognized op: {kind}"),
    };
    let lhs = expr_(builder, &expr.lhs().unwrap());
    let rhs = expr_(builder, &expr.rhs().unwrap());
    ExprKind::Op(Op {
        fixity: OpFixity::Infix,
        kind,
        operands: vec![lhs, rhs],
    })
}

fn group_expr(builder: &mut Builder, group: &Arc<ast::Group>) -> ID {
    // Just return inner expression
    expr_(builder, &group.inner().unwrap())
}

fn block_expr(builder: &mut Builder, block: &Arc<ast::Block>) -> ExprKind {
    ExprKind::Block {
        scope: block_(builder, ScopeKind::Block, "block", block),
    }
}

fn call_expr(builder: &mut Builder, expr: &Arc<ast::CallExpr>) -> ExprKind {
    let receiver = expr_(builder, &expr.receiver().unwrap());
    let mut operands = Vec::new();
    for arg in expr.arguments().by_ref() {
        operands.push(expr_(builder, &arg));
    }
    ExprKind::Call { receiver, operands }
}

fn return_expr(builder: &mut Builder, ret: &Arc<ast::Return>) -> ExprKind {
    ExprKind::Return {
        expr: expr_(builder, &ret.expr().unwrap()),
    }
}

fn index_expr(builder: &mut Builder, expr: &Arc<ast::IndexExpr>) -> ExprKind {
    ExprKind::Index {
        receiver: expr_(builder, &expr.receiver().unwrap()),
        index: expr_(builder, &expr.index().unwrap()),
    }
}

fn if_expr(builder: &mut Builder, expr: &Arc<ast::IfExpr>) -> ExprKind {
    let condition = expr_(builder, &expr.condition().unwrap());
    let left = block_(builder, ScopeKind::Block, "then", &expr.then().unwrap());
    let (kind, right) = if let Some(alt) = expr.alternate() {
        let right = block_(builder, ScopeKind::Block, "else", &alt);
        (BranchKind::IfElse, right)
    } else {
        (BranchKind::If, NONE)
    };
    ExprKind::Branch {
        condition,
        kind,
        left,
        right,
    }
}

fn loop_expr(builder: &mut Builder, loop_: &Arc<ast::LoopExpr>) -> ExprKind {
    ExprKind::Loop {
        kind: LoopKind::Loop,
        body: block_(builder, ScopeKind::Block, "loop", &loop_.body().unwrap()),
    }
}

fn while_expr(builder: &mut Builder, while_: &Arc<ast::WhileExpr>) -> ExprKind {
    ExprKind::Loop {
        kind: LoopKind::While,
        body: builder.in_new_scope("while".to_string(), ScopeKind::Block, |builder| {
            let condition = expr_(builder, &while_.condition().unwrap());
            let body = block_(builder, ScopeKind::Loop, "body", &while_.body().unwrap());
            let exit_block =
                builder.in_new_scope("exit".to_string(), ScopeKind::Block, |builder| {
                    builder.new_expr_item(ExprKind::Break {
                        label: builder.last_loop_label(),
                    });
                });
            let id = builder.new_expr_item(ExprKind::Branch {
                condition,
                kind: BranchKind::IfElse,
                left: body,
                right: exit_block,
            });
            builder.set_ast(id, while_.clone());
        }),
    }
}

fn break_expr(builder: &mut Builder, break_: &Arc<ast::Break>) -> ExprKind {
    ExprKind::Break {
        label: builder.last_loop_label(),
    }
}

fn continue_expr(builder: &mut Builder, continue_: &Arc<ast::Continue>) -> ExprKind {
    ExprKind::Continue {
        label: builder.last_loop_label(),
    }
}

fn literal(builder: &mut Builder, lit: &Arc<ast::Literal>) -> ID {
    let id = builder.new_literal(match lit.value().unwrap() {
        ast::LiteralValue::Number(n) => Literal::Number(n.text().parse().unwrap()),
        ast::LiteralValue::Str(s) => Literal::Str(s.text().to_string()),
    });
    builder.set_ast(id, lit.clone());
    id
}
