use std::cmp::min;

use crate::{DbMessage, DiagnosticsMessage, OpenInkDocument, UpdateInkDocument};
use im::hashmap::HashMap;
use ropey::Rope;
use tokio::sync::mpsc::Sender;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Position, Range, Uri};
use tree_sitter::{InputEdit, Point, Query, QueryCapture, StreamingIterator, Tree};

#[derive(Clone)]
pub struct InkDocument {
    uri: Uri,
    version: i32,
    contents: Rope,
    ink_tree: Option<Tree>,
}

impl InkDocument {
    fn feeder(&self, byte: usize, _position: Point) -> String {
        const FEED_CHAR_LENGTH: usize = 256;
        let start_char = self.contents.byte_to_char(byte);
        let end_char = min(start_char + FEED_CHAR_LENGTH, self.contents.len_chars());
        let rope_slice = self.contents.slice(start_char..end_char);
        rope_slice.to_string()
    }
}

pub fn create_ink_parser() -> InkParser {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_ink::LANGUAGE.into())
        .expect("Error loading Ink Grammar");
    InkParser { parser }
}

pub fn create_deep_ink_parser() -> DeepInkParser {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_deep_pink_ink::LANGUAGE.into())
        .expect("Error loading Deep Ink Grammar");
    DeepInkParser { parser }
}

pub struct InkParser {
    pub parser: tree_sitter::Parser,
}

pub struct DeepInkParser {
    pub parser: tree_sitter::Parser,
}

fn update_deep_ink_document_contents(
    document: &mut InkDocument,
    deep_ink_parser: &mut DeepInkParser,
) {
}

fn create_ink_document(
    open_ink_document: OpenInkDocument,
    ink_parser: &mut InkParser,
    deep_ink_parser: &mut DeepInkParser,
) -> InkDocument {
    let rope = Rope::from_str(&open_ink_document.contents);
    let mut document = InkDocument {
        uri: open_ink_document.uri.clone(),
        version: open_ink_document.version,
        contents: rope.clone(),
        ink_tree: None,
    };
    let mut feeder = |byte: usize, _position: Point| {
        const FEED_CHAR_LENGTH: usize = 256;
        let char_pos = rope.byte_to_char(byte);
        let rope_slice = rope.slice(char_pos..(char_pos + FEED_CHAR_LENGTH));
        rope_slice.to_string()
    };
    // let mut feeder = create_feeder(document.contents.clone());
    document.ink_tree = ink_parser.parser.parse_with_options(
        &mut |byte: usize, position: Point| document.feeder(byte, position),
        document.ink_tree.as_ref(),
        None,
    );
    update_deep_ink_document_contents(&mut document, deep_ink_parser);
    document
}

fn update_ink_documents(
    ink_parser: &mut InkParser,
    deep_ink_parser: &mut DeepInkParser,
    documents: &mut HashMap<Uri, InkDocument>,
    updates: Vec<UpdateInkDocument>,
) {
    for update in updates {
        eprintln!(
            "Updating doc {} with version {}",
            update.uri.to_string(),
            update.version
        );
        let uri = update.uri;
        let document = documents.get_mut(&uri);
        let Some(document) = document else {
            eprintln!("NO DOCUMENT");
            continue;
        };
        let range = match update.range {
            crate::UpdateRange::Range(range) => range,
            crate::UpdateRange::All => {
                let (line_count, last_line) = update
                    .new_text
                    .lines()
                    .fold((0, ""), |(prev_count, _), current_line| {
                        (prev_count + 1, current_line)
                    });
                let rope = Rope::from_str(last_line);
                Range::new(
                    Position {
                        line: 0,
                        character: 0,
                    },
                    Position {
                        line: line_count,
                        character: rope.len_chars() as u32,
                    },
                )
            }
        };

        let mut rope = document.contents.clone();
        let start = range.start;
        let end = range.end;
        let start_char = rope.line_to_char(start.line as usize) + (start.character as usize);
        let start_byte = rope.char_to_byte(start_char);
        let old_end_char = rope.line_to_char(end.line as usize) + (end.character as usize);
        let old_end_byte = rope.char_to_byte(old_end_char);
        let new_text_rope = Rope::from_str(&update.new_text);
        let new_end_char = start_char + new_text_rope.len_chars();
        rope.remove(start_char..old_end_char);
        rope.insert(start_char, &update.new_text);
        let new_end_byte = rope.char_to_byte(new_end_char);
        let end_line = rope.char_to_line(new_end_char);
        let start_char_of_end_line = rope.line_to_char(end_line);
        let new_end_position = Point::new(end_line, new_end_char - start_char_of_end_line);
        if let Some(ink_tree) = document.ink_tree.as_mut() {
            let edit = InputEdit {
                start_byte: start_byte,
                old_end_byte: old_end_byte,
                new_end_byte: new_end_byte,
                start_position: Point::new(start.line as usize, start.character as usize),
                old_end_position: Point::new(end.line as usize, end.character as usize),
                new_end_position,
            };
            ink_tree.edit(&edit);
        };
        document.contents = rope.clone();
        document.ink_tree = ink_parser.parser.parse_with_options(
            &mut |byte: usize, position: Point| document.feeder(byte, position),
            document.ink_tree.as_ref(),
            None,
        );
        document.version = update.version;
        update_deep_ink_document_contents(document, deep_ink_parser);
    }
}

pub async fn start_database_service(
    mut db_message_receiver: Receiver<DbMessage>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        eprintln!("STARTING DATABASE SERVICE");
        let mut documents: HashMap<Uri, InkDocument> = HashMap::new();
        let mut ink_parser = create_ink_parser();
        let mut deep_ink_parser = create_deep_ink_parser();
        while let Some(cmd) = db_message_receiver.blocking_recv() {
            eprintln!("RECEIVED MESSAGE");
            let mut diagnostics_message_sender: Option<Sender<DiagnosticsMessage>> = None;
            match cmd {
                DbMessage::Open(open_ink_document, sender) => {
                    diagnostics_message_sender = Some(sender);
                    eprintln!("OPEN INK DOCUMENT");
                    documents.insert(
                        open_ink_document.uri.clone(),
                        create_ink_document(
                            open_ink_document,
                            &mut ink_parser,
                            &mut deep_ink_parser,
                        ),
                    );
                }
                DbMessage::Update(update, sender) => {
                    diagnostics_message_sender = Some(sender);
                    eprintln!("UPDATE INK DOCUMENT");
                    update_ink_documents(
                        &mut ink_parser,
                        &mut deep_ink_parser,
                        &mut documents,
                        update,
                    );
                }
                DbMessage::Remove(remove_ink_document, sender) => {
                    diagnostics_message_sender = Some(sender);
                    documents = documents.without(&remove_ink_document.uri);
                }
                DbMessage::Rename(rename_ink_document, sender) => {
                    diagnostics_message_sender = Some(sender);
                    if let Some(mut old) = documents.remove(&rename_ink_document.old_uri) {
                        old.uri = rename_ink_document.new_uri.clone();
                        documents.insert(rename_ink_document.new_uri, old);
                    };
                }
                DbMessage::RequestDiagnostics(sender) => {
                    diagnostics_message_sender = Some(sender);
                    eprintln!("REQUESTED DIAGNOSTICS");
                }
            }
            if let Some(sender) = diagnostics_message_sender {
                let diagnostics = analyze_documents(&mut documents);
                match sender.blocking_send(DiagnosticsMessage(diagnostics)) {
                    Ok(_) => eprintln!("SENT DIAGNOSTICS"),
                    Err(err) => eprintln!(
                        "FAILED TO SEND DIAGNOSTICS DUE TO ERROR: {}",
                        err.to_string()
                    ),
                }
            };
        }
        eprintln!("Terminating Db Service");
        ()
    })
}

fn diagnostic_from_error_or_missing_capture(
    capture: &QueryCapture,
    capture_names: &[&str],
) -> Diagnostic {
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

    Diagnostic {
        range: Range {
            start: Position::new(
                range.start_point.row as u32,
                range.start_point.column as u32,
            ),
            end: Position::new(range.end_point.row as u32, range.end_point.column as u32),
        },
        severity,
        message,
        code: None,
        code_description: None,
        source: None,
        related_information: None,
        tags: None,
        data: None,
    }
}

pub fn raise_ink_syntax_errors(document: &InkDocument) -> Vec<Diagnostic> {
    let mut result = vec![];
    if let Some(tree) = document.ink_tree.as_ref() {
        let error_query = Query::new(
            &tree_sitter_ink::LANGUAGE.into(),
            &"
            (ERROR) @ink_errors
            (MISSING) @missing_errors
        ",
        )
        .expect("Valid Query");
        eprintln!("TREE IS FINE");
        let mut cursor = tree_sitter::QueryCursor::new();
        let ropey_text_provider =
            crate::ropey_text_provider::RopeyTextProvider::new(document.contents.clone());
        let capture_names = error_query.capture_names();
        let mut matches = cursor.matches(&error_query, tree.root_node(), ropey_text_provider);
        while let Some(mtch) = matches.next() {
            eprintln!("MATCH {}", mtch.id());
            for capture in mtch.captures {
                eprintln!("ADDING SYNTAX ERRORS WITH CAPTURE {}", capture.index);
                result.push(diagnostic_from_error_or_missing_capture(
                    capture,
                    capture_names,
                ));
            }
        }
    };
    result
}

pub fn analyze_documents(
    documents: &mut HashMap<Uri, InkDocument>,
) -> HashMap<(Uri, i32), Vec<Diagnostic>> {
    let mut diagnostic_map: HashMap<(Uri, i32), Vec<Diagnostic>> = HashMap::new();
    for document in documents.values() {
        diagnostic_map.insert(
            (document.uri.clone(), document.version),
            raise_ink_syntax_errors(&document),
        );
    }
    diagnostic_map
}
