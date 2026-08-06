use std::{borrow::Cow, sync::LazyLock};

use dashmap::{
    DashMap,
    mapref::one::{Ref, RefMut},
};
use ropey::Rope;
use salsa::Storage;
use tower_lsp_server::ls_types::{Range, Uri};
use tree_sitter::{InputEdit, Point, Tree};

#[salsa::input]
#[derive(Debug)]
pub struct OpenInkDocument {
    pub uri: Uri,
    pub version: i32,
    #[returns(deref)]
    pub contents: String,
}

#[salsa::input]
#[derive(Debug)]
pub struct UpdateInkDocument {
    #[returns(deref)]
    pub uri: Uri,
    #[returns(clone)]
    pub version: i32,
    #[returns(clone)]
    pub range: Option<Range>,
    #[returns(deref)]
    pub new_text: String,
}

#[salsa::input]
#[derive(Debug)]
pub struct RemoveInkDocument {
    pub uri: Uri,
}

#[salsa::input]
#[derive(Debug)]
pub struct RenameInkDocument {
    #[returns(deref)]
    pub old_uri: Uri,
    #[returns(deref)]
    pub new_uri: Uri,
}

#[salsa::tracked]
pub struct InkDocument<'db> {
    #[tracked]
    uri: Uri,
    #[tracked]
    version: i32,
}
pub struct InkAst {
    pub document_id: salsa::Id,
    pub uri: Uri,
    pub version: i32,
    pub rope: ropey::Rope,
    pub ink_tree: Option<Tree>,
    pub deep_ink_tree: Option<Tree>,
    pub ink_parser: tree_sitter::Parser,
    pub deep_ink_parser: tree_sitter::Parser,
}

fn create_ink_parser() -> tree_sitter::Parser {
    let mut ink_parser = tree_sitter::Parser::new();
    ink_parser
        .set_language(&tree_sitter_ink::LANGUAGE.into())
        .expect("Error loading Ink Grammar");
    ink_parser
}

fn create_deep_ink_parser() -> tree_sitter::Parser {
    let mut ink_parser = tree_sitter::Parser::new();
    ink_parser
        .set_language(&tree_sitter_deep_pink_ink::LANGUAGE.into())
        .expect("Error loading Deep Ink Grammar");
    ink_parser
}

impl InkAst {
    pub fn new(document_id: salsa::Id, uri: Uri, version: i32, file_contents: &str) -> InkAst {
        let mut ink_parser = create_ink_parser();
        let mut deep_ink_parser = create_deep_ink_parser();
        let mut rope = Rope::from_str(file_contents);
        let maybe_ink_tree = ink_parser.parse(file_contents, None);
        let maybe_deep_ink_tree = deep_ink_parser.parse(file_contents, None);

        InkAst {
            document_id,
            uri,
            version,
            ink_tree: maybe_ink_tree,
            deep_ink_tree: maybe_deep_ink_tree,
            ink_parser,
            deep_ink_parser,
            rope,
        }
    }

    pub fn edit(
        mut self,
        uri: &Uri,
        version: i32,
        range: &Option<Range>,
        new_text: &str,
    ) -> InkAst {
        let Some(range) = range else {
            return InkAst {
                uri: uri.clone(),
                version,
                ..self
            };
        };

        let mut rope = self.rope;
        let start = range.start;
        let end = range.end;
        let start_byte = rope.line_to_byte(start.line as usize) + (start.character as usize);
        let old_end_byte = rope.line_to_byte(end.line as usize) + (end.character as usize);
        let new_end_byte = start_byte + new_text.len();

        rope.remove(start_byte..old_end_byte);
        rope.insert(start_byte, new_text);
        let end_line = rope.byte_to_line(new_end_byte);
        let start_byte_of_end_line = rope.line_to_byte(end_line);
        let new_end_position = Point::new(end_line, new_end_byte - start_byte_of_end_line);

        let edit = InputEdit {
            start_byte: start_byte,
            old_end_byte: old_end_byte,
            new_end_byte: new_end_byte,
            start_position: Point::new(start.line as usize, start.character as usize),
            old_end_position: Point::new(end.line as usize, end.character as usize),
            new_end_position,
        };
        if let Some(ink_tree) = self.ink_tree.as_mut() {
            ink_tree.edit(&edit);
        };
        if let Some(deep_ink_tree) = self.deep_ink_tree.as_mut() {
            deep_ink_tree.edit(&edit);
        };

        let mut feeder = |_: usize, position: Point| rope.line(position.row).to_string();
        InkAst {
            uri: uri.clone(),
            document_id: self.document_id,
            version,
            ink_tree: self
                .ink_parser
                .parse_with_options(&mut feeder, self.ink_tree.as_ref(), None),
            deep_ink_tree: self.deep_ink_parser.parse_with_options(
                &mut feeder,
                self.deep_ink_tree.as_ref(),
                None,
            ),
            ink_parser: self.ink_parser,
            deep_ink_parser: self.deep_ink_parser,
            rope,
        }
    }
}

#[salsa::tracked]
pub struct LspWorkspace<'db> {}

#[salsa::db]
pub struct LspDb {
    storage: Storage<Self>,
    document_trees: DashMap<Uri, InkAst>,
}

#[salsa::db]
pub trait Db: salsa::Database {
    fn get_ast_mut(&self, uri: &Uri) -> Option<RefMut<'_, Uri, InkAst>>;
    fn get_ast(&self, uri: &Uri) -> Option<Ref<'_, Uri, InkAst>>;
    fn set_ast(&self, uri: Uri, ast: InkAst);
    fn remove_ast(&self, uri: &Uri);
}

#[salsa::db]
impl salsa::Database for LspDb {}
#[salsa::db]
impl Db for LspDb {
    fn get_ast_mut(&self, uri: &Uri) -> Option<RefMut<'_, Uri, InkAst>> {
        self.document_trees.get_mut(uri)
    }
    fn get_ast(&self, uri: &Uri) -> Option<Ref<'_, Uri, InkAst>> {
        self.document_trees.get(uri)
    }
    fn set_ast(&self, uri: Uri, ast: InkAst) {
        self.document_trees.insert(uri, ast);
    }
    fn remove_ast(&self, uri: &Uri) {
        self.document_trees.remove(uri);
    }
}

#[salsa::tracked(returns(copy))]
pub fn open_document(db: &dyn Db, open_ink_document: OpenInkDocument) -> InkDocument<'_> {
    let uri = open_ink_document.uri(db).clone();
    let document = InkDocument::new(db, uri.clone(), *open_ink_document.version(db));
    let ink_ast = InkAst::new(
        document.0,
        uri.clone(),
        *open_ink_document.version(db),
        open_ink_document.contents(db),
    );
    db.set_ast(uri, ink_ast);
    document
}
