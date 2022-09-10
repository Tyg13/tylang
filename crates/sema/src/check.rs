use crate::{
    errors::{Error, ErrorKind},
    types::*,
};

struct Checker<'bir> {
    map: Map,
    bir: &'bir bir::Map,

    namespace_stack: Vec<ID>,
    global_namespace: Option<ID>,

    based_types: Vec<BasedType>,

    bool_type: Option<ID>,
    index_type: Option<ID>,
    void_type: Option<ID>,
    string_type: Option<ID>,
    number_type: Option<ID>,
    never_type: Option<ID>,
}

impl<'bir> Checker<'bir> {
    fn new(bir: &'bir bir::Map) -> Self {
        Self {
            map: Map::default(),
            bir,
            namespace_stack: Vec::default(),
            global_namespace: None,
            based_types: Vec::default(),
            bool_type: None,
            index_type: None,
            void_type: None,
            string_type: None,
            number_type: None,
            never_type: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BasedType {
    id: ID,
    based_on: ID,
    kind: BasedTypeKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum BasedTypeKind {
    Pointer,
}

impl Checker<'_> {
    fn current_ns(&mut self) -> NamespaceHandle<'_> {
        let ns_id = self.namespace_stack.last().expect("namespace stack empty!");
        self.map
            .ns_mut(*ns_id)
            .expect("current namespace ID has no associated namespace!")
    }

    fn new_marker_ty(&mut self) -> ID {
        self.map.new_ty(TypeKind::Marker)
    }

    fn bool_type(&self) -> ID {
        self.bool_type.unwrap()
    }

    fn index_type(&self) -> ID {
        self.index_type.unwrap()
    }

    fn void_type(&self) -> ID {
        self.void_type.unwrap()
    }

    fn string_type(&self) -> ID {
        self.string_type.unwrap()
    }

    fn never_type(&self) -> ID {
        self.never_type.unwrap()
    }

    fn global_ns(&mut self) -> NamespaceHandle<'_> {
        let ns_id = self.global_namespace.expect("no global namespace set!");
        self.map
            .ns_mut(ns_id)
            .expect("global namespace ID has no associated namespace!")
    }

    fn in_ns<R>(&mut self, ns: ID, f: impl FnOnce(&mut Self) -> R) -> R {
        self.namespace_stack.push(ns);
        let res = f(self);
        self.namespace_stack.pop();
        res
    }

    fn lookup(&self, ident: &str) -> Option<&Name> {
        for ns in self
            .namespace_stack
            .iter()
            .rev()
            .map(|id| self.map.ns(*id).unwrap())
        {
            if let Some(name) = ns.lookup(&self.map, ident) {
                return Some(name);
            }
        }
        None
    }

    fn ty_of(&self, id: ID) -> ID {
        self.map.ty_id(id).unwrap_or_else(|| {
            debug_assert_eq!(self.map.kind(id), Kind::Error);
            id
        })
    }

    fn add_fn_proto(
        &mut self,
        bir: bir::ID,
        ident: &str,
        return_: ID,
        param_types: Vec<ID>,
    ) -> PrototypeFn {
        debug_assert!(self.map.ty_id(return_).is_some());
        debug_assert!(param_types.iter().all(|id| self.map.ty_id(*id).is_some()));

        let proto = self.current_ns().new_fn_proto(ident, bir, return_);
        let ty = {
            let return_ty = self.map.ty_id(return_).unwrap();
            self.current_ns().new_ty(
                None,
                TypeKind::Function(FunctionType {
                    return_ty,
                    parameters: param_types,
                }),
            )
        };
        self.map.set_ty(proto.id, ty);
        self.map.set_bir(ty, bir);
        self.map.set_bir(proto.id, bir);
        proto
    }

    fn finish_fn_proto(&mut self, proto: PrototypeFn, params: Vec<ID>) -> ID {
        proto.finish(&mut self.map, params)
    }

    fn add_param(&mut self, bir: bir::ID, ident: &str, ty: ID) -> ID {
        let id = self.current_ns().new_param(ident);
        self.map.set_ty(id, ty);
        self.map.set_bir(id, bir);
        id
    }

    fn add_var(&mut self, bir: bir::ID, ident: &str, ty: ID) -> ID {
        let id = self.current_ns().new_var(ident);
        self.map.set_ty(id, ty);
        self.map.set_bir(id, bir);
        id
    }

    fn set_err(&mut self, id: ID, err_kind: ErrorKind, ids: &[ID]) {
        let ids = match err_kind {
            ErrorKind::UnknownType
            | ErrorKind::UnknownName
            | ErrorKind::DuplicateBinding
            | ErrorKind::DuplicateType
            | ErrorKind::UnknownCall
            | ErrorKind::InvalidPointeeType
            | ErrorKind::ParamAssignment => vec![ids[0]],
            ErrorKind::Unification | ErrorKind::InvalidIndexType => vec![ids[0], ids[1]],
        };
        self.map.set_err(
            id,
            Error {
                ids,
                kind: err_kind,
            },
        )
    }

    fn err(&mut self, err_kind: ErrorKind, bir: bir::ID) -> ID {
        let id = self.map.new_node(Kind::Error);
        self.map.set_bir(id, bir);
        self.set_err(id, err_kind, &[id]);
        id
    }

    fn new_ty_proto(&mut self, bir: bir::ID, ident: &str) -> PrototypeTy {
        let ty = self.current_ns().new_ty_proto(Some(ident));
        self.map.set_bir(ty.id, bir);
        ty
    }

    fn finish_ty_proto(&mut self, proto: PrototypeTy, kind: TypeKind) -> ID {
        proto.finish(&mut self.map, kind)
    }

    fn unify(&mut self, a: ID, b: ID) -> Option<ID> {
        fn is_marker(this: &Checker, ty: ID) -> bool {
            if let Some(ty) = this.map.try_get::<Type>(ty) {
                ty.is_marker()
            } else {
                false
            }
        }
        match (self.map.ty_id(a), self.map.ty_id(b)) {
            (None, None) => Some(b),
            (Some(..), None) => Some(a),
            (None, Some(..)) => Some(b),
            (Some(a_ty), Some(b_ty)) => {
                if a_ty == b_ty {
                    return Some(a_ty);
                }

                if is_marker(self, a_ty) {
                    self.map.resolve_marker(a_ty, b_ty);
                    return Some(b_ty);
                }
                if is_marker(self, b_ty) {
                    self.map.resolve_marker(b_ty, a_ty);
                    return Some(a_ty);
                }

                if self.map.is_err(a_ty) {
                    return Some(b_ty);
                }
                if self.map.is_err(b_ty) {
                    return Some(a_ty);
                }

                if a_ty == self.never_type() {
                    return Some(b_ty);
                }
                if b_ty == self.never_type() {
                    return Some(a_ty);
                }
                None
            }
        }
    }

    fn no_prototypes(&self) -> bool {
        self.map
            .types()
            .find(|ty| matches!(ty.kind, TypeKind::Prototype))
            .is_none()
    }

    fn assert_no_markers(&self) {
        for ty in self.map.types() {
            if matches!(ty.kind, TypeKind::Marker) {
                let tyref = self.bir.typeref(&self.map.bir(ty.id).unwrap());
                panic!("marker type found: {:?}\n{:#?}", ty.id, tyref);
            }
        }
    }

    fn add_based_ty(&mut self, kind: BasedTypeKind, ty: ID) -> ID {
        debug_assert_eq!(self.map.kind(ty), Kind::Type);
        let id = match kind {
            // TODO should this use the type's defining namespace instead of the global one?
            BasedTypeKind::Pointer => self
                .global_ns()
                .new_ty(None, TypeKind::Pointer { pointee: ty }),
        };
        self.based_types.push(BasedType {
            id,
            based_on: ty,
            kind,
        });
        id
    }

    fn find_based_ty(&self, ty: ID, kind: BasedTypeKind) -> Option<ID> {
        self.based_types.iter().find_map(|based_ty| {
            (based_ty.based_on == ty && based_ty.kind == kind).then(|| based_ty.id)
        })
    }

    fn get_based_ty(&mut self, ty: ID, kind: BasedTypeKind) -> ID {
        self.find_based_ty(ty, kind)
            .unwrap_or_else(|| self.add_based_ty(kind, ty))
    }
}

pub fn check(bir: &bir::Map) -> Map {
    let mut ck = Checker::new(bir);

    let root = bir.root_module();
    let global_module = ck.map.new_node(Kind::Module);
    ck.global_namespace = Some(global_module);
    ck.map.set_bir(global_module, root.id);

    ck.in_ns(global_module, |ck| {
        add_builtin_tys(ck);
        check_mod_inner(ck, root);
    });

    debug_assert!(!ck.map.any_markers_or_prototypes());

    ck.map
}

fn add_builtin_tys(ck: &mut Checker) {
    fn add_builtin(ck: &mut Checker, name: &str, kind: TypeKind) -> ID {
        ck.current_ns().new_ty(Some(name), kind)
    }
    ck.void_type = Some(add_builtin(ck, "void", TypeKind::Void));
    ck.number_type = Some(
        ck.current_ns()
            .new_ty(None, TypeKind::Integer { size: usize::MAX }),
    );
    ck.string_type = Some(add_builtin(ck, "str", TypeKind::String));
    ck.bool_type = Some(add_builtin(ck, "bool", TypeKind::Integer { size: 1 }));
    add_builtin(ck, "i8", TypeKind::Integer { size: 8 });
    add_builtin(ck, "i16", TypeKind::Integer { size: 16 });
    add_builtin(ck, "i32", TypeKind::Integer { size: 32 });
    ck.index_type = Some(add_builtin(ck, "i64", TypeKind::Integer { size: 64 }));

    ck.never_type = Some(add_builtin(ck, "!", TypeKind::Never));
}

fn check_mod_inner(ck: &mut Checker, mod_: &bir::Module) {
    check_typedefs(ck, mod_.typedefs(ck.bir));
    check_functions(ck, mod_.functions(ck.bir));
}

fn check_typedefs<'bir>(ck: &mut Checker<'bir>, defs: impl Iterator<Item = &'bir bir::TypeDef>) {
    // First pass over the types to add their names as prototypes
    let prototypes: Vec<(&'bir bir::TypeDef, PrototypeTy)> = defs
        .map(|ty| (ty, ck.new_ty_proto(ty.id, &ty.identifier)))
        .collect();

    // Now pass over the prototypes and finish their definition
    for (def, p) in prototypes {
        let members = def
            .members
            .iter()
            .map(|member| {
                let id = check_typeref(ck, member.ty(ck.bir));
                ck.ty_of(id)
            })
            .collect();
        ck.finish_ty_proto(p, TypeKind::Aggregate { members });
    }
    debug_assert!(ck.no_prototypes());
}

fn check_functions<'bir>(ck: &mut Checker<'bir>, fns: impl Iterator<Item = &'bir bir::Function>) {
    // First iterate over all functions, processing the param types and return type.
    let proto_fns: Vec<PrototypeFn> = fns
        .map(|fn_| {
            let parameters = fn_
                .parameters(&ck.bir)
                .map(|param| {
                    let id = check_typeref(ck, param.ty(ck.bir));
                    ck.ty_of(id)
                })
                .collect::<Vec<_>>();
            let return_ = check_typeref(ck, fn_.return_type(ck.bir));
            ck.add_fn_proto(fn_.id, &fn_.identifier, return_, parameters)
        })
        .collect();

    // Now iterate over the function bodies. We also create the function's
    // namespace and register the parameters' names
    for proto in proto_fns {
        ck.map.set_bir(proto.id, proto.bir);

        ck.in_ns(proto.id, |ck| {
            let fn_ = ck.bir.fn_(&proto.bir);
            let fn_ty = ck.map.ty(proto.id).unwrap().as_fn_ty();
            let params = fn_
                .parameters(&ck.bir)
                .enumerate()
                .map(|(idx, param)| {
                    ck.add_param(param.id, &param.identifier, fn_ty.parameters[idx])
                })
                .collect();

            if let Some(body) = fn_.body(ck.bir) {
                let scope_ = check_scope(ck, body);
                if ck.unify(scope_, fn_ty.return_ty).is_none() {
                    let b_id = body
                        .return_expr(ck.bir)
                        .map(|expr| expr.id)
                        .or_else(|| body.items(ck.bir).last().map(|item| item.id))
                        .unwrap_or_else(|| body.id);
                    ck.set_err(
                        scope_,
                        ErrorKind::Unification,
                        &[proto.return_, ck.map.bir_to_id(b_id).unwrap()],
                    );
                }
                ck.assert_no_markers();
            }
            ck.finish_fn_proto(proto, params)
        });
    }
}

fn check_typeref(ck: &mut Checker, tyref: &bir::TypeRef) -> ID {
    let ty = match &tyref.kind {
        bir::TypeRefKind::Void => ck.void_type(),
        bir::TypeRefKind::Named { name } => ck
            .lookup(&name)
            .map(|name| name.ty(&ck.map).unwrap().id)
            .unwrap_or_else(|| ck.err(ErrorKind::UnknownType, tyref.id)),
        bir::TypeRefKind::Pointer { pointee } => {
            let pointee = check_typeref(ck, ck.bir.typeref(&pointee));
            let pointee_ty = ck.ty_of(pointee);
            ck.get_based_ty(pointee_ty, BasedTypeKind::Pointer)
        }
    };
    let id = ck.map.new_node(Kind::Other);
    ck.map.set_bir(id, tyref.id);
    ck.map.set_ty(id, ty);
    id
}

fn check_scope<'bir>(ck: &mut Checker<'bir>, scope: &'bir bir::Scope) -> ID {
    let id = ck.current_ns().new_block();
    ck.map.set_bir(id, scope.id);

    for item in scope.items(ck.bir) {
        check_item(ck, item);
    }

    let expr = scope
        .return_expr(ck.bir)
        .map(|expr| {
            let expr = check_expr(ck, expr);
            ck.ty_of(expr)
    });
    let ty = expr.unwrap_or(ck.void_type());
    ck.map.set_ty(id, ty);
    id
}

fn check_item<'bir>(ck: &mut Checker<'bir>, item: &'bir bir::Item) -> ID {
    match &item.kind {
        bir::ItemKind::Let(id) => check_let(ck, ck.bir.let_(id)),
        bir::ItemKind::Expr(id) => check_expr(ck, ck.bir.expr(id)),
    }
}

fn check_let<'bir>(ck: &mut Checker<'bir>, let_: &'bir bir::Let) -> ID {
    let tyref = match let_.ty(ck.bir) {
        Some(ty) => check_typeref(ck, ty),
        None => ck.new_marker_ty(),
    };
    let mut ty = ck.ty_of(tyref);
    if let Some(expr) = let_.expr(ck.bir) {
        let expr = check_expr(ck, expr);
        match ck.unify(tyref, expr) {
            Some(t) => ty = t,
            None => ck.set_err(expr, ErrorKind::Unification, &[expr, tyref]),
        }
    }
    ck.add_var(let_.id, &let_.name, ty)
}

fn check_expr<'bir>(ck: &mut Checker<'bir>, expr: &'bir bir::Expr) -> ID {
    // TODO? don't skip making Other nodes, but implement parent system so that we can walk up
    // Other nodes to the actual node that has data?
    match &expr.kind {
        bir::ExprKind::Literal(lit) => {
            let id = match ck.bir.lit(lit) {
                bir::Literal::Number(n) => {
                    let ty = ck.new_marker_ty();
                    ck.map.new_constant(ty, Constant::Int(*n))
                }
                bir::Literal::Str(s) => {
                    let ty = ck.string_type();
                    ck.map.new_constant(ty, Constant::Str(s.clone()))
                }
            };
            ck.map.set_bir(id, *lit);
            ck.map.set_bir(id, expr.id);
            return id;
        }
        bir::ExprKind::NameRef { name } => {
            let id = ck
                .lookup(&name)
                .map(|name| name.id)
                .unwrap_or_else(|| ck.err(ErrorKind::UnknownName, expr.id));
            ck.map.set_bir(id, expr.id);
            return id;
        }
        _ => {}
    };

    let ty = match &expr.kind {
        bir::ExprKind::Literal(..) | bir::ExprKind::NameRef { .. } => unreachable!(),
        bir::ExprKind::Call { receiver, operands } => check_call_expr(ck, receiver, operands),
        bir::ExprKind::Index { receiver, index } => check_index_expr(ck, receiver, index),
        bir::ExprKind::Op(op) => check_op_expr(ck, op),
        bir::ExprKind::Block { scope } => {
            let scope = check_scope(ck, ck.bir.scope(scope));
            scope
        }
        bir::ExprKind::Return { expr } => ck.never_type(),
        bir::ExprKind::Break { label } => todo!(),
        bir::ExprKind::Continue { label } => todo!(),
        bir::ExprKind::Branch {
            condition,
            kind,
            left,
            right,
        } => {
            let cond = todo!();
            if ck.unify(cond, ck.bool_type()).is_none() {
                ck.set_err(cond, ErrorKind::Unification, &[cond, cond]);
            }
            let left = check_scope(ck, ck.bir.scope(left));
            match kind {
                bir::BranchKind::If => ck.void_type(),
                bir::BranchKind::IfElse => {
                    let right = check_scope(ck, ck.bir.scope(right));
                    match ck.unify(left, right) {
                        Some(ty) => ty,
                        None => {
                            ck.set_err(cond, ErrorKind::Unification, &[left, right]);
                            left
                        }
                    }
                }
            }
        }
        bir::ExprKind::Loop { kind, body } => todo!(),
    };
    let id = ck.map.new_node(Kind::Other);
    ck.map.set_ty(id, ty);
    ck.map.set_bir(id, expr.id);
    id
}

fn check_index_expr(ck: &mut Checker, receiver: &bir::ID, index: &bir::ID) -> ID {
    let receiver_id = check_expr(ck, ck.bir.expr(receiver));
    let expr_id = check_expr(ck, ck.bir.expr(index));
    if ck.unify(expr_id, ck.index_type()).is_none() {
        ck.set_err(
            expr_id,
            ErrorKind::InvalidIndexType,
            &[receiver_id, expr_id],
        );
    }
    let receiver_ty = ck.map.ty(receiver_id).unwrap();
    receiver_ty
        .pointee()
        .unwrap_or_else(|| ck.err(ErrorKind::InvalidPointeeType, *receiver))
}

fn check_call_expr(ck: &mut Checker, receiver: &bir::ID, operands: &Vec<bir::ID>) -> ID {
    let expr = ck.bir.expr(receiver).name().unwrap();
    let fn_id = match ck.lookup(expr) {
        Some(name) => name.id,
        None => return ck.err(ErrorKind::UnknownName, *receiver),
    };
    ck.map.set_bir(fn_id, *receiver);

    let fn_ty = ck.map.ty(fn_id).unwrap().as_fn_ty();
    let op_exprs: Vec<_> = operands
        .iter()
        .map(|id| check_expr(ck, ck.bir.expr(id)))
        .collect();
    let mut call_sig_mismatch = false;
    if fn_ty.parameters.len() != op_exprs.len() {
        // Call sig length mismatch, just ignore params -- they won't apply anyways.
        call_sig_mismatch = true;
    } else {
        // Call sig length matches at least -- now check if the params match as well.
        for (idx, param_ty) in fn_ty.parameters.iter().enumerate() {
            let op = op_exprs[idx];
            if ck.unify(ck.ty_of(op), *param_ty).is_none() {
                ck.set_err(op, ErrorKind::Unification, &[op, *param_ty]);
                call_sig_mismatch = true;
            }
        }
    }
    if call_sig_mismatch {
        ck.err(ErrorKind::UnknownCall, *receiver);
    }
    fn_ty.return_ty
}

fn check_op_expr(ck: &mut Checker, op: &bir::Op) -> ID {
    match (op.fixity, op.kind) {
        (bir::OpFixity::Infix, kind) => match kind {
            bir::OpKind::Plus
            | bir::OpKind::Minus
            | bir::OpKind::Multiply
            | bir::OpKind::Divide => {
                let lhs = check_expr(ck, ck.bir.expr(&op.operands[0]));
                let rhs = check_expr(ck, ck.bir.expr(&op.operands[1]));
                ck.unify(lhs, rhs).unwrap_or_else(|| {
                    // TODO this should be set on the expr itself
                    ck.set_err(lhs, ErrorKind::Unification, &[lhs, rhs]);
                    lhs
                })
            }
            bir::OpKind::FieldAccess => todo!(),
            bir::OpKind::LessThan
            | bir::OpKind::LessThanEquals
            | bir::OpKind::GreaterThan
            | bir::OpKind::GreaterThanEquals
            | bir::OpKind::Equals => {
                let lhs = check_expr(ck, ck.bir.expr(&op.operands[0]));
                let rhs = check_expr(ck, ck.bir.expr(&op.operands[1]));
                if ck.unify(lhs, rhs).is_none() {
                    // TODO this should be set on the expr itself
                    ck.set_err(lhs, ErrorKind::Unification, &[lhs, rhs]);
                }
                ck.bool_type()
            }
            bir::OpKind::Assignment => {
                let dst = check_expr(ck, ck.bir.expr(&op.operands[0]));
                if ck.map.param(dst).is_some() {
                    ck.set_err(dst, ErrorKind::ParamAssignment, &[dst]);
                }
                let src = check_expr(ck, ck.bir.expr(&op.operands[1]));
                if ck.unify(dst, src).is_none() {
                    // TODO this should be set on the expr itself
                    ck.set_err(dst, ErrorKind::Unification, &[dst, src]);
                }
                ck.void_type()
            }
        },
        (bir::OpFixity::Postfix, _) => todo!(),
        (bir::OpFixity::Prefix, _) => todo!(),
    }
}
