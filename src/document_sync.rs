use std::{hash::Hash, vec::Vec};

use tower_lsp_server::ls_types::{TextDocumentItem, Uri};
use tree_sitter::{self, Tree};

#[derive(Debug)]
pub struct InkDocument {
    uri: Uri,
    version: i32,
    ink_tree: Option<Tree>,
    deep_pink_tree: Option<Tree>,
    references: Vec<Uri>,
}

impl Drop for InkDocument {
    fn drop(&mut self) {
        match self.ink_tree.take() {
            Some(tree) => drop(tree),
            None => {}
        };
        match self.deep_pink_tree.take() {
            Some(tree) => drop(tree),
            None => {}
        };
    }
}

impl PartialEq for InkDocument {
    fn eq(&self, other: &Self) -> bool {
        self.uri == other.uri
    }
}

impl Hash for InkDocument {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uri.hash(state);
    }
}

impl PartialOrd for InkDocument {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.uri.eq(&other.uri) {
            Some(self.version.cmp(&other.version))
        } else {
            None
        }
    }
}

impl InkDocument {
    fn try_parse_ink(text_document_item: &TextDocumentItem) -> Option<Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_ink::LANGUAGE.into())
            .expect("Error loading Ink Grammar");
        let tree = parser.parse(&text_document_item.text, None);
        return tree;
    }

    pub fn new(document: &TextDocumentItem) -> InkDocument {
        let maybe_ink_tree = InkDocument::try_parse_ink(&document);
        InkDocument {
            version: document.version,
            uri: document.uri.clone(),
            ink_tree: maybe_ink_tree,
            deep_pink_tree: None,
            references: vec![],
        }
    }

    pub fn references(&self) -> &[Uri] {
        &self.references
    }
}
