use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use crate::types::*;

static NAME_ID: AtomicU32 = AtomicU32::new(0);
fn get_unique_name(mut n: String) -> String {
    let id = NAME_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    n.push_str(&id.to_string());
    n
}

pub struct Builder {
    pub(crate) map: Map,
    pub(crate) current_module: Option<ID>,
    current_function: Option<ID>,
    scope_stack: ScopeStack,

    string_literals: HashMap<String, ID>,
    number_literals: HashMap<usize, ID>,
}

struct ScopeStack {
    entries: Vec<ScopeStackEntry>,
}

struct ScopeStackEntry {
    pub id: ID,
    pub kind: BlockKind,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn top(&mut self) -> Option<ID> {
        self.entries.last_mut().map(|scope| scope.id)
    }

    fn last_of_kind(&self, kind: BlockKind) -> Option<ID> {
        self.entries.iter().rev().find_map(|entry| {
            if entry.kind == kind {
                Some(entry.id)
            } else {
                None
            }
        })
    }

    fn push(&mut self, id: ID, kind: BlockKind) {
        self.entries.push(ScopeStackEntry { id, kind });
    }

    fn pop(&mut self) {
        self.entries.pop();
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            map: Map::default(),
            current_module: None,
            current_function: None,
            scope_stack: ScopeStack::new(),
            string_literals: HashMap::default(),
            number_literals: HashMap::default(),
        }
    }

    pub fn finish(self) -> Map {
        self.map
    }

    fn new_node(&mut self, kind: Kind) -> ID {
        let id = ID(self.map.nodes.len());
        self.map.nodes.push(kind);
        id
    }

    pub fn set_ast(&mut self, id: ID, ast: Arc<dyn ast::Node>) {
        self.map.ast.insert(id, ast.clone());
    }

    pub fn new_module(&mut self, ast: Option<Arc<dyn ast::Node>>) -> ID {
        let id = self.new_node(Kind::Module);
        if self.map.modules.len() == 0 {
            self.map.root_module = Some(id);
        }
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        self.map.modules.insert(id, Module::new(id));
        id
    }

    pub fn root_module(&self) -> Option<ID> {
        self.map.root_module
    }

    pub fn new_typedef(
        &mut self,
        identifier: &str,
        members: Vec<TypeMember>,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::TypeDef);
        self.map.typedefs.insert(
            id,
            TypeDef {
                id,
                identifier: identifier.to_string(),
                members,
                mod_: self.current_module.unwrap(),
            },
        );
        self.current_module().typedefs.push(id);
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn new_import(&mut self, name: String) -> ID {
        let id = self.new_node(Kind::Import);
        let module = self.current_module().id;
        self.map.imports.insert(
            id,
            Import {
                id,
                parent: module,
                name,
            },
        );
        self.current_module().imports.push(id);
        id
    }

    pub fn new_function(
        &mut self,
        identifier: &str,
        return_type: ID,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        debug_assert!(self.map.typerefs.contains_key(&return_type));

        let id = self.new_node(Kind::Function);
        let func = Function::new(
            id,
            self.current_module().id,
            identifier.to_string(),
            return_type,
        );
        self.map.functions.insert(id, func);

        self.current_module().functions.push(id);
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }

        id
    }

    pub fn new_param(
        &mut self,
        identifier: String,
        ty: ID,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        debug_assert!(self.map.typerefs.contains_key(&ty));

        let id = self.new_node(Kind::Parameter);
        let param =
            Parameter::new(id, self.current_function().id, ty, identifier);
        self.map.params.insert(id, param);
        self.current_function().parameters.push(id);
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn new_name(
        &mut self,
        segments: Vec<String>,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::Name);
        self.map.names.insert(id, Name { id, segments });
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn new_typeref(
        &mut self,
        kind: TypeRefKind,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::TypeRef);
        self.map.typerefs.insert(id, TypeRef { id, kind });
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn new_block(&mut self, kind: BlockKind, label: Option<String>) -> ID {
        let id = self.new_node(Kind::Block);
        let parent = self.scope_stack.top();
        let function = self.current_function.unwrap();
        let scope =
            Block::new(id, kind, parent, function, label.map(get_unique_name));
        self.map.blocks.insert(id, scope);
        id
    }

    pub fn in_new_scope(
        &mut self,
        label: Option<String>,
        kind: BlockKind,
        f: impl FnOnce(&mut Self),
    ) -> ID {
        let new_scope = self.new_block(kind, label);
        self.scope_stack.push(new_scope, kind);
        f(self);
        self.scope_stack.pop();
        new_scope
    }

    pub fn new_item(
        &mut self,
        kind: ItemKind,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::Item);
        self.current_scope().items.push(id);
        if let ItemKind::Let(id) = kind {
            self.current_scope().lets.push(id);
        }
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        self.map.items.insert(id, Item { id, kind });
        id
    }

    pub fn new_let_item(
        &mut self,
        name: String,
        ty: Option<ID>,
        expr: Option<ID>,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::Let);
        self.map.lets.insert(
            id,
            Let {
                id,
                ident: name,
                ty,
                expr,
            },
        );
        self.new_item(ItemKind::Let(id), ast)
    }

    pub fn new_expr(
        &mut self,
        kind: ExprKind,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = self.new_node(Kind::Expr);
        self.map.exprs.insert(id, Expr { id, kind });
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn new_expr_item(
        &mut self,
        kind: ExprKind,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let expr_id = self.new_expr(kind, ast.clone());
        let id = self.new_item(ItemKind::Expr(expr_id), ast);
        id
    }

    pub fn new_literal(
        &mut self,
        literal: Literal,
        ast: Option<Arc<dyn ast::Node>>,
    ) -> ID {
        let id = match &literal {
            Literal::Str(s) => {
                if let Some(&id) = self.string_literals.get(s) {
                    return id;
                } else {
                    let id = self.new_node(Kind::Literal);
                    self.string_literals.insert(s.to_string(), id);
                    id
                }
            }
            Literal::Number(n) => {
                if let Some(&id) = self.number_literals.get(n) {
                    return id;
                } else {
                    let id = self.new_node(Kind::Literal);
                    self.number_literals.insert(*n, id);
                    id
                }
            }
            Literal::Struct(..) => self.new_node(Kind::Literal),
        };
        self.map.literals.insert(id, literal);
        if let Some(ast) = ast {
            self.set_ast(id, ast);
        }
        id
    }

    pub fn current_module(&mut self) -> &mut Module {
        self.current_module
            .map(|id| self.map.modules.get_mut(&id).unwrap())
            .unwrap()
    }

    pub fn add_module_child(&mut self, parent: ID, child: ID) {
        debug_assert_eq!(self.map.mod_(&child).parent, None);
        self.map.mod_mut(&child).parent = Some(parent);
        self.map.mod_mut(&parent).modules.push(child);
    }

    pub fn current_function(&mut self) -> &mut Function {
        self.current_function
            .map(|id| self.map.functions.get_mut(&id).unwrap())
            .unwrap()
    }

    pub fn current_scope(&mut self) -> &mut Block {
        self.scope_stack
            .top()
            .map(|id| self.map.blocks.get_mut(&id).unwrap())
            .unwrap()
    }

    pub fn last_loop_label(&self) -> String {
        let id = self.scope_stack.last_of_kind(BlockKind::Loop).unwrap();
        debug_assert!(self.map.kind(&id) == Kind::Block);
        self.map.blocks[&id].label.clone().unwrap()
    }

    pub fn set_current_module(&mut self, id: ID) {
        debug_assert!(self.map.kind(&id) == Kind::Module);
        self.current_module = Some(id);
    }

    pub fn set_current_function(&mut self, id: ID) {
        debug_assert!(self.map.kind(&id) == Kind::Function);
        self.current_function = Some(id);
    }
}
