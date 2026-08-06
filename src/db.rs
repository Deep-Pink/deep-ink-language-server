use ropey::Rope;
use salsa::{Accumulator, Setter, Storage};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tower_lsp_server::ls_types::{
    DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Position, Range, Uri,
};
use tree_sitter::{Point, Query, QueryCapture, StreamingIterator, Tree};

use crate::{DbMessage, LspMessage, OpenInkDocument, UpdateInkDocument};

#[salsa::input(debug)]
pub struct InkDocument {
    #[returns(clone)]
    uri: Uri,
    version: i32,
    #[returns(clone)]
    contents: Rope,
}

pub fn create_ink_parser(db: &dyn Db) -> InkParser {
    let mut ink_parser = tree_sitter::Parser::new();
    ink_parser
        .set_language(&tree_sitter_ink::LANGUAGE.into())
        .expect("Error loading Ink Grammar");
    InkParser::new(db, Arc::new(RwLock::new(ink_parser)))
}

pub fn create_deep_ink_parser(db: &dyn Db) -> DeepInkParser {
    let mut ink_parser = tree_sitter::Parser::new();
    ink_parser
        .set_language(&tree_sitter_deep_pink_ink::LANGUAGE.into())
        .expect("Error loading Deep Ink Grammar");
    DeepInkParser::new(db, Arc::new(RwLock::new(ink_parser)))
}

#[salsa::tracked]
pub struct InkAst<'db> {
    #[tracked]
    pub tree: InkTree,
}

pub struct InkTree {
    pub version: i32,
    pub tree: Option<Tree>,
}

impl PartialEq for InkTree {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
    }
}

impl std::hash::Hash for InkTree {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.version.hash(state);
    }
}

#[salsa::input]
pub struct InkParser {
    #[returns(clone)]
    pub parser: Arc<RwLock<tree_sitter::Parser>>,
}

#[salsa::input]
pub struct DeepInkParser {
    #[returns(clone)]
    pub parser: Arc<RwLock<tree_sitter::Parser>>,
}

impl Documents {
    pub fn handle_open_document(&mut self, db: &mut dyn Db, open_document: &OpenInkDocument) {
        let new_document = InkDocument::new(
            db,
            open_document.uri.clone(),
            open_document.version,
            Rope::from_str(&open_document.contents),
        );
        let mut new_documents = self.documents(db).clone();
        new_documents.insert(new_document.uri(db).clone(), new_document);
        self.set_documents(db).to(new_documents);
    }

    pub fn update_ink_documents(&mut self, db: &mut dyn Db, updates: Vec<UpdateInkDocument>) {
        let documents = self.documents(db).clone();
        for update in updates {
            let document = documents.get(&update.uri);
            let Some(document) = document else {
                continue;
            };
            let Some(range) = update.range else { return };
            let mut rope = document.contents(db).clone();
            let start = range.start;
            let end = range.end;
            let start_char = rope.line_to_char(start.line as usize) + (start.character as usize);
            // let start_byte = rope.char_to_byte(start_char);
            let old_end_char = rope.line_to_char(end.line as usize) + (end.character as usize);
            // let old_end_byte = rope.char_to_byte(old_end_char);
            let new_text_rope = Rope::from_str(&update.new_text);
            let new_end_char = start_char + new_text_rope.len_chars();
            rope.remove(start_char..old_end_char);
            rope.insert(start_char, &update.new_text);
            // let new_end_byte = rope.char_to_byte(new_end_char);
            // let end_line = rope.char_to_line(new_end_char);
            // let start_char_of_end_line = rope.line_to_char(end_line);
            // let new_end_position = Point::new(end_line, new_end_char - start_char_of_end_line);

            // if let Some(ink_tree) = ast.tree.as_mut() {
            //     let edit = InputEdit {
            //         start_byte: start_byte,
            //         old_end_byte: old_end_byte,
            //         new_end_byte: new_end_byte,
            //         start_position: Point::new(start.line as usize, start.character as usize),
            //         old_end_position: Point::new(end.line as usize, end.character as usize),
            //         new_end_position,
            //     };
            //     ink_tree.edit(&edit);
            // };

            // let mut feeder = |byte: usize, _position: Point| {
            //     const FEED_CHAR_LENGTH: usize = 256;
            //     let char_pos = rope.byte_to_char(byte);
            //     let rope_slice = rope.slice(char_pos..(char_pos + FEED_CHAR_LENGTH));
            //     rope_slice.to_string()
            // };
            document.set_contents(db).to(rope);
            document.set_version(db).to(update.version);
            document.set_uri(db).to(update.uri.clone());
        }
    }
}

#[salsa::input(debug)]
pub struct Documents {
    pub documents: HashMap<Uri, InkDocument>,
}

#[salsa::db]
pub struct LspDb {
    storage: Storage<Self>,
    ink_parser: Option<InkParser>,
    deep_ink_parser: Option<DeepInkParser>,
    documents: Option<Documents>,
    db_message_receiver: Receiver<DbMessage>,
    lsp_message_sender: Sender<LspMessage>,
}

impl LspDb {
    pub fn new(
        db_message_receiver: Receiver<DbMessage>,
        lsp_message_sender: Sender<LspMessage>,
    ) -> LspDb {
        let db = Storage::new(None);
        let mut result = LspDb {
            storage: db,
            ink_parser: None,
            deep_ink_parser: None,
            documents: None,
            db_message_receiver,
            lsp_message_sender,
        };
        result.deep_ink_parser = Some(create_deep_ink_parser(&result));
        result.ink_parser = Some(create_ink_parser(&result));
        result.documents = Some(Documents::new(&result, HashMap::new()));
        result
    }

    pub fn start_database_service(
        db_message_receiver: Receiver<DbMessage>,
        lsp_message_sender: Sender<LspMessage>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut db = LspDb::new(db_message_receiver, lsp_message_sender);
            while let Some(cmd) = db.db_message_receiver.recv().await {
                match cmd {
                    DbMessage::Start => {}
                    DbMessage::Open(open_ink_document) => {
                        db.documents
                            .unwrap()
                            .handle_open_document(&mut db, &open_ink_document);
                    }
                    DbMessage::Update(update_ink_documents) => {
                        db.documents
                            .unwrap()
                            .update_ink_documents(&mut db, update_ink_documents);
                    }
                    DbMessage::Remove(remove_ink_document) => {
                        let documents = db.documents.unwrap();
                        let mut documents_map = documents.documents(&db).clone();
                        documents_map.remove(&remove_ink_document.uri);
                        documents.set_documents(&mut db).to(documents_map);
                    }
                    DbMessage::Rename(rename_ink_document) => {
                        let documents = db.documents.unwrap();
                        let mut documents_map = documents.documents(&db).clone();
                        if let Some(old_value) = documents_map.remove(&rename_ink_document.old_uri)
                        {
                            documents_map.insert(rename_ink_document.new_uri, old_value);
                        };
                        documents.set_documents(&mut db).to(documents_map);
                    }
                }
            }
            ()
        })
    }
}

#[salsa::db]
pub trait Db: salsa::Database {
    fn parse_ink(&self, rope: &Rope) -> Option<Tree>;

    fn documents(&self) -> std::collections::hash_map::Values<Uri, InkDocument>;
}

#[salsa::db]
impl salsa::Database for LspDb {}

#[salsa::db]
impl Db for LspDb {
    fn parse_ink(&self, rope: &Rope) -> Option<Tree> {
        let parser = self.ink_parser.unwrap().parser(self);
        let mut parser = parser.write().expect("Expected lock");
        let mut feeder = |byte: usize, _position: Point| {
            const FEED_CHAR_LENGTH: usize = 256;
            let char_pos = rope.byte_to_char(byte);
            let rope_slice = rope.slice(char_pos..(char_pos + FEED_CHAR_LENGTH));
            rope_slice.to_string()
        };
        parser.parse_with_options(&mut feeder, None, None)
    }

    fn documents(&self) -> std::collections::hash_map::Values<Uri, InkDocument> {
        self.documents.unwrap().documents(self).values()
    }
}

#[salsa::tracked]
pub fn parse_document<'db>(db: &'db dyn Db, document: InkDocument) -> InkAst<'db> {
    let rope = document.contents(db);
    let tree = db.parse_ink(&rope);
    InkAst::new(
        db,
        InkTree {
            version: *document.version(db),
            tree,
        },
    )
}

#[salsa::accumulator]
pub struct InkDiagnostic {
    pub uri: Uri,
    // /// The range at which the message applies.
    pub range: Range,

    // /// The diagnostic's severity. Can be omitted. If omitted it is up to the
    // /// client to interpret diagnostics as error, warning, info or hint.
    pub severity: Option<DiagnosticSeverity>,

    // /// The diagnostic's code. Can be omitted.
    // pub code: Option<NumberOrString>,

    // /// An optional property to describe the error code.
    // ///
    // /// @since 3.16.0
    // pub code_description: Option<CodeDescription>,

    // /// A human-readable string describing the source of this
    // /// diagnostic, e.g. 'typescript' or 'super lint'.
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    // /// The diagnostic's message.
    pub message: String,

    // /// An array of related diagnostic information, e.g. when symbol-names within
    // /// a scope collide all definitions can be marked via this property.
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub related_information: Option<Vec<DiagnosticRelatedInformation>>,

    // /// Additional metadata about the diagnostic.
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<DiagnosticTag>>,
}

impl InkDiagnostic {
    fn from_error_or_missing_capture(
        uri: Uri,
        capture: &QueryCapture,
        capture_names: &[&str],
    ) -> Self {
        let severity: Option<DiagnosticSeverity> = Some(DiagnosticSeverity::ERROR);
        let mut message: String = "".into();
        if (capture.index as usize) < capture_names.len() {
            let capture_name = capture_names[capture.index as usize];
            if capture_name == "error" {
                message = "Syntax Error".into();
            } else if capture_name == "missing" {
                message = "Missing Value".into();
            }
        }
        let range = capture.node.range();

        InkDiagnostic {
            uri,
            range: Range {
                start: Position::new(
                    range.start_point.row as u32,
                    range.start_point.column as u32,
                ),
                end: Position::new(range.end_point.row as u32, range.end_point.column as u32),
            },
            severity,
            source: None,
            message,
            related_information: None,
            tags: None,
        }
    }
}

#[salsa::tracked]
pub fn raise_ink_syntax_errors<'db>(
    db: &'db dyn Db,
    ink_document: InkDocument,
    ink_ast: InkAst<'db>,
) {
    if let Some(tree) = ink_ast.tree(db).tree.as_ref() {
        let query = Query::new(
            &tree_sitter_ink::LANGUAGE.into(),
            &"
            (ERROR) @errors
            (MISSING) @missing
        ",
        )
        .expect("Valid Query");
        let mut cursor = tree_sitter::QueryCursor::new();
        let ropey_text_provider =
            crate::ropey_text_provider::RopeyTextProvider::new(ink_document.contents(db));
        let capture_names = query.capture_names();
        let mut matches = cursor.matches(&query, tree.root_node(), ropey_text_provider);
        while let Some(mtch) = matches.next() {
            for capture in mtch.captures {
                InkDiagnostic::from_error_or_missing_capture(
                    ink_document.uri(db),
                    capture,
                    capture_names,
                )
                .accumulate(db);
            }
        }
    };
}

#[salsa::tracked]
pub fn analyze_documents<'db>(db: &'db dyn Db) {
    let mut syntax_trees: HashMap<Uri, &InkAst> = HashMap::new();
    for document in db.documents() {
        let ink_ast = parse_document(db, document.clone());
        raise_ink_syntax_errors(db, document.clone(), ink_ast.clone());
        syntax_trees.insert(document.uri(db), ink_ast);
    }
}
