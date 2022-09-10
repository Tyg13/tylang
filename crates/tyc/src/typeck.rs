use ast::AstNode;
use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct ID(usize);

pub struct TypeDatabase {
    types: Vec<Type>,
    type_identifiers: HashMap<ID, String>,
    based_types: Vec<BasedType>,
    error_id: ID,
}

struct TypeChecker<'ast, 'source> {
    db: TypeDatabase,
    module: &'ast ast::Module,
    source: &'source utils::Source,
}

impl<'ast, 'source> TypeChecker<'ast, 'source> {
    fn new(module: &'ast ast::Module, source: &'source utils::Source) -> Self {
        let mut db = TypeDatabase::new();
        for (name, kind) in BUILTINS {
            db.new_type(name, kind);
        }
        Self { db, module, source }
    }

    fn context(&self, id: usize) -> Option<String> {
        self.module.span(id).and_then(|span| {
            self.source
                .give_context_span(span, utils::HandPosition::WholeSpan)
        })
    }

    fn unify(&self, a: TypeRef, b: TypeRef) -> Result<ID, Error> {
        if a.id != b.id {
            Err(Error::Unification { a, b })
        } else {
            Ok(a.id)
        }
    }

    fn check(&mut self, module: &ast::Module) -> Result<(), Error> {
        self.check_tree(&module.tree)
    }

    fn check_tree(&mut self, tree: &ast::Tree) -> Result<(), Error> {
        for item in &tree.items {
            use ast::ItemKind::*;
            match &item.kind {
                Function(f) => {
                    self.check_function(f)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn check_function(&mut self, function: &ast::Function) -> Result<(), Error> {
        let mut bindings = HashMap::new();
        let mut param_types = Vec::new();
        for param in &function.parameters {
            let typ_ = self.check_parameter(&param)?;
            param_types.push(typ_.id);
            bindings.insert(param.variable.identifier.clone(), typ_);
        }
        let return_type = self.check_type(&function.return_type);
        self.db.new_type(
            &function.identifier,
            TypeKind::Function(param_types, return_type.id),
        );
        if let Some(ref body) = function.body {
            let body_type = self.check_scope(body, &bindings)?;
            self.unify(body_type, return_type)?;
        }
        Ok(())
    }

    fn check_parameter(&mut self, parameter: &ast::Ast<ast::Parameter>) -> Result<TypeRef, Error> {
        let param_type_id = self.check_type(&parameter.type_).id;
        Ok(self.type_ref(parameter, param_type_id))
    }

    fn check_scope(
        &mut self,
        scope: &ast::Ast<ast::Scope>,
        bindings: &HashMap<String, TypeRef>,
    ) -> Result<TypeRef, Error> {
        let mut bindings = bindings.clone();
        for statement in &scope.statements {
            match &statement.kind {
                ast::StatementKind::Declaration(ref decl) => {
                    if let Some(initializer) = &decl.initializer {
                        let initializer_type = self.check_expression(initializer, &bindings)?;
                        let decl_type = self.check_type(&decl.type_);
                        self.unify(initializer_type, decl_type)?;
                        bindings.insert(decl.var.identifier.clone(), decl_type);
                    }
                }
                ast::StatementKind::Assignment { ref src, ref dst } => match dst.kind {
                    ast::ExpressionKind::Variable(ref var) => {
                        self.unify(
                            bindings.get(&var.identifier).copied().unwrap(),
                            self.check_expression(src, &bindings)?,
                        )?;
                    }
                    _ => {}
                },
                ast::StatementKind::Return(ref exp) => {
                    return self.check_expression(exp, &bindings);
                }
                _ => (),
            }
        }
        Ok(self.type_ref(scope, self.db.error()))
    }

    fn check_expression(
        &self,
        exp: &ast::Ast<ast::Expression>,
        bindings: &HashMap<String, TypeRef>,
    ) -> Result<TypeRef, Error> {
        match exp.kind {
            ast::ExpressionKind::Group(ref exp) => self.check_expression(exp, bindings),
            ast::ExpressionKind::Constant(ref constant) => self.check_constant(constant),
            ast::ExpressionKind::Variable(ref variable) => self.check_variable(variable, bindings),
            ast::ExpressionKind::BinaryOp {
                ref lhs, ref rhs, ..
            } => Ok(self.type_ref(
                exp,
                self.unify(
                    self.check_expression(lhs, bindings)?,
                    self.check_expression(rhs, bindings)?,
                )?,
            )),
            ast::ExpressionKind::Call {
                ref name,
                ref arguments,
            } => Ok(self.type_ref(exp, self.check_function_call(name, arguments, bindings)?)),
            ast::ExpressionKind::Error => Ok(self.type_ref(exp, self.db.error())),
        }
    }

    fn check_function_call(
        &self,
        name: &str,
        arguments: &[ast::Ast<ast::Expression>],
        bindings: &HashMap<String, TypeRef>,
    ) -> Result<ID, Error> {
        let func_type = self
            .db
            .type_by_identifier(name)
            .map(|id| &self.db.types[id.0])
            .ok_or(Error::Generic(format!("no function {}", name)))?;
        match &func_type.kind {
            TypeKind::Function(args, return_type) => {
                for i in 0..std::cmp::max(args.len(), arguments.len()) {
                    let use_arg = self.check_expression(
                        arguments.get(i).ok_or(Error::Generic(format!(
                            "in function call to `{}`: not enough arguments given",
                            name
                        )))?,
                        bindings,
                    )?;
                    let def_arg = TypeRef {
                        ast_id: None,
                        id: args.get(i).copied().ok_or(Error::Generic(format!(
                            "in function call to `{}`: too many arguments given",
                            name
                        )))?,
                    };
                    self.unify(use_arg, def_arg)?;
                }
                Ok(*return_type)
            }
            _ => Err(Error::Generic(format!(
                "{} is not a function",
                self.db.identifier_from_id(func_type.id).unwrap()
            ))),
        }
    }

    fn check_constant(&self, constant: &ast::Ast<ast::Constant>) -> Result<TypeRef, Error> {
        Ok(self.type_ref(
            constant,
            self.db.type_by_identifier("i64").ok_or(Error::Generic(
                "internal type map inconsistency".to_string(),
            ))?,
        ))
    }

    fn check_variable(
        &self,
        variable: &ast::Ast<ast::Variable>,
        bindings: &HashMap<String, TypeRef>,
    ) -> Result<TypeRef, Error> {
        Ok(self.type_ref(
            variable,
            bindings
                .get(&variable.identifier)
                .map(|typeref| typeref.id)
                .unwrap_or(self.db.error()),
        ))
    }

    fn check_type(&mut self, type_: &ast::Ast<ast::Type>) -> TypeRef {
        let id = match &type_.kind {
            ast::TypeKind::Error => self.db.error(),
            ast::TypeKind::Pointer(ref type_) => {
                let pointed_to_id = self.check_type(type_).id;
                self.db.based_type_of(pointed_to_id)
            }
            ast::TypeKind::Type(name) => {
                self.db.type_by_identifier(&name).unwrap_or(self.db.error())
            }
        };
        self.type_ref(type_, id)
    }

    fn type_ref<T: AstNode>(&self, node: &T, type_id: ID) -> TypeRef {
        TypeRef {
            ast_id: Some(node.id()),
            id: type_id,
        }
    }
}

#[derive(Debug)]
struct BasedType {
    id: ID,
    based_id: ID,
}

const BUILTINS: [(&'static str, TypeKind); 4] = [
    ("<err>", TypeKind::Error),
    (
        "i64",
        TypeKind::Builtin(BuiltinKind::Integer {
            signed: true,
            size: 64,
        }),
    ),
    (
        "u64",
        TypeKind::Builtin(BuiltinKind::Integer {
            signed: false,
            size: 64,
        }),
    ),
    ("str", TypeKind::Builtin(BuiltinKind::String)),
];

impl TypeDatabase {
    fn new() -> Self {
        Self {
            types: Vec::new(),
            type_identifiers: Default::default(),
            based_types: Vec::new(),
            error_id: ID(0),
        }
    }

    fn error(&self) -> ID {
        self.error_id
    }

    fn identifier_from_id(&self, id: ID) -> Option<&str> {
        self.type_identifiers
            .get(&id)
            .map(|identifier| identifier.as_str())
    }

    fn new_type(&mut self, identifier: &str, kind: TypeKind) -> ID {
        let id = ID(self.types.len());
        self.type_identifiers.insert(id, identifier.to_string());
        self.types.push(Type { id, kind });
        id
    }

    fn type_by_identifier(&self, type_identifier: &str) -> Option<ID> {
        self.type_identifiers.iter().find_map(|(id, identifier)| {
            if type_identifier == identifier {
                Some(*id)
            } else {
                None
            }
        })
    }

    fn find_based_type(&self, type_: ID) -> Option<ID> {
        self.based_types.iter().find_map(|based_type| {
            if based_type.based_id == type_ {
                Some(based_type.id)
            } else {
                None
            }
        })
    }

    fn based_type_of(&mut self, type_: ID) -> ID {
        self.find_based_type(type_).unwrap_or_else(|| {
            let based_type_name = self.identifier_from_id(type_).unwrap();
            let identifier = format!("*{based_type_name}");
            let id = self.new_type(&identifier, TypeKind::Pointer(type_));
            self.based_types.push(BasedType {
                id,
                based_id: type_,
            });
            id
        })
    }
}

pub fn check(module: &ast::Module, source: &utils::Source) -> Result<TypeDatabase, Error> {
    let mut checker = TypeChecker::new(module, source);
    checker.check(module).map_err(|e| e.normalize(&checker))?;
    Ok(checker.db)
}

#[derive(Debug)]
pub enum Error {
    Unification { a: TypeRef, b: TypeRef },
    Generic(String),
}

impl Error {
    fn normalize(self, checker: &TypeChecker) -> Self {
        let fmt = |name: &str, id: Option<ast::ID>| -> Option<String> {
            id.map(|id| {
                let kind = checker.module.kind(id.0);
                if let Some(ctxt) = checker.context(id.0) {
                    format!("\n{name} ({id}: `{kind}`):\n{ctxt}", id = id.0)
                } else {
                    format!("\n{name} ({id}: `{kind}`)", id = id.0)
                }
            })
        };
        match self {
            Error::Unification { a, b } => {
                let a_name = checker.db.identifier_from_id(a.id).unwrap();
                let b_name = checker.db.identifier_from_id(b.id).unwrap();
                Error::Generic(format!(
                    "unifying {a_name} with {b_name}{context}",
                    context = {
                        let mut context = String::new();
                        fmt(a_name, a.ast_id).map(|ctxt| context.push_str(&ctxt));
                        fmt(b_name, b.ast_id).map(|ctxt| context.push_str(&ctxt));
                        context
                    }
                ))
            }
            _ => self,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Error::Unification { .. } => write!(f, "internal error: unification error escaped!"),
            Error::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

#[derive(Debug)]
struct Type {
    id: ID,
    kind: TypeKind,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeRef {
    id: ID,
    ast_id: Option<ast::ID>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TypeKind {
    Function(Vec<ID>, ID),
    Builtin(BuiltinKind),
    Pointer(ID),
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BuiltinKind {
    Integer { signed: bool, size: usize },
    String,
}
