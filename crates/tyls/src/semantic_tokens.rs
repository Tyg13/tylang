use std::collections::HashMap;
use std::sync::Arc;

use ast::Token;
use cst::SyntaxKind::*;
use lsp_types::SemanticTokenType;
use once_cell::sync::OnceCell;

use crate::ModuleInfo;

trait TokenExt {
    fn parse_as<T: ast::Node>(&self) -> Option<Arc<T>>;
}

impl TokenExt for cst::syntax::Token {
    fn parse_as<T: ast::Node>(&self) -> Option<Arc<T>> {
        self.ancestors().find_map(T::cast)
    }
}

#[derive(Default)]
pub struct Legend {
    types: Vec<SemanticTokenType>,
    type_to_idx: HashMap<SemanticTokenType, u32>,
}

impl Legend {
    fn insert_type(&mut self, idx: u32, ty: SemanticTokenType) {
        debug_assert_eq!(idx as usize, self.types.len());
        self.type_to_idx.insert(ty.clone(), idx);
        self.types.push(ty);
    }

    pub fn types(&self) -> Vec<SemanticTokenType> {
        self.types.clone()
    }
}

impl std::ops::Index<SemanticTokenType> for Legend {
    type Output = u32;
    fn index(&self, index: SemanticTokenType) -> &Self::Output {
        self.type_to_idx
            .get(&index)
            .expect("not a valid semantic token kind?")
    }
}

#[rustfmt::skip]
pub fn legend() -> &'static Legend {
    static LEGEND: OnceCell<Legend> = OnceCell::new();
    LEGEND.get_or_init(|| {
        let mut legend = Legend::default();
        legend.insert_type(0, SemanticTokenType::OPERATOR);
        legend.insert_type(1, SemanticTokenType::KEYWORD);
        legend.insert_type(2, SemanticTokenType::STRING);
        legend.insert_type(3, SemanticTokenType::NUMBER);
        legend.insert_type(4, SemanticTokenType::TYPE);
        legend.insert_type(5, SemanticTokenType::FUNCTION);
        legend.insert_type(6, SemanticTokenType::STRUCT);
        legend.insert_type(7, SemanticTokenType::COMMENT);
        legend.insert_type(8, SemanticTokenType::NAMESPACE);
        legend
    })
}

pub(crate) fn compute_from_module(
    info: &ModuleInfo,
) -> Vec<lsp_types::SemanticToken> {
    let TokenInfo { tokens, deltas } = collect_tokens(info);

    let mut semantic_tokens = Vec::new();
    let (mut acc_delta_line, mut acc_delta_column) = (0, 0);
    for i in 0..tokens.len() {
        let token = &tokens[i];
        let (delta_line, delta_column) = deltas[i];
        if delta_line > 0 {
            acc_delta_line += delta_line;
            acc_delta_column = delta_column;
        } else {
            acc_delta_column += delta_column;
        }
        if let Some(kind) = type_of_token(token) {
            semantic_tokens.push(lsp_types::SemanticToken {
                delta_line: acc_delta_line as u32,
                delta_start: acc_delta_column as u32,
                length: token.text().len() as u32,
                token_type: legend()[kind],
                token_modifiers_bitset: 0,
            });
            (acc_delta_line, acc_delta_column) = (0, 0);
        }
    }

    semantic_tokens
}

#[derive(Default)]
struct TokenInfo {
    tokens: Vec<cst::syntax::Token>,
    deltas: Vec<(usize, usize)>,
}

fn collect_tokens(info: &ModuleInfo) -> TokenInfo {
    fn delta_position(text: &str) -> (usize, usize) {
        let (mut line, mut column) = (0, 0);
        for c in text.chars() {
            if c == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        (line, column)
    }
    impl cst::syntax::traverse::Visitor for TokenInfo {
        fn visit(&mut self, node: cst::syntax::NodeOrToken) {
            if let Some(token) = node.into_token() {
                self.tokens.push(token.clone());
                self.deltas.push(delta_position(token.text()));
            }
        }
    }

    let mut collector = TokenInfo::default();
    collector.deltas.push((0, 0));
    cst::syntax::traverse::preorder(&mut collector, info.mod_.clone());
    assert_eq!(collector.deltas.len(), collector.tokens.len() + 1);
    collector
}

fn type_of_token(token: &cst::syntax::Token) -> Option<SemanticTokenType> {
    match token.kind() {
        NUMBER => Some(SemanticTokenType::NUMBER),
        STRING => Some(SemanticTokenType::STRING),
        COMMENT => Some(SemanticTokenType::COMMENT),
        IDENT => type_of_ident(token),
        kind if kind.is_operator() => Some(SemanticTokenType::OPERATOR),
        kind if kind.is_keyword() => Some(SemanticTokenType::KEYWORD),
        _ => None,
    }
}

fn type_of_ident(ident: &cst::syntax::Token) -> Option<SemanticTokenType> {
    assert_eq!(ident.kind(), IDENT);

    match ident.parent.kind() {
        MODULE => Some(SemanticTokenType::NAMESPACE),
        TYPE_ITEM => Some(SemanticTokenType::STRUCT),
        NAME => match ident.parent.parent()?.kind() {
            FN_ITEM => Some(SemanticTokenType::FUNCTION),
            STRUCT_LITERAL | BASIC_TYPE => Some(SemanticTokenType::TYPE),
            _ => ident.parse_as::<ast::CallExpr>().and_then(|call_expr| {
                ident_is_basic_name(
                    &call_expr.receiver()?.name_ref()?.name()?,
                    ident,
                )
                .then_some(SemanticTokenType::FUNCTION)
            }),
        },
        _ => None,
    }
}

fn ident_is_basic_name(
    name: &Arc<ast::Name>,
    ident: &cst::syntax::Token,
) -> bool {
    match name.as_ref() {
        ast::Name::BasicName(n) => {
            n.ident().map_or(false, |n| n.syntax() == ident)
        }
        ast::Name::DottedName(n) => n
            .tail()
            .map_or(false, |tail| ident_is_basic_name(&tail, ident)),
    }
}
