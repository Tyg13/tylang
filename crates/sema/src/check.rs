use crate::{
    errors::{Error, ErrorKind},
    types::*,
};
use assert_matches::debug_assert_matches;

struct Checker<'bir> {
    map: Map,
    bir: &'bir bir::Map,

    current_namespace: Option<ID>,
    current_function: Option<ID>,
    global_namespace: Option<ID>,
    based_types: Vec<BasedType>,

    check_namespace_parents: bool,
}

impl<'bir> Checker<'bir> {
    fn new(bir: &'bir bir::Map) -> Self {
        Self {
            map: Map::default(),
            bir,
            current_namespace: None,
            current_function: None,
            global_namespace: None,
            based_types: Default::default(),
            check_namespace_parents: true,
        }
    }
}

impl Checker<'_> {
    fn bir_to_id(&self, bir: &bir::ID) -> ID {
        self.map
            .bir_to_id(bir)
            .expect(&format!("no sema for bir {bir:?}"))
    }

    fn current_ns(&mut self) -> NamespaceHandle<'_> {
        self.map
            .ns_mut(self.current_namespace.expect("No current namespace set!"))
            .expect("current namespace ID has no associated namespace!")
    }

    fn current_fn(&self) -> &Function {
        let id = self.current_function.expect("Not in a function context!");
        self.map
            .fn_(id)
            .expect("Current function is not a function?")
    }

    fn new_marker_ty(&mut self) -> ID {
        self.map.new_ty(TypeKind::Marker)
    }

    fn void_type(&mut self) -> ID {
        self.map.void_type()
    }

    fn bool_type(&self) -> ID {
        self.map.bool_type()
    }

    fn index_type(&self) -> ID {
        self.map.index_type()
    }

    fn string_type(&self) -> ID {
        self.map.string_type()
    }

    fn never_type(&self) -> ID {
        self.map.never_type()
    }

    fn global_ns(&self) -> ID {
        self.global_namespace.expect("no global namespace set!")
    }

    fn global_ns_mut(&mut self) -> NamespaceHandle<'_> {
        self.map
            .ns_mut(self.global_ns())
            .expect("global namespace ID has no associated namespace!")
    }

    fn in_ns<R>(&mut self, ns: ID, f: impl FnOnce(&mut Self) -> R) -> R {
        debug_assert!(self.map.ns(ns).is_some());

        let old_ns = self.current_namespace;
        let old_fn = self.current_function;

        self.current_namespace = Some(ns);
        if self.map.kind(ns) == Kind::Function {
            self.current_function = Some(ns);
        }

        let res = f(self);

        self.current_namespace = old_ns;
        self.current_function = old_fn;

        res
    }

    fn lookup_ref(&self, name: &bir::ID) -> Option<ID> {
        self.lookup_in(self.current_namespace?, name)
    }

    fn lookup_in(&self, mut ns: ID, name: &bir::ID) -> Option<ID> {
        let name = self.bir.name(name);
        assert!(name.segments.len() > 0);
        let mut result = None;
        for ident in name.segments.iter() {
            let id = self
                .map
                .ns(ns)?
                .lookup(&self.map, ident, self.check_namespace_parents)?
                .id;
            result = Some(id);
            ns = id;
        }
        result
    }

    fn ty_id(&self, id: ID) -> ID {
        self.map.ty_id(id).unwrap_or_else(|| {
            debug_assert_eq!(self.map.kind(id), Kind::Error);
            id
        })
    }

    fn add_fn_proto(
        &mut self,
        bir: bir::ID,
        ident: &str,
        return_ty: ID,
        param_types: Vec<ID>,
        is_var_args: bool,
    ) -> PrototypeFn {
        debug_assert_matches!(
            self.map.kind(return_ty),
            Kind::Type | Kind::Error
        );
        debug_assert!(param_types
            .iter()
            .all(|id| self.map.ty_id(*id).is_some()));

        let ty = {
            let return_ty = self.map.ty_id(return_ty).unwrap_or(return_ty);
            // TODO should re-use existing function types with same ret/params (use FoldingSet)
            self.current_ns().new_ty(
                None,
                TypeKind::Function(FunctionType {
                    return_ty,
                    is_var_args,
                    parameters: param_types,
                }),
            )
        };
        let proto = self.current_ns().new_fn_proto(ident, bir, return_ty);
        self.map.set_ty(proto.id, ty);
        self.map.set_bir(proto.id, bir);
        proto
    }

    fn add_var(&mut self, bir: bir::ID, ident: &str, ty: ID) -> ID {
        let id = self.current_ns().new_var(ident);
        self.map.set_ty(id, ty);
        self.map.set_bir(id, bir);
        id
    }

    fn add_param(&mut self, bir: bir::ID, ident: &str, ty: ID) -> ID {
        let id = self.current_ns().new_param(ident);
        self.map.set_ty(id, ty);
        self.map.set_bir(id, bir);
        id
    }

    fn finish_fn_proto(&mut self, proto: PrototypeFn, params: Vec<ID>) -> ID {
        proto.finish(&mut self.map, params)
    }

    fn set_err(&mut self, id: ID, err_kind: ErrorKind, ids: &[ID]) {
        let ids = match err_kind {
            ErrorKind::UnknownType
            | ErrorKind::UnknownName
            | ErrorKind::DuplicateBinding
            | ErrorKind::DuplicateType
            | ErrorKind::UnknownCall
            | ErrorKind::InvalidPointeeType
            | ErrorKind::ParamAssignment
            | ErrorKind::InvalidField
            | ErrorKind::InvalidCallReceiver
            | ErrorKind::InvalidFieldReceiver
            | ErrorKind::CallToNonFnType => vec![ids[0]],
            ErrorKind::Unification | ErrorKind::InvalidIndexType => {
                vec![ids[0], ids[1]]
            }
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
        let id = self.current_ns().new_node(Kind::Error);
        self.map.set_bir(id, bir);
        self.set_err(id, err_kind, &[id]);
        id
    }

    fn new_ty_proto(&mut self, bir: bir::ID, ident: &str) -> PrototypeTy {
        let ty = self.current_ns().new_ty_proto(Some(ident));
        self.map.set_bir(ty.id, bir);
        ty
    }

    fn finish_ty_proto(&mut self, proto: PrototypeTy, members: Vec<ID>) -> ID {
        debug_assert!(members
            .iter()
            .all(|id| self.map.kind(*id) == Kind::TypeMember));
        let id = proto.id;
        proto.finish(
            &mut self.map,
            TypeKind::Aggregate(AggregateType { id, members }),
        )
    }

    fn unify(&mut self, sink: ID, src: ID) -> Option<ID> {
        fn is_marker(this: &Checker, ty: ID) -> bool {
            this.map.get::<Type>(ty).is_marker()
        }
        fn is_err(this: &Checker, ty: ID) -> bool {
            this.map.is_err(ty)
        }
        fn is_never(this: &Checker, ty: ID) -> bool {
            matches!(this.map.get::<Type>(ty).kind, TypeKind::Never)
        }
        match (self.map.ty_id(sink), self.map.ty_id(src)) {
            (None, None) => Some(sink),
            (Some(sink), None) => Some(sink),
            (None, Some(src)) => Some(src),
            (Some(sink), Some(src)) => {
                if sink == src {
                    return Some(sink);
                }

                if is_err(self, sink) {
                    return Some(src);
                }
                if is_err(self, src) {
                    return Some(sink);
                }

                if is_marker(self, sink) {
                    self.map.resolve_marker(sink, src);
                    return Some(src);
                }

                if is_marker(self, src) {
                    self.map.resolve_marker(src, sink);
                    return Some(sink);
                }

                if is_never(self, sink) {
                    return Some(sink);
                }
                if is_never(self, src) {
                    return Some(sink);
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
    fn no_markers(&self) -> bool {
        self.map.types().all(|ty| {
            if matches!(ty.kind, TypeKind::Marker) {
                let tyref = self.bir.typeref(&self.map.bir(ty.id).unwrap());
                panic!("marker type found: {:?}\n{:#?}", ty.id, tyref);
            } else {
                true
            }
        })
    }

    fn add_based_ty(&mut self, kind: BasedTypeKind, ty: ID) -> ID {
        debug_assert_eq!(self.map.kind(ty), Kind::Type);
        let id = match kind {
            // TODO should this use the type's defining namespace instead of the global one?
            BasedTypeKind::Pointer => self
                .global_ns_mut()
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
            (based_ty.based_on == ty && based_ty.kind == kind)
                .then_some(based_ty.id)
        })
    }

    fn get_based_ty(&mut self, ty: ID, kind: BasedTypeKind) -> ID {
        self.find_based_ty(ty, kind)
            .unwrap_or_else(|| self.add_based_ty(kind, ty))
    }
}

pub fn check(bir: &bir::Map) -> Map {
    let mut ck = Checker::new(bir);

    // The algorithm for checking in the presence of modules and possibly
    // out-of-order definitions is follows:
    //
    // 1. Starting from the root, walk the module tree and create empty
    //    modules.
    //
    // 2. For each module, create prototypes for each type. (e.g. `type 'foo'
    //    in module 'bar'`)
    //
    // 3. For each module, create prototypes for each function. (e.g. `fn
    //    'abs(i32) -> i32' in module 'std'`)
    //
    // 4. For each prototype ty, finish checking its body.
    //
    // 5. For each prototype function, finish checking its body.
    //
    // This ensures at each step, enough information is available to continue
    // checking without requiring any particular ordering or nesting.

    create_modules(&mut ck);

    ck.in_ns(ck.global_ns(), |ck| add_builtin_tys(ck));

    let tys = check_prototype_tys(&mut ck, bir);
    let fns = check_prototype_fns(&mut ck, bir);

    for ty in tys {
        check_ty_inner(&mut ck, ty);
    }

    for fn_ in fns {
        let _ = check_fn_inner(&mut ck, fn_);
    }

    if !ck.map.any_errors() {
        debug_assert!(ck.no_markers());
    }

    ck.map
}

fn check_ty_inner(ck: &mut Checker, ty: PrototypeTy) {
    ck.in_ns(ty.id, |ck| {
        let def = ck.bir.typedef(&ck.map.bir(ty.id).unwrap());
        let members = def
            .members
            .iter()
            .map(|m| {
                let member_ty = check_typeref(ck, m.ty(ck.bir));
                ck.current_ns().new_ty_member(&m.ident, member_ty)
            })
            .collect();
        ck.finish_ty_proto(ty, members);
    })
}

fn create_modules(ck: &mut Checker) {
    // Walk the module tree and create an empty module for each
    let root = create_mod_and_children(ck, ck.bir.root_module(), None);
    ck.global_namespace = Some(root);

    fn create_mod_and_children(
        ck: &mut Checker,
        mod_: &bir::Module,
        parent: Option<ID>,
    ) -> ID {
        let m = if let Some(parent) = parent {
            ck.map
                .ns_mut(parent)
                .unwrap()
                .new_module(mod_.ident.as_deref())
        } else {
            ck.map.new_node(Kind::Module)
        };
        ck.map.set_bir(m, mod_.id);

        for mod_ in mod_.modules(ck.bir) {
            create_mod_and_children(ck, mod_, Some(m));
        }
        m
    }
}

fn check_prototype_tys(ck: &mut Checker, bir: &bir::Map) -> Vec<PrototypeTy> {
    let mut prototype_tys =
        Vec::with_capacity(ck.bir.modules().map(|m| m.typedefs.len()).sum());
    for mod_ in bir.modules() {
        for ty in mod_.typedefs(ck.bir) {
            let mod_ = ck.bir_to_id(&ty.mod_);
            let proto =
                ck.in_ns(mod_, |ck| ck.new_ty_proto(ty.id, &ty.identifier));
            prototype_tys.push(proto);
        }
    }
    prototype_tys
}

fn check_prototype_fns(ck: &mut Checker, bir: &bir::Map) -> Vec<PrototypeFn> {
    let mut prototype_fns =
        Vec::with_capacity(ck.bir.modules().map(|m| m.functions.len()).sum());
    for mod_ in bir.modules() {
        for fn_ in mod_.functions(ck.bir) {
            let mod_ = ck.bir_to_id(&fn_.mod_);
            let proto = ck.in_ns(mod_, |ck| {
                let param_types = fn_
                    .parameters(&ck.bir)
                    .map(|param| {
                        let id = check_typeref(ck, param.ty(ck.bir));
                        ck.ty_id(id)
                    })
                    .collect::<Vec<_>>();
                let return_ty = check_typeref(ck, fn_.return_type(ck.bir));
                ck.add_fn_proto(
                    fn_.id,
                    &fn_.identifier,
                    return_ty,
                    param_types,
                    fn_.is_var_args,
                )
            });
            prototype_fns.push(proto);
        }
    }
    prototype_fns
}

fn add_builtin_tys(ck: &mut Checker) {
    fn add_ty(ck: &mut Checker, name: &str, kind: TypeKind) -> ID {
        ck.current_ns().new_ty(Some(name), kind)
    }
    ck.map.builtins.string_type = Some(add_ty(ck, "str", TypeKind::String));
    ck.map.builtins.bool_type =
        Some(add_ty(ck, "bool", TypeKind::Integer { size: 1 }));
    ck.map.builtins.void_type = Some(add_ty(ck, "void", TypeKind::Void));
    ck.map.builtins.index_type =
        Some(add_ty(ck, "i64", TypeKind::Integer { size: 64 }));

    ck.map.builtins.never_type = Some(add_ty(ck, "!", TypeKind::Never));
    add_ty(ck, "i8", TypeKind::Integer { size: 8 });
    add_ty(ck, "i16", TypeKind::Integer { size: 16 });
    add_ty(ck, "i32", TypeKind::Integer { size: 32 });
}

fn check_fn_inner(ck: &mut Checker, proto: PrototypeFn) -> Result<ID, ID> {
    ck.in_ns(proto.id, |ck| {
        let fn_ = ck.bir.fn_(&proto.bir);
        let fn_ty = ck.map.ty(proto.id).unwrap().as_fn_ty();
        let params = fn_
            .parameters(&ck.bir)
            .enumerate()
            .filter_map(|(idx, param)| {
                if idx == fn_ty.parameters.len() {
                    debug_assert!(fn_ty.is_var_args);
                    return None;
                }
                Some(ck.add_param(
                    param.id,
                    &param.identifier,
                    fn_ty.parameters[idx],
                ))
            })
            .collect();

        let ret_id = proto.return_ty;
        let id = ck.finish_fn_proto(proto, params);

        if let Some(body) = fn_.body(ck.bir) {
            let scope_ = check_block(ck, body)?;
            if ck.unify(fn_ty.return_ty, scope_).is_none() {
                let ctx_id = body
                    .return_expr(ck.bir)
                    .map(|expr| expr.id)
                    .or_else(|| body.items(ck.bir).last().map(|item| item.id))
                    .unwrap_or_else(|| body.id);
                ck.set_err(
                    scope_,
                    ErrorKind::Unification,
                    &[ret_id, ck.bir_to_id(&ctx_id)],
                );
            }
        }

        Ok(id)
    })
}

fn check_typeref(ck: &mut Checker, tyref: &bir::TypeRef) -> ID {
    match &tyref.kind {
        bir::TypeRefKind::Void => ck.void_type(),
        bir::TypeRefKind::Named { name } => ck
            .lookup_ref(name)
            .unwrap_or_else(|| ck.err(ErrorKind::UnknownType, tyref.id)),
        bir::TypeRefKind::Pointer { pointee } => {
            let pointee = check_typeref(ck, ck.bir.typeref(&pointee));
            let pointee_ty = ck.ty_id(pointee);
            ck.get_based_ty(pointee_ty, BasedTypeKind::Pointer)
        }
    }
}

fn check_block<'bir>(
    ck: &mut Checker<'bir>,
    scope: &'bir bir::Block,
) -> Result<ID, ID> {
    let id = ck.current_ns().new_block();
    ck.in_ns(id, |ck| {
        for item in scope.items(ck.bir) {
            check_item(ck, item)?;
        }

        let ty = if let Some(expr) = scope.return_expr(ck.bir) {
            let id = check_expr(ck, expr)?;
            ck.ty_id(id)
        } else {
            ck.void_type()
        };
        ck.map.set_ty(id, ty);
        ck.map.set_bir(id, scope.id);
        Ok(id)
    })
}

fn check_item<'bir>(
    ck: &mut Checker<'bir>,
    item: &'bir bir::Item,
) -> Result<ID, ID> {
    match &item.kind {
        bir::ItemKind::Let(id) => check_let(ck, ck.bir.let_(id)),
        bir::ItemKind::Expr(id) => check_expr(ck, ck.bir.expr(id)),
    }
}

fn check_let<'bir>(
    ck: &mut Checker<'bir>,
    let_: &'bir bir::Let,
) -> Result<ID, ID> {
    let tyref = match let_.ty(ck.bir) {
        Some(ty) => check_typeref(ck, ty),
        None => ck.new_marker_ty(),
    };
    let mut ty = ck.ty_id(tyref);
    if let Some(expr) = let_.expr(ck.bir) {
        let expr = check_expr(ck, expr)?;
        match ck.unify(tyref, expr) {
            Some(t) => ty = t,
            None => ck.set_err(expr, ErrorKind::Unification, &[expr, tyref]),
        }
    }
    Ok(ck.add_var(let_.id, &let_.ident, ty))
}

fn check_expr<'bir>(
    ck: &mut Checker<'bir>,
    expr: &'bir bir::Expr,
) -> Result<ID, ID> {
    let expr_id = ck.current_ns().new_node(Kind::Expr);
    let ty = match &expr.kind {
        bir::ExprKind::NameRef { id } => {
            if let Some(name) = ck.lookup_ref(id) {
                ck.map.associate_bir_with_id(expr.id, name);
                return Ok(name);
            } else {
                return Err(ck.err(ErrorKind::UnknownName, expr.id));
            }
        }
        bir::ExprKind::Literal(lit) => {
            let (id, ty) = match ck.bir.lit(lit) {
                bir::Literal::Number(n) => {
                    let ty = ck.new_marker_ty();
                    (ck.map.new_constant(ty, Constant::Int(*n)), ty)
                }
                bir::Literal::Str(s) => {
                    let ty = ck.string_type();
                    (ck.map.new_constant(ty, Constant::Str(s.clone())), ty)
                }
                bir::Literal::Struct(s) => {
                    let ty = ck.lookup_ref(&s.name).ok_or_else(|| {
                        ck.err(ErrorKind::UnknownName, expr.id)
                    })?;
                    (ck.map.new_constant(ty, todo!()), ty)
                }
            };
            ck.map.set_expr_constant(expr_id, id);
            ty
        }
        bir::ExprKind::Call { receiver, operands } => {
            check_call_expr(ck, receiver, operands)?
        }
        bir::ExprKind::Index { receiver, index } => {
            check_index_expr(ck, receiver, index)?
        }
        bir::ExprKind::Op(op) => check_op_expr(ck, op)?,
        bir::ExprKind::Block { scope } => {
            let scope = check_block(ck, ck.bir.block(scope))?;
            ck.ty_id(scope)
        }
        bir::ExprKind::Return { expr } => {
            let id = if let Some(expr) = expr {
                check_expr(ck, ck.bir.expr(expr))?
            } else {
                ck.void_type()
            };
            let return_ty = ck.current_fn().return_ty;
            ck.unify(return_ty, id).unwrap_or_else(|| {
                ck.set_err(
                    expr_id,
                    ErrorKind::Unification,
                    &[return_ty, expr_id],
                );
                ck.never_type()
            })
        }
        bir::ExprKind::Break { label: _ } => ck.never_type(),
        bir::ExprKind::Continue { label: _ } => ck.never_type(),
        bir::ExprKind::Branch {
            condition,
            kind,
            left,
            right,
        } => {
            let cond = check_expr(ck, ck.bir.expr(condition))?;
            if ck.unify(ck.bool_type(), cond).is_none() {
                ck.set_err(cond, ErrorKind::Unification, &[cond, cond]);
            }
            let left_scope = check_block(ck, ck.bir.block(left))?;
            match kind {
                bir::BranchKind::If => ck.void_type(),
                bir::BranchKind::IfElse => {
                    let right_scope =
                        check_block(ck, ck.bir.block(&right.unwrap()))?;
                    match ck.unify(left_scope, right_scope) {
                        Some(ty) => ty,
                        None => {
                            ck.set_err(
                                cond,
                                ErrorKind::Unification,
                                &[left_scope, right_scope],
                            );
                            ck.ty_id(left_scope)
                        }
                    }
                }
            }
        }
        bir::ExprKind::Loop { body, kind } => {
            check_block(ck, ck.bir.block(body))?;
            match kind {
                bir::LoopKind::Loop => ck.never_type(),
                bir::LoopKind::While => ck.void_type(),
            }
        }
        bir::ExprKind::Cast { val, to } => {
            let expr = check_expr(ck, ck.bir.expr(val))?;
            let tyref = check_typeref(ck, ck.bir.typeref(to));
            if ck.map.ty(expr).unwrap().is_marker() {
                ck.unify(tyref, expr);
            }
            ck.ty_id(tyref)
        }
    };
    ck.map.set_ty(expr_id, ty);
    ck.map.set_bir(expr_id, expr.id);
    Ok(expr_id)
}

fn check_index_expr(
    ck: &mut Checker,
    receiver: &bir::ID,
    index: &bir::ID,
) -> Result<ID, ID> {
    let receiver_id = check_expr(ck, ck.bir.expr(receiver))?;
    let expr_id = check_expr(ck, ck.bir.expr(index))?;
    if ck.unify(ck.index_type(), expr_id).is_none() {
        ck.set_err(
            expr_id,
            ErrorKind::InvalidIndexType,
            &[receiver_id, expr_id],
        );
    }
    let receiver_ty = ck.map.ty(receiver_id).unwrap();
    if receiver_ty.is_ptr() {
        Ok(receiver_ty.pointee())
    } else {
        Err(ck.err(ErrorKind::InvalidPointeeType, *receiver))
    }
}

fn lookup_or_err(
    ck: &mut Checker,
    name: &bir::ID,
    bir: &bir::ID,
) -> Result<ID, ID> {
    let ns = ck.current_ns().id;
    ck.lookup_in(ns, name)
        .ok_or_else(|| ck.err(ErrorKind::UnknownName, *bir))
}

fn check_call_expr(
    ck: &mut Checker,
    receiver: &bir::ID,
    operands: &Vec<bir::ID>,
) -> Result<ID, ID> {
    let called_fn = ck
        .bir
        .expr(receiver)
        .name(ck.bir)
        .ok_or_else(|| ck.err(ErrorKind::InvalidCallReceiver, *receiver))?;
    let fn_id = lookup_or_err(ck, &called_fn.id, receiver)?;
    ck.map.associate_bir_with_id(*receiver, fn_id);

    let fn_ty = ck
        .map
        .ty(fn_id)
        .unwrap()
        .into_fn_ty()
        .ok_or_else(|| ck.err(ErrorKind::CallToNonFnType, *receiver))?;
    let args = operands
        .iter()
        .map(|id| check_expr(ck, ck.bir.expr(id)))
        .collect::<Result<Vec<_>, ID>>()?;
    let mut call_sig_match = args.len() == fn_ty.parameters.len()
        || fn_ty.is_var_args && args.len() > fn_ty.parameters.len();
    if call_sig_match {
        // Call sig length matches at least -- now check if the params match as well.
        for (idx, param_ty) in fn_ty.parameters.iter().enumerate() {
            let arg = args[idx];
            if ck.unify(*param_ty, arg).is_none() {
                ck.set_err(arg, ErrorKind::Unification, &[arg, *param_ty]);
                call_sig_match = false;
            }
        }
    }
    if !call_sig_match {
        return Err(ck.err(ErrorKind::UnknownCall, *receiver));
    }
    ck.map.add_caller(ck.current_fn().id, fn_id);
    Ok(fn_ty.return_ty)
}

fn check_op_expr(ck: &mut Checker, op: &bir::Op) -> Result<ID, ID> {
    match (op.fixity, op.kind) {
        (bir::OpFixity::Infix, kind) => match kind {
            bir::OpKind::Plus
            | bir::OpKind::Minus
            | bir::OpKind::Multiply
            | bir::OpKind::Divide => {
                let lhs = check_expr(ck, ck.bir.expr(&op.operands[0]))?;
                let rhs = check_expr(ck, ck.bir.expr(&op.operands[1]))?;
                Ok(ck.unify(lhs, rhs).unwrap_or_else(|| {
                    // TODO this should be set on the expr itself
                    ck.set_err(lhs, ErrorKind::Unification, &[lhs, rhs]);
                    ck.ty_id(lhs)
                }))
            }
            // Need to add checks
            bir::OpKind::FieldAccess => check_field_access(ck, op),
            bir::OpKind::LessThan
            | bir::OpKind::LessThanEquals
            | bir::OpKind::GreaterThan
            | bir::OpKind::GreaterThanEquals
            | bir::OpKind::Equals
            | bir::OpKind::NotEquals => {
                let lhs = check_expr(ck, ck.bir.expr(&op.operands[0]))?;
                let rhs = check_expr(ck, ck.bir.expr(&op.operands[1]))?;
                match ck.unify(lhs, rhs) {
                    Some(ty) if ck.map.ty(ty).unwrap().is_marker() => {
                        ck.set_err(lhs, ErrorKind::Unification, &[lhs, rhs]);
                    }
                    None => {
                        // TODO this should be set on the expr itself
                        ck.set_err(lhs, ErrorKind::Unification, &[lhs, rhs]);
                    }
                    _ => {}
                }
                Ok(ck.bool_type())
            }
            bir::OpKind::Assignment => {
                let dst = check_expr(ck, ck.bir.expr(&op.operands[0]))?;
                if ck.map.param(dst).is_some() {
                    ck.set_err(dst, ErrorKind::ParamAssignment, &[dst]);
                }
                let src = check_expr(ck, ck.bir.expr(&op.operands[1]))?;
                if ck.unify(dst, src).is_none() {
                    // TODO this should be set on the expr itself
                    ck.set_err(dst, ErrorKind::Unification, &[dst, src]);
                }
                Ok(ck.void_type())
            }
        },
        (bir::OpFixity::Postfix, _) => todo!(),
        (bir::OpFixity::Prefix, _) => todo!(),
    }
}

fn check_field_access(ck: &mut Checker, op: &bir::Op) -> Result<ID, ID> {
    let receiver = check_expr(ck, ck.bir.expr(&op.operands[0]))?;
    let receiver_ty = match ck.map.ty(receiver) {
        Some(ty) if ty.is_aggregate() => ty.id,
        Some(ty) if ty.is_ptr() => ty.pointee(),
        _ => {
            return Err(ck.err(ErrorKind::InvalidFieldReceiver, op.operands[0]));
        }
    };
    ck.in_ns(receiver_ty, |ck| {
        ck.check_namespace_parents = false;
        let result = check_expr(ck, ck.bir.expr(&op.operands[1]))?;
        ck.check_namespace_parents = true;
        Ok(ck.ty_id(result))
    })
}
