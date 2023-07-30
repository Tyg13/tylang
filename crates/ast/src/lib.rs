use std::sync::Arc;

use cst::syntax;
use cst::T;

pub use cst::green::SyntaxKind;
use cst::green::SyntaxKind::*;

pub trait Node {
    fn cast(node: syntax::Node) -> Option<Arc<Self>>
    where
        Self: Sized;
    fn syntax(&self) -> &syntax::Node;
    fn text(&self) -> String {
        self.syntax().text()
    }
    fn kind(&self) -> SyntaxKind {
        self.syntax().kind()
    }
}

pub trait Token {
    fn cast(token: syntax::Token) -> Option<Self>
    where
        Self: Sized;
    fn syntax(&self) -> &syntax::Token;
    fn text(&self) -> &str {
        self.syntax().text()
    }
}

pub trait HasKind {
    const KIND: SyntaxKind;
}

#[rustfmt::skip]
mod grammar {
    use super::*;

    macro_rules! decl_node_enum {
        (enum $EnumNode:ident { $($Variant:ident ($getter_name:ident),)+ }) => {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub enum $EnumNode {
                $(
                    $Variant(Arc<$Variant>)
                ),*
            }
            impl Node for $EnumNode {
                #[inline]
                fn cast(node: syntax::Node) -> Option<Arc<Self>> {
                    match node.green.kind {
                        $($Variant::KIND => Some(Arc::new(Self::$Variant(Arc::new(
                            $Variant { syntax: node }
                        )))),)+
                        _ => None
                    }
                }

                #[inline]
                fn syntax(&self) -> &syntax::Node {
                    match self {
                        $(Self::$Variant(data) => data.syntax()),*
                    }
                }
            }
            impl $EnumNode {
                $(
                    #[inline]
                    pub fn $getter_name(&self) -> Option<Arc<$Variant>> {
                        match self {
                            Self::$Variant(data) => Some(data.clone()),
                            _ => None,
                        }
                    }
                )+

                #[inline]
                pub fn to_string_indented(&self, indent: usize) -> String {
                    match self {
                        $(Self::$Variant(data) => data.to_string_indented(indent),)*
                    }
                }
            }

            impl std::fmt::Display for $EnumNode {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        $(Self::$Variant(data) => data.fmt(f),)*
                    }
                }
            }
        }
    }

    macro_rules! decl_token_enum {
        (enum $EnumToken:ident { $($Variant:ident ($getter_name:ident),)+ }) => {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub enum $EnumToken {
                $(
                    $Variant($Variant)
                ),*
            }
            impl Token for $EnumToken {
                #[inline]
                fn cast(token: syntax::Token) -> Option<Self> {
                    match token.green.kind {
                        $($Variant::KIND => Some(Self::$Variant(
                            $Variant { syntax: token }
                        )),)+
                        _ => None,
                    }
                }

                #[inline]
                fn syntax(&self) -> &syntax::Token {
                    match self {
                        $(Self::$Variant(data) => data.syntax()),*
                    }
                }
            }
            impl $EnumToken {
                $(
                    #[inline]
                    pub fn $getter_name(&self) -> Option<$Variant> {
                        match self {
                            Self::$Variant(data) => Some(data.clone()),
                            _ => None,
                        }
                    }
                )+

                #[inline]
                pub fn to_string_indented(&self, indent: usize) -> String {
                    match self {
                        $(Self::$Variant(data) => data.to_string_indented(indent),)*
                    }
                }
            }

            impl std::fmt::Display for $EnumToken {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    match self {
                        $(Self::$Variant(data) => data.fmt(f),)*
                    }
                }
            }
        }
    }

    macro_rules! decl_node_child_impl {
        () => {};
        (($node_fn:ident: Node<$Child:ty>) $($rest:tt)*) => {
            #[inline] pub fn $node_fn(&self) -> Option<Arc<$Child>> {
                self.syntax().children().find_map(<$Child as Node>::cast)
            }
            decl_node_child_impl!($($rest)*);
        };
        (($nth_node_fn:ident: NthNode<$index:literal, $Child:ty>) $($rest:tt)*) => {
            #[inline] pub fn $nth_node_fn(&self) -> Option<Arc<$Child>> {
                self.syntax().children().nth($index).and_then(<$Child as Node>::cast)
            }
            decl_node_child_impl!($($rest)*);
        };
        (($node_list_fn:ident: NodeList<$Child:ty>) $($rest:tt)*) => {
            #[inline] pub fn $node_list_fn(&self) -> impl Iterator<Item = Arc<$Child>> + '_ {
                self.syntax().children().filter_map(<$Child as Node>::cast)
            }
            decl_node_child_impl!($($rest)*);
        };
        (($node_list_fn:ident: NodeList<$index:literal, $Child:ty>) $($rest:tt)*) => {
            #[inline] pub fn $node_list_fn(&self) -> impl Iterator<Item = Arc<$Child>> + '_ {
                self.syntax().children().skip($index).filter_map(<$Child as Node>::cast)
            }
            decl_node_child_impl!($($rest)*);
        };
        (($token_fn:ident: Token<$Token:ident>) $($rest:tt)*) => {
            #[inline] pub fn $token_fn(&self) -> Option<$Token> {
                self.syntax()
                    .children_with_tokens()
                    .filter_map(|child| child.into_token())
                    .find_map(|token| $Token::cast(token))
            }
            decl_node_child_impl!($($rest)*);
        };
    }

    macro_rules! none_to_string {
        ($name:ty, $indent:expr) => {
            format!("\n{}{}: None", str::repeat(" ", $indent), stringify!($name))
        }
    }

    macro_rules! node_child_to_string {
        ($node:expr, $s:expr, $indent:expr$(,)?) => {};
        ($node:expr, $s:expr, $indent:expr, ($f:ident: Node<$Child:ty>) $($rest:tt)*) => {
            if let Some(child) = $node.children().find_map(<$Child as Node>::cast) {
                $s.push_str(&format!("\n{}", child.to_string_indented($indent + 2)));
            } else {
                $s.push_str(&none_to_string!($Child, $indent + 2));
            }
            node_child_to_string!($node, $s, $indent, $($rest)*);
        };
        ($node:expr, $s:expr, $indent:expr, ($f:ident: NthNode<$index:literal, $Child:ty>) $($rest:tt)*) => {
            if let Some(child) = $node.children().nth($index).and_then(<$Child as Node>::cast) {
                $s.push_str(&format!("\n{}", child.to_string_indented($indent + 2)));
            } else {
                $s.push_str(&none_to_string!($Child, $indent + 2));
            }
            node_child_to_string!($node, $s, $indent, $($rest)*);
        };
        ($node:expr, $s:expr, $indent:expr, ($f:ident: NodeList<$Child:ty>) $($rest:tt)*) => {
            for child in $node.children().filter_map(<$Child as Node>::cast) {
                $s.push_str(&format!("\n{}", child.to_string_indented($indent + 2)));
            }
            node_child_to_string!($node, $s, $indent, $($rest)*);
        };
        ($node:expr, $s:expr, $indent:expr, ($f:ident: NodeList<$index:literal, $Child:ty>) $($rest:tt)*) => {
            for child in $node.children().skip($index).filter_map(<$Child as Node>::cast) {
                $s.push_str(&format!("\n{}", child.to_string_indented($indent + 2)));
            }
            node_child_to_string!($node, $s, $indent, $($rest)*);
        };
        ($node:expr, $s:expr, $indent:expr, ($f:ident: Token<$Token:ident>) $($rest:tt)*) => {
            if let Some(child) = $node
                .children_with_tokens()
                .filter_map(|child| child.into_token())
                .find_map(|token| $Token::cast(token)) {
                $s.push_str(&format!("\n{}", child.to_string_indented($indent + 2)));
            } else {
                $s.push_str(&none_to_string!($Token, $indent + 2));
            }
            node_child_to_string!($node, $s, $indent, $($rest)*);
        };
    }

    macro_rules! decl_node {
        (struct $Node:ident: $KIND:ident { $($child_specifier:tt)* }) => {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $Node {
                syntax: syntax::Node,
            }
            impl HasKind for $Node {
                const KIND: SyntaxKind = $KIND;
            }
            impl Node for $Node {
                #[inline]
                fn cast(node: syntax::Node) -> Option<Arc<Self>> {
                    match node.green.kind {
                        $KIND => Some(Arc::new($Node { syntax: node })),
                        _ => None,
                    }
                }
                #[inline]
                fn syntax(&self) -> &syntax::Node {
                    &self.syntax
                }
            }
            impl $Node {
                decl_node_child_impl!($($child_specifier)*);

                #[inline]
                pub fn to_string_indented(&self, indent: usize) -> String {
                    let mut result = String::new();
                    result.push_str(&format!("{}{}:", str::repeat(" ", indent), stringify!($Node)));
                    node_child_to_string!(self.syntax(), result, indent, $($child_specifier)*);
                    result
                }
            }
            impl std::fmt::Display for $Node {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(&self.to_string_indented(0))
                }
            }
        };
    }

    macro_rules! decl_token {
        (struct $Token:ident: $($KIND:tt)*) => {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct $Token {
                syntax: syntax::Token,
            }
            impl HasKind for $Token {
                const KIND: SyntaxKind = $($KIND)*;
            }
            impl Token for $Token {
                #[inline]
                fn cast(token: syntax::Token) -> Option<Self> {
                    match token.green.kind {
                        #[allow(unused_parens)]
                        $($KIND)* => Some($Token { syntax: token }),
                        _ => None,
                    }
                }

                #[inline]
                fn syntax(&self) -> &syntax::Token {
                    &self.syntax
                }
            }
            impl $Token {
                #[inline]
                pub fn to_string_indented(&self, indent: usize) -> String {
                    format!("{}{}: {}",
                        str::repeat(" ", indent),
                        stringify!($Token),
                        self.syntax().text()
                    )
                }
            }
            impl std::fmt::Display for $Token {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(&self.to_string_indented(0))
                }
            }
        };
    }

    decl_node!(struct Module: MODULE {
        (mod_kw  : Token    <ModKw     >)
        (name    : Token    <Ident     >)
        (l_curly : Token    <LeftCurly >)
        (items   : NodeList <Item      >)
        (r_curly : Token    <RightCurly>)
    });

    impl Module {
        pub fn inner_mods(&self) -> impl Iterator<Item = Arc<Module>> + '_ {
            self.items().filter_map(|i| i.mod_())
        }
        pub fn imports(&self) -> impl Iterator<Item = Arc<Import>> + '_ {
            self.items().filter_map(|i| i.import())
        }
        pub fn types(&self) -> impl Iterator<Item = Arc<TypeItem>> + '_ {
            self.items().filter_map(|i| i.type_())
        }
        pub fn fns(&self) -> impl Iterator<Item = Arc<FnDef>> + '_ {
            self.items().filter_map(|i| i.fn_())
        }
    }

    decl_node_enum!(enum Item {
        Module(mod_),
        Import(import),
        FnDef(fn_),
        Let(let_),
        ExprItem(expr_item),
        TypeItem(type_),
    });

    decl_node!(struct Import: IMPORT_ITEM {
        (import_kw : Token<ImportKw >)
        (ident     : Token<Ident    >)
        (semi      : Token<SemiColon>)
    });

    decl_node!(struct Let: LET_ITEM {
        (let_kw    : Token  <LetKw>    )
        (name      : Node   <Name>     )
        (colon     : Token  <Colon>    )
        (type_     : Node   <Type>     )
        (equals    : Token  <Equals>   )
        (expr      : Node   <Expr>     )
        (semicolon : Token  <SemiColon>)
    });
    decl_node!(struct FnDef: FN_ITEM {
        (fn_kw      : Token<FnKw>     )
        (name       : Node <Name>     )
        (param_list : Node <ParamList>)
        (arrow      : Token<DashArrow>)
        (return_ty  : Node <Type>     )
        (extern_    : Token<ExternKw> )
        (body       : Node <Expr >    )
        (semicolon  : Token<SemiColon>)
    });
    decl_node!(struct ExprItem: EXPR_ITEM {
        (expr      : Node <Expr     >)
        (semicolon : Token<SemiColon>)
    });

    decl_node!(struct TypeItem: TYPE_ITEM {
        (type_kw    : Token<TypeKw       >)
        (ident      : Token<Ident        >)
        (left_curly : Token<LeftCurly    >)
        (members    : NodeList<TypeMember>)
        (right_curly: Token<RightCurly   >)
    });

    decl_node!(struct TypeMember: TYPE_MEMBER {
        (ident    : Token<Ident    >)
        (semicolon: Token<SemiColon>)
        (type_    : Node <Type     >)
        (comma    : Token<Comma    >)
    });

    decl_node!(struct ParamList: PARAM_LIST {
        (l_paren : Token   <LeftParen> )
        (params  : NodeList<Param>     )
        (r_paren : Token   <RightParen>)
    });
    decl_node_enum!(enum Param {
        NamedParam(param),
        VaParam(va_param),
    });
    decl_node!(struct NamedParam: PARAM {
        (name  : Node <Name> )
        (colon : Token<Colon>)
        (type_ : Node <Type> )
    });
    decl_node!(struct VaParam: VA_PARAM {
        (ellipsis: Token<Ellipsis>)
    });

    decl_node_enum!(enum Name {
        BasicName(basic_name),
        ScopedName(scoped_name),
    });
    decl_node!(struct BasicName: NAME {
        (ident: Token<Ident>)
    });
    decl_node!(struct ScopedName: SCOPED_NAME {
        (head       : Token<Ident     >)
        (coloncolon : Token<ColonColon>)
        (tail       : Node <Name      >)
    });


    decl_node_enum!(enum Type {
        BasicType(basic_type),
        PointerType(pointer_type),
    });

    decl_node!(struct BasicType: BASIC_TYPE {
        (name: Node<Name>)
    });
    decl_node!(struct PointerType: POINTER_TYPE {
        (pointee: Node<Type>)
    });

    decl_node_enum!(enum Expr {
        Literal(literal),
        StructLiteral(struct_literal),
        NameRef(name_ref),
        PrefixExpr(prefix_op),
        BinExpr(bin_op),
        Group(group),
        Block(block),
        Return(return_),
        Break(break_),
        Continue(continue_),
        Cast(cast_),
        CallExpr(call_expr),
        IndexExpr(index_expr),
        IfExpr(if_expr),
        LoopExpr(loop_expr),
        WhileExpr(while_expr),
    });

    decl_node!(struct Literal: LITERAL {
        (value: Token<LiteralValue>)
    });

    decl_token_enum!(enum LiteralValue {
        Number(number),
        Str(string),
    });

    decl_node!(struct StructLiteral: STRUCT_LITERAL {
        (name       : Node <Name      >)
        (left_curly : Token<LeftCurly >)
        (right_curly: Token<RightCurly>)
    });

    decl_node!(struct NameRef: NAME_REF {
        (name: Node<Name>)
    });
    decl_node!(struct PrefixExpr: PREFIX_EXPR {
        (op      : Token<PrefixOp>)
        (operand : Node <Expr>)
    });
    decl_node!(struct BinExpr: BIN_EXPR {
        (lhs : NthNode<0, Expr >)
        (op  : Token  <   BinOp>)
        (rhs : NthNode<1, Expr >)
    });
    decl_node!(struct Group: PAREN_EXPR {
        (inner: Node<Expr>)
    });
    decl_node!(struct Block: BLOCK_EXPR {
        (l_curly : Token   <LeftCurly >)
        (items   : NodeList<Item      >)
        (expr    : Node    <Expr      >)
        (r_curly : Token   <RightCurly>)
    });
    decl_node!(struct Return: RETURN_EXPR {
        (return_kw : Token<ReturnKw>)
        (expr      : Node <Expr    >)
    });
    decl_node!(struct Break: BREAK_EXPR {
        (break_kw  : Token<BreakKw>)
    });
    decl_node!(struct Continue: CONTINUE_EXPR {
        (continue_kw  : Token<ContinueKw>)
    });
    decl_node!(struct Cast: AS_EXPR {
        (expr   : Node<Expr>)
        (as_kw  : Token<AsKw>)
        (ty     : Node<Type>)
    });
    decl_node!(struct CallExpr: CALL_EXPR {
        (receiver  : NthNode <0, Expr   >)
        (l_paren   : Token   <LeftParen >)
        (arguments : NodeList<1, Expr   >)
        (r_paren   : Token   <RightParen>)
    });
    decl_node!(struct IndexExpr: INDEX_EXPR {
        (receiver : NthNode <0, Expr   >)
        (l_paren  : Token   <LeftParen >)
        (index    : NthNode <1, Expr   >)
        (r_paren  : Token   <RightParen>)
    });
    decl_node!(struct IfExpr: IF_EXPR {
        (if_kw     : Token   <IfKw    >)
        (condition : NthNode <0, Expr >)
        (then      : NthNode <1, Block>)
        (else_kw   : Token   <ElseKw  >)
        (alternate : NthNode <2, Block>)
    });
    decl_node!(struct LoopExpr: LOOP_EXPR {
        (loop_kw   : Token <LoopKw>)
        (body      : Node  <Block >)
    });
    decl_node!(struct WhileExpr: WHILE_EXPR {
        (while_kw   : Token <WhileKw>)
        (condition  : Node  <Expr>)
        (body       : Node  <Block >)
    });

    decl_token_enum!(enum PrefixOp {
        Plus(plus),
        Minus(minus),
        Star(star),
    });

    decl_token_enum!(enum BinOp {
        Plus(plus),
        Minus(minus),
        Star(star),
        Slash(slash),
        Dot(dot),
        Arrow(arrow),
        Gt(gt),
        Lt(lt),
        Eq(eq),
        Ne(ne),
        Lte(lte),
        Gte(gte),
        And(and),
        Assign(assign),
        ColonColon(colon_colon),
    });

    decl_token!(struct Plus       : T![+]);
    decl_token!(struct Minus      : T![-]);
    decl_token!(struct Star       : T![*]);
    decl_token!(struct Slash      : T![/]);
    decl_token!(struct Dot        : T![.]);
    decl_token!(struct Gt         : T![>]);
    decl_token!(struct Lt         : T![<]);
    decl_token!(struct Eq         : T![==]);
    decl_token!(struct Ne         : T![!=]);
    decl_token!(struct Lte        : T![>=]);
    decl_token!(struct Gte        : T![<=]);
    decl_token!(struct And        : T![&&]);
    decl_token!(struct Assign     : T![=]);
    decl_token!(struct ColonColon : T![::]);
    decl_token!(struct Arrow      : T![->]);

    decl_token!(struct Ident      : IDENT);
    decl_token!(struct Number     : NUMBER);
    decl_token!(struct Str        : STRING);
    decl_token!(struct LeftParen  : T!['(']);
    decl_token!(struct RightParen : T![')']);
    decl_token!(struct LeftCurly  : T!['{']);
    decl_token!(struct RightCurly : T!['}']);
    decl_token!(struct DashArrow  : T![->]);
    decl_token!(struct Ellipsis   : T![...]);
    decl_token!(struct SemiColon  : T![;]);
    decl_token!(struct Comma      : T![,]);
    decl_token!(struct Colon      : T![:]);
    decl_token!(struct Equals     : T![=]);
    decl_token!(struct ModKw      : T![mod]);
    decl_token!(struct ImportKw   : T![import]);
    decl_token!(struct TypeKw     : T![type]);
    decl_token!(struct FnKw       : T![fn]);
    decl_token!(struct LetKw      : T![let]);
    decl_token!(struct ReturnKw   : T![return]);
    decl_token!(struct BreakKw    : T![break]);
    decl_token!(struct ContinueKw : T![continue]);
    decl_token!(struct AsKw       : T![as]);
    decl_token!(struct ExternKw   : T![extern]);
    decl_token!(struct IfKw       : T![if]);
    decl_token!(struct ElseKw     : T![else]);
    decl_token!(struct LoopKw     : T![loop]);
    decl_token!(struct WhileKw    : T![while]);
}

pub use grammar::*;

#[cfg(test)]
mod tests {
    use expect_test::{expect, Expect};

    use super::*;
    use parser::{grammar::EntryPoint, Output};

    fn parse_with_entry(s: &str, entry: EntryPoint) -> syntax::Node {
        let Output { root, errors } =
            parser::parse_str_from_entry(s.trim(), entry);
        eprintln!("{}", root);
        eprintln!("{:?}", errors);
        assert_eq!(errors.len(), 0);
        root
    }

    fn check_module(s: &str, expected: Expect) {
        let expr =
            Module::cast(parse_with_entry(s, EntryPoint::Module)).unwrap();
        expected.assert_eq(&expr.to_string());
    }

    fn check_block(s: &str, expected: Expect) {
        let expr = Expr::cast(parse_with_entry(s, EntryPoint::Block)).unwrap();
        expected.assert_eq(&expr.to_string());
    }

    fn check_expr(s: &str, expected: Expect) {
        let expr =
            Expr::cast(parse_with_entry(s, EntryPoint::Expression)).unwrap();
        expected.assert_eq(&expr.to_string());
    }

    #[test]
    fn module() {
        check_module(
            r#"
let foo:char;
fn bar() {}
       "#,
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  Let:
                    LetKw: let
                    BasicName:
                      Ident: foo
                    Colon: :
                    BasicType:
                      BasicName:
                        Ident: char
                    Equals: None
                    Expr: None
                    SemiColon: ;
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: bar
                    ParamList:
                      LeftParen: (
                      RightParen: )
                    DashArrow: None
                    Type: None
                    ExternKw: None
                    Block:
                      LeftCurly: {
                      Expr: None
                      RightCurly: }
                    SemiColon: None
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn let_no_expr() {
        check_module(
            "let foo: int;",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  Let:
                    LetKw: let
                    BasicName:
                      Ident: foo
                    Colon: :
                    BasicType:
                      BasicName:
                        Ident: int
                    Equals: None
                    Expr: None
                    SemiColon: ;
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn let_with_expr() {
        check_module(
            "let foo: int = 10;",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  Let:
                    LetKw: let
                    BasicName:
                      Ident: foo
                    Colon: :
                    BasicType:
                      BasicName:
                        Ident: int
                    Equals: =
                    Literal:
                      Number: 10
                    SemiColon: ;
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn multiple_lets() {
        check_block(
            r#"{
    if n <= 1 {
        return n
    }
    let j: i32 = n - 2;
    n
}"#,
            expect![[r#"
                Block:
                  LeftCurly: {
                  ExprItem:
                    IfExpr:
                      IfKw: if
                      BinExpr:
                        NameRef:
                          BasicName:
                            Ident: n
                        Gte: <=
                        Literal:
                          Number: 1
                      Block:
                        LeftCurly: {
                        Return:
                          ReturnKw: return
                          NameRef:
                            BasicName:
                              Ident: n
                        RightCurly: }
                      ElseKw: None
                      Block: None
                    SemiColon: None
                  Let:
                    LetKw: let
                    BasicName:
                      Ident: j
                    Colon: :
                    BasicType:
                      BasicName:
                        Ident: i32
                    Equals: =
                    BinExpr:
                      NameRef:
                        BasicName:
                          Ident: n
                      Minus: -
                      Literal:
                        Number: 2
                    SemiColon: ;
                  NameRef:
                    BasicName:
                      Ident: n
                  RightCurly: }"#]],
        )
    }

    #[test]
    fn fn_no_type_no_body() {
        check_module(
            "fn foo();",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: foo
                    ParamList:
                      LeftParen: (
                      RightParen: )
                    DashArrow: None
                    Type: None
                    ExternKw: None
                    Block: None
                    SemiColon: ;
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn fn_no_type_with_body() {
        check_module(
            "fn foo() {}",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: foo
                    ParamList:
                      LeftParen: (
                      RightParen: )
                    DashArrow: None
                    Type: None
                    ExternKw: None
                    Block:
                      LeftCurly: {
                      Expr: None
                      RightCurly: }
                    SemiColon: None
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn fn_with_type_no_body() {
        check_module(
            "fn foo() -> int;",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: foo
                    ParamList:
                      LeftParen: (
                      RightParen: )
                    DashArrow: ->
                    BasicType:
                      BasicName:
                        Ident: int
                    ExternKw: None
                    Block: None
                    SemiColon: ;
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn fn_with_type_with_body() {
        check_module(
            "fn foo() -> int {}",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: foo
                    ParamList:
                      LeftParen: (
                      RightParen: )
                    DashArrow: ->
                    BasicType:
                      BasicName:
                        Ident: int
                    ExternKw: None
                    Block:
                      LeftCurly: {
                      Expr: None
                      RightCurly: }
                    SemiColon: None
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn fn_with_params() {
        check_module(
            "fn foo(bar: u32) -> int {}",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  FnDef:
                    FnKw: fn
                    BasicName:
                      Ident: foo
                    ParamList:
                      LeftParen: (
                      NamedParam:
                        BasicName:
                          Ident: bar
                        Colon: :
                        BasicType:
                          BasicName:
                            Ident: u32
                      RightParen: )
                    DashArrow: ->
                    BasicType:
                      BasicName:
                        Ident: int
                    ExternKw: None
                    Block:
                      LeftCurly: {
                      Expr: None
                      RightCurly: }
                    SemiColon: None
                  RightCurly: None"#]],
        );
    }

    #[test]
    fn paren_expr() {
        check_expr(
            "(10)",
            expect![[r#"
            Group:
              Literal:
                Number: 10"#]],
        );
    }

    #[test]
    fn bin_op() {
        check_expr(
            "10 + 10",
            expect![[r#"
                BinExpr:
                  Literal:
                    Number: 10
                  Plus: +
                  Literal:
                    Number: 10"#]],
        );
    }

    #[test]
    fn prefix_op() {
        check_expr(
            "-10",
            expect![[r#"
                PrefixExpr:
                  Minus: -
                  Literal:
                    Number: 10"#]],
        );
    }

    #[test]
    fn call_expr() {
        check_expr(
            "foo()",
            expect![[r#"
                CallExpr:
                  NameRef:
                    BasicName:
                      Ident: foo
                  LeftParen: (
                  RightParen: )"#]],
        );
    }

    #[test]
    fn expr_item() {
        check_module(
            "10 + 10;",
            expect![[r#"
                Module:
                  ModKw: None
                  Ident: None
                  LeftCurly: None
                  ExprItem:
                    BinExpr:
                      Literal:
                        Number: 10
                      Plus: +
                      Literal:
                        Number: 10
                    SemiColon: ;
                  RightCurly: None"#]],
        );
    }
}
