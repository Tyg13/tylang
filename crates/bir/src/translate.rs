use std::rc::Rc;

use crate::types::*;
use ast::{self, Token};

pub fn module(module: &ast::Module) -> Option<Module> {
    let mut functions = Vec::new();
    for func in module.items().filter_map(|item| item.fn_()) {
        functions.push(function(&func)?);
    }
    let mut type_definitions = Vec::new();
    for type_ in module.items().filter_map(|item| item.type_()) {
        type_definitions.push(type_definition(&type_)?);
    }
    Some(Module {
        functions,
        types: type_definitions,
    })
}

fn function(function: &ast::FnDef) -> Option<Function> {
    let identifier = function.name()?.ident()?.text().to_string();
    let mut parameters = Vec::new();
    for p in function.param_list().unwrap().params() {
        parameters.push(param(&p)?);
    }
    let return_type = if let Some(ty) = function.type_() {
        Some(type_(&ty)?)
    } else {
        None
    };
    let body = if let Some(block) = function.block() {
        Some(scope(&block)?)
    } else {
        None
    };
    Some(Function {
        identifier,
        parameters,
        body,
        return_type,
    })
}

fn name_(name: &ast::Name) -> Option<String> {
    Some(name.ident()?.text().to_string())
}

fn param(param: &ast::Param) -> Option<Parameter> {
    match param {
        ast::Param::NamedParam(param) => Some(Parameter::Named {
            name: name_(&param.name()?)?,
            typ_: type_(&param.type_()?)?,
        }),
        ast::Param::VaParam(..) => Some(Parameter::VariableArgs),
    }
}

fn type_(ty: &ast::Type) -> Option<TypeRef> {
    Some(match ty {
        ast::Type::BasicType(ty) => {
            let name = ty.ident()?.text().to_string();
            if name.as_str() == "void" {
                TypeRef::Void
            } else {
                TypeRef::Basic { name }
            }
        }
        ast::Type::PointerType(ty) => TypeRef::Pointer {
            pointee: Rc::new(type_(&ty.pointee().unwrap())?),
        },
    })
}

fn scope(block: &ast::Block) -> Option<Scope> {
    let mut items = Vec::new();
    for item in block.items() {
        items.push(item_(&item)?);
    }
    let expr = if let Some(expr) = block.expr() {
        Some(expr_(&expr)?)
    } else {
        None
    };
    Some(Scope { items, expr })
}

fn item_(item: &ast::Item) -> Option<Item> {
    match item {
        ast::Item::FnDef(fn_item) => Some(Item::FnDef(function(fn_item)?)),
        ast::Item::Let(let_) => let_item(&let_),
        ast::Item::ExprItem(expr) => expr_item(&expr),
        ast::Item::TypeItem(type_) => todo!(),
    }
}

fn let_item(let_: &ast::Let) -> Option<Item> {
    Some(Item::Let {
        name: name_(&let_.name()?)?,
        typ_: if let Some(ty) = let_.type_() {
            Some(type_(&ty)?)
        } else {
            None
        },
        expr: if let Some(expr) = let_.expr() {
            Some(expr_(&expr)?)
        } else {
            None
        },
    })
}

fn expr_item(expr: &ast::ExprItem) -> Option<Item> {
    Some(Item::Expr(expr_(&expr.expr()?)?))
}

fn expr_(expr: &ast::Expr) -> Option<Expr> {
    match expr {
        ast::Expr::Literal(lit) => literal(&lit),
        ast::Expr::NameRef(name) => name_ref(&name),
        ast::Expr::PrefixExpr(expr) => prefix_expr(&expr),
        ast::Expr::BinExpr(expr) => binary_expr(&expr),
        ast::Expr::Group(expr) => group_expr(&expr),
        ast::Expr::Block(expr) => block_expr(&expr),
        ast::Expr::CallExpr(expr) => call_expr(&expr),
        ast::Expr::Return(expr) => return_expr(&expr),
        ast::Expr::IndexExpr(expr) => index_expr(&expr),
        ast::Expr::IfExpr(expr) => if_expr(&expr),
        ast::Expr::LoopExpr(expr) => loop_expr(&expr),
    }
}

fn literal(lit: &ast::Literal) -> Option<Expr> {
    // Trim off leading and trailing quotation marks '"'
    fn trim(s: &str) -> String {
        s[1..s.len() - 1].to_string()
    }

    Some(match lit.value().unwrap() {
        ast::LiteralValue::Number(n) => Expr::Literal(Literal::Number(n.text().parse().unwrap())),
        ast::LiteralValue::Str(s) => Expr::Literal(Literal::Str(trim(s.text()))),
    })
}

fn name_ref(ref_: &ast::NameRef) -> Option<Expr> {
    Some(Expr::NameRef {
        name: ref_.name()?.text().to_string(),
    })
}

fn prefix_expr(expr: &ast::PrefixExpr) -> Option<Expr> {
    let kind = match expr.op()?.text() {
        "+" => OpKind::Plus,
        "-" => OpKind::Minus,
        _ => unreachable!(),
    };
    let operand = expr_(&expr.operand().unwrap())?;
    Some(Expr::Op(Op {
        fixity: Fixity::Prefix,
        kind,
        operands: vec![operand],
    }))
}

fn binary_expr(expr: &ast::BinExpr) -> Option<Expr> {
    let kind = match expr.op()?.text() {
        "+"  => OpKind::Plus,
        "-"  => OpKind::Minus,
        "*"  => OpKind::Multiply,
        "/"  => OpKind::Divide,
        "."  => OpKind::FieldAccess,
        "==" => OpKind::Equals,
        ">"  => OpKind::GreaterThan,
        ">=" => OpKind::GreaterThanEquals,
        "<"  => OpKind::LessThan,
        "<=" => OpKind::LessThanEquals,
        "="  => OpKind::Assignment,
        kind => panic!("unrecognized op: {kind}"),
    };
    let lhs = expr_(&expr.lhs()?)?;
    let rhs = expr_(&expr.rhs()?)?;
    Some(Expr::Op(Op {
        fixity: Fixity::Infix,
        kind,
        operands: vec![lhs, rhs],
    }))
}

fn group_expr(group: &ast::Group) -> Option<Expr> {
    expr_(&group.inner().unwrap())
}

fn block_expr(block: &ast::Block) -> Option<Expr> {
    let mut items = Vec::new();
    for item in block.items() {
        items.push(item_(&item)?);
    }
    Some(Expr::Block { items })
}

fn call_expr(expr: &ast::CallExpr) -> Option<Expr> {
    let receiver = Box::new(expr_(&expr.receiver().unwrap())?);
    let mut operands = Vec::new();
    for arg in expr.arguments().by_ref() {
        operands.push(expr_(&arg)?);
    }
    Some(Expr::Call { receiver, operands })
}

fn return_expr(ret: &ast::Return) -> Option<Expr> {
    Some(Expr::Return {
        expr: Box::new(expr_(&ret.expr().unwrap())?),
    })
}

fn index_expr(expr: &ast::IndexExpr) -> Option<Expr> {
    Some(Expr::Index {
        receiver: Box::new(expr_(&expr.receiver().unwrap())?),
        index: Box::new(expr_(&expr.index().unwrap())?),
    })
}

fn if_expr(expr: &ast::IfExpr) -> Option<Expr> {
    Some(Expr::Branch {
        condition: Box::new(expr_(&expr.condition().unwrap())?),
        left: Box::new(scope(&expr.then().unwrap())?),
        right: if let Some(alt) = expr.alternate() {
            Box::new(scope(&alt)?)
        } else {
            Box::new(Scope::empty())
        },
    })
}

fn loop_expr(loop_: &ast::LoopExpr) -> Option<Expr> {
    Some(Expr::Loop {
        kind: LoopKind::Loop,
        body: Box::new(scope(&loop_.body().unwrap())?),
    })
}

fn type_definition(typ_: &ast::TypeItem) -> Option<TypeDefinition> {
    let mut members = Vec::new();
    for member in typ_.members() {
        members.push(TypeMember {
            name: member.ident().unwrap().text().to_string(),
            typ_: type_(&member.type_().unwrap())?,
        });
    }
    Some(TypeDefinition {
        name: typ_.ident().unwrap().text().to_string(),
        members,
    })
}
