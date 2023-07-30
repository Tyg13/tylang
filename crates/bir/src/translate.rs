use ast::{Node, Token};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use crate::build::*;
use crate::types::*;

pub trait AstBuilder {
    type Error: std::fmt::Debug;
    fn build(
        &mut self,
        module_name: &str,
    ) -> Result<Arc<ast::Module>, Self::Error>;
}

struct AstCacher<'builder, Builder: AstBuilder> {
    name_to_ast: HashMap<String, Arc<ast::Module>>,
    ast_builder: &'builder mut Builder,
}

enum CacheResult<E: std::fmt::Debug> {
    Resolved(Arc<ast::Module>),
    NotFound(E),
}

impl<E: std::fmt::Debug> CacheResult<E> {
    fn unwrap(self) -> Arc<ast::Module> {
        match self {
            Self::Resolved(a) => a,
            Self::NotFound(e) => panic!("Module not found: '{e:#?}'"),
        }
    }
}

impl<'b, Builder: AstBuilder> AstCacher<'b, Builder> {
    fn new(ast_builder: &'b mut Builder) -> Self {
        AstCacher {
            name_to_ast: Default::default(),
            ast_builder,
        }
    }

    fn get(&mut self, name: &str) -> CacheResult<Builder::Error> {
        if let Some(ast) = self.name_to_ast.get(name) {
            return CacheResult::Resolved(ast.clone());
        }
        let ast = match self.ast_builder.build(name) {
            Ok(ast) => ast,
            Err(e) => return CacheResult::NotFound(e),
        };
        self.name_to_ast.insert(name.to_string(), ast.clone());
        CacheResult::Resolved(ast)
    }
}

pub fn ast(
    root_module: &Arc<ast::Module>,
    ast_builder: &mut impl AstBuilder,
) -> crate::Map {
    let mut builder = Builder::new();
    let mut ast_cacher = AstCacher::new(ast_builder);
    build_module_tree(&mut builder, &mut ast_cacher, root_module);
    builder.finish()
}

fn build_module_tree<B: AstBuilder>(
    builder: &mut Builder,
    ast_cacher: &mut AstCacher<B>,
    root: &Arc<ast::Module>,
) {
    struct WorkItem {
        parent: Option<ID>,
        ast: Arc<ast::Module>,
        name: Option<String>,
        imported: bool,
    }
    let mut worklist: Vec<WorkItem> = vec![WorkItem {
        parent: None,
        ast: root.clone(),
        name: None,
        imported: false,
    }];

    while let Some(WorkItem {
        parent,
        ast,
        name,
        imported,
    }) = worklist.pop()
    {
        let module = builder.new_module(Some(ast.clone()));
        builder.set_current_module(module);
        if let Some(parent) = parent {
            builder.add_module_child(parent, module);
        }
        if let Some(name) = name {
            builder.current_module().ident = Some(name);
        }
        if imported {
            builder.current_module().imported = true;
        }
        module_inner(builder, &ast);

        for mod_ in ast.inner_mods() {
            worklist.push(WorkItem {
                parent: Some(module),
                ast: mod_.clone(),
                name: mod_.name().map(|n| n.text().to_string()),
                imported: false,
            });
        }

        for import in ast.imports() {
            let ident = import.ident().unwrap();
            worklist.push(WorkItem {
                parent: Some(module),
                ast: ast_cacher.get(ident.text()).unwrap(),
                name: Some(ident.text().to_string()),
                imported: true,
            });
        }
    }
}

fn module_inner(builder: &mut Builder, mod_: &Arc<ast::Module>) {
    for typedef in mod_.types() {
        typedef_(builder, typedef);
    }

    for fn_ in mod_.fns() {
        let identifier = fn_.name().unwrap().text();

        let return_type = if let Some(ty) = fn_.return_ty() {
            typeref_(builder, &ty)
        } else {
            builder.new_typeref(TypeRefKind::Void, None)
        };

        let fn_id =
            builder.new_function(&identifier, return_type, Some(fn_.clone()));
        builder.set_current_function(fn_id);

        for param in fn_.param_list().unwrap().params() {
            match param.as_ref() {
                ast::Param::NamedParam(param) => {
                    let name = param.name().unwrap().text();
                    let ty = typeref_(builder, &param.type_().unwrap());
                    builder.new_param(name, ty, Some(param.clone()));
                }
                ast::Param::VaParam(_) => {
                    builder.current_function().is_var_args = true;
                }
            }
        }

        if fn_.extern_().is_some() {
            builder.current_function().is_extern = true;
        }

        if let Some(body) = fn_.body() {
            if let Some(block) = body.block() {
                builder.current_function().body =
                    Some(block_(builder, BlockKind::Function, None, &block));
            } else {
                builder.current_function().body = Some(builder.in_new_scope(
                    None,
                    BlockKind::Function,
                    |builder| {
                        builder.current_scope().return_expr =
                            Some(expr_(builder, &body));
                    },
                ));
            }
        }
    }
}

fn typedef_(builder: &mut Builder, typedef: Arc<ast::TypeItem>) -> ID {
    let ident = typedef.ident().unwrap();
    let members = typedef
        .members()
        .map(|member| TypeMember {
            ident: member.ident().unwrap().text().to_string(),
            ty: typeref_(builder, &member.type_().unwrap()),
        })
        .collect();
    builder.new_typedef(ident.text(), members, Some(typedef.clone()))
}

fn name(builder: &mut Builder, name: &Arc<ast::Name>) -> ID {
    let mut segments = Vec::new();
    let mut curr = Some(name.clone());
    while let Some(ref name) = curr {
        match name.kind() {
            ast::SyntaxKind::SCOPED_NAME => {
                let name = name.scoped_name().unwrap();
                segments.push(name.head().unwrap().text().to_string());
                curr = name.tail();
            }
            ast::SyntaxKind::NAME => {
                segments.push(name.text().to_string());
                curr = None;
            }
            _ => unreachable!(),
        }
    }
    builder.new_name(segments, Some(name.clone()))
}

fn typeref_(builder: &mut Builder, ty: &Arc<ast::Type>) -> ID {
    let kind = match ty.as_ref() {
        ast::Type::BasicType(ty) => {
            let n = ty.name().unwrap();
            match n.as_ref() {
                ast::Name::BasicName(n) if n.text() == "void" => {
                    TypeRefKind::Void
                }
                _ => TypeRefKind::Named {
                    name: name(builder, &n),
                },
            }
        }
        ast::Type::PointerType(ty) => TypeRefKind::Pointer {
            pointee: typeref_(builder, &ty.pointee().unwrap()),
        },
    };
    builder.new_typeref(kind, Some(ty.clone()))
}

fn block_(
    builder: &mut Builder,
    kind: BlockKind,
    label: Option<String>,
    block: &Arc<ast::Block>,
) -> ID {
    let id = builder.in_new_scope(label, kind, |builder| {
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

fn item_(builder: &mut Builder, item: &Arc<ast::Item>) -> ID {
    match item.as_ref() {
        ast::Item::Let(item) => let_(builder, item),
        ast::Item::ExprItem(expr) => expr_item(builder, expr),
        _ => todo!(),
    }
}

fn let_(builder: &mut Builder, item: &Arc<ast::Let>) -> ID {
    let name = item.name().unwrap().text().to_string();
    let ty = item.type_().map(|ty| typeref_(builder, &ty));
    let expr = item.expr().map(|ex| expr_(builder, &ex));
    builder.new_let_item(name, ty, expr, Some(item.clone()))
}

fn expr_item(builder: &mut Builder, expr: &Arc<ast::ExprItem>) -> ID {
    let id = expr_(builder, &expr.expr().unwrap());
    builder.new_item(ItemKind::Expr(id), Some(expr.clone()))
}

fn expr_(builder: &mut Builder, expr: &Arc<ast::Expr>) -> ID {
    let kind = match expr.as_ref() {
        ast::Expr::Group(expr) => return group_expr(builder, &expr),
        ast::Expr::Literal(lit) => literal_expr(builder, &lit),
        ast::Expr::StructLiteral(lit) => struct_literal_expr(builder, &lit),
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
        ast::Expr::Cast(expr) => cast_expr(builder, &expr),
    };
    builder.new_expr(kind, Some(expr.clone()))
}

fn literal_expr(builder: &mut Builder, lit: &Arc<ast::Literal>) -> ExprKind {
    ExprKind::Literal(literal(builder, lit))
}

fn struct_literal_expr(
    builder: &mut Builder,
    struct_: &Arc<ast::StructLiteral>,
) -> ExprKind {
    let name = name(builder, &struct_.name().unwrap());
    let lit = Literal::Struct(StructLiteral {
        name,
        members: Vec::new(),
    });
    ExprKind::Literal(builder.new_literal(lit, Some(struct_.clone())))
}

fn name_ref(builder: &mut Builder, ref_: &Arc<ast::NameRef>) -> ExprKind {
    ExprKind::NameRef {
        id: name(builder, &ref_.name().unwrap()),
    }
}

fn prefix_expr(builder: &mut Builder, expr: &Arc<ast::PrefixExpr>) -> ExprKind {
    ExprKind::Op(Op::Prefix {
        kind: match expr.op().unwrap().text() {
            "+" => PrefixOpKind::Plus,
            "-" => PrefixOpKind::Negate,
            "*" => PrefixOpKind::Deref,
            _ => unreachable!(),
        },
        arg: expr_(builder, &expr.operand().unwrap()),
    })
}

fn binary_expr(builder: &mut Builder, expr: &Arc<ast::BinExpr>) -> ExprKind {
    let kind = match expr.op().unwrap().text() {
        "+" => BinaryOpKind::Add,
        "-" => BinaryOpKind::Sub,
        "*" => BinaryOpKind::Mul,
        "." => BinaryOpKind::DotAccess,
        "->" => BinaryOpKind::ArrowAccess,
        "==" => BinaryOpKind::Equals,
        "!=" => BinaryOpKind::NotEquals,
        ">" => BinaryOpKind::GreaterThan,
        ">=" => BinaryOpKind::GreaterThanEquals,
        "<" => BinaryOpKind::LessThan,
        "<=" => BinaryOpKind::LessThanEquals,
        "=" => BinaryOpKind::Assign,
        kind => panic!("unrecognized op: {kind}"),
    };
    let lhs = expr_(builder, &expr.lhs().unwrap());
    let rhs = expr_(builder, &expr.rhs().unwrap());

    // Kind of ugly hack to transform
    //   (*expr).field   -->   (expr->field)
    //
    // This is intended as a kind of canonicalization to make lowering
    // easier (since this means we can always lower Derefs to loads).
    if kind == BinaryOpKind::DotAccess {
        if let Some(op) = builder.map.expr(&lhs).op() {
            if op.is_prefix() && op.prefix_kind() == PrefixOpKind::Deref {
                let new_lhs = op.prefix_arg();
                builder.unlink_node(lhs);
                return ExprKind::Op(Op::Binary {
                    kind: BinaryOpKind::ArrowAccess,
                    lhs: new_lhs,
                    rhs,
                });
            }
        }
    }

    ExprKind::Op(Op::Binary { kind, lhs, rhs })
}

fn group_expr(builder: &mut Builder, group: &Arc<ast::Group>) -> ID {
    // Just return inner expression; no need to handle precedence
    expr_(builder, &group.inner().unwrap())
}

fn block_expr(builder: &mut Builder, block: &Arc<ast::Block>) -> ExprKind {
    ExprKind::Block {
        scope: block_(builder, BlockKind::Expr, None, block),
    }
}

fn call_expr(builder: &mut Builder, expr: &Arc<ast::CallExpr>) -> ExprKind {
    ExprKind::Call {
        receiver: expr_(builder, &expr.receiver().unwrap()),
        operands: expr
            .arguments()
            .by_ref()
            .map(|arg| expr_(builder, &arg))
            .collect(),
    }
}

fn return_expr(builder: &mut Builder, ret: &Arc<ast::Return>) -> ExprKind {
    ExprKind::Return {
        expr: ret.expr().map(|e| expr_(builder, &e)),
    }
}

fn index_expr(builder: &mut Builder, expr: &Arc<ast::IndexExpr>) -> ExprKind {
    ExprKind::Index {
        receiver: expr_(builder, &expr.receiver().unwrap()),
        index: expr_(builder, &expr.index().unwrap()),
    }
}

fn if_expr(builder: &mut Builder, expr: &Arc<ast::IfExpr>) -> ExprKind {
    ExprKind::Branch {
        condition: expr_(builder, &expr.condition().unwrap()),
        kind: if expr.alternate().is_some() {
            BranchKind::IfElse
        } else {
            BranchKind::If
        },
        left: block_(builder, BlockKind::Expr, None, &expr.then().unwrap()),
        right: expr
            .alternate()
            .map(|alt| block_(builder, BlockKind::Expr, None, &alt)),
    }
}

fn loop_expr(builder: &mut Builder, loop_: &Arc<ast::LoopExpr>) -> ExprKind {
    // this is an extremely suspect method to obtain semi-unique loop labels
    let id = loop_.deref() as *const ast::LoopExpr as usize;
    let label = format!("loop{}", (id & 0xFF0000) >> 16);
    ExprKind::Loop {
        kind: LoopKind::Loop,
        body: block_(
            builder,
            BlockKind::Loop,
            Some(label),
            &loop_.body().unwrap(),
        ),
    }
}

fn while_expr(builder: &mut Builder, while_: &Arc<ast::WhileExpr>) -> ExprKind {
    let body = builder.in_new_scope(
        Some("while.latch".to_string()),
        BlockKind::Loop,
        |builder| {
            let condition = expr_(builder, &while_.condition().unwrap());

            let body = block_(
                builder,
                BlockKind::Loop,
                Some("while.body".to_string()),
                &while_.body().unwrap(),
            );

            let exit_block =
                builder.in_new_scope(None, BlockKind::Expr, |builder| {
                    builder.new_expr_item(
                        ExprKind::Break {
                            label: builder.last_loop_label(),
                        },
                        None,
                    );
                });

            builder.new_expr_item(
                ExprKind::Branch {
                    condition,
                    kind: BranchKind::IfElse,
                    left: body,
                    right: Some(exit_block),
                },
                Some(while_.clone()),
            );
        },
    );
    ExprKind::Loop {
        kind: LoopKind::While,
        body,
    }
}

fn break_expr(builder: &mut Builder, _: &Arc<ast::Break>) -> ExprKind {
    ExprKind::Break {
        label: builder.last_loop_label(),
    }
}

fn continue_expr(builder: &mut Builder, _: &Arc<ast::Continue>) -> ExprKind {
    ExprKind::Continue {
        label: builder.last_loop_label(),
    }
}

fn cast_expr(builder: &mut Builder, cast: &Arc<ast::Cast>) -> ExprKind {
    ExprKind::Cast {
        val: expr_(builder, &cast.expr().unwrap()),
        to: typeref_(builder, &cast.ty().unwrap()),
    }
}

fn literal(builder: &mut Builder, lit: &Arc<ast::Literal>) -> ID {
    use utils::string_utils::trim_and_unescape;
    builder.new_literal(
        match lit.value().unwrap() {
            ast::LiteralValue::Number(n) => {
                Literal::Number(n.text().parse().unwrap())
            }
            ast::LiteralValue::Str(s) => {
                Literal::Str(trim_and_unescape(s.text()))
            }
        },
        Some(lit.clone()),
    )
}
