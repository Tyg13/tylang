use std::collections::HashMap;

use once_cell::sync::OnceCell;

#[derive(Default)]
pub struct Legend {
    types: Vec<lsp_types::SemanticTokenType>,
    type_to_idx: HashMap<lsp_types::SemanticTokenType, u32>,
}

impl Legend {
    fn insert_type(&mut self, ty: lsp_types::SemanticTokenType) {
        self.type_to_idx.insert(ty.clone(), self.types.len() as u32);
        self.types.push(ty);
    }

    pub fn types(&self) -> &Vec<lsp_types::SemanticTokenType> {
        &self.types
    }

    pub fn idx_of(&self, ty: &lsp_types::SemanticTokenType) -> Option<u32> {
        self.type_to_idx.get(ty).cloned()
    }
}

#[rustfmt::skip]
pub fn legend() -> &'static Legend {
    static LEGEND: OnceCell<Legend> = OnceCell::new();
    LEGEND.get_or_init(|| {
        let mut legend = Legend::default();
        /* 0 */ legend.insert_type(lsp_types::SemanticTokenType::OPERATOR);
        /* 1 */ legend.insert_type(lsp_types::SemanticTokenType::KEYWORD);
        /* 2 */ legend.insert_type(lsp_types::SemanticTokenType::STRING);
        /* 3 */ legend.insert_type(lsp_types::SemanticTokenType::NUMBER);
        /* 4 */ legend.insert_type(lsp_types::SemanticTokenType::TYPE);
        /* 5 */ legend.insert_type(lsp_types::SemanticTokenType::FUNCTION);
        /* 6 */ legend.insert_type(lsp_types::SemanticTokenType::STRUCT);
        /* 7 */ legend.insert_type(lsp_types::SemanticTokenType::COMMENT);
        legend
    })
}
