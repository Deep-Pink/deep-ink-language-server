use std::cmp::min;

use crate::deep_ink_type_sitter::nodes::{
    ContentLine, ContentLineWithSpeaker, LineCommand, TagCommand,
};
use crate::ink_type_sitter::nodes::Tag;
use crate::multimap::Multimap;
use crate::{
    DbMessage, DiagnosticsMessage, OpenInkDocument, UpdateInkDocument, deep_ink_type_sitter,
    ink_type_sitter,
};
use im::HashSet;
use im::hashmap::HashMap;
use ropey::Rope;
use tokio::sync::mpsc::Sender;
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Position, Range, Uri};
use tree_sitter::{InputEdit, Point, Query, QueryCapture, StreamingIterator, Tree};
use type_sitter::Node;

#[derive(Clone, Debug)]
pub struct InkDocument<'s> {
    uri: Uri,
    version: i32,
    contents: Rope,
    ink_tree: Option<Tree>,
    deep_ink_tree: Option<Tree>,
    deep_ink_content: HashSet<DeepInkContent<'s>>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum DeepInkContent<'s> {
    TagCommand(TagCommand<'s>),
    LineCommand(LineCommand<'s>),
    ContentLineWithSpeaker(ContentLineWithSpeaker<'s>),
    ContentLine(ContentLine<'s>),
    Error(DeepInkError<'s>),
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum DeepInkError<'s> {
    TagCommandError(type_sitter::Error<'s>),
    LineError(type_sitter::Error<'s>),
    FormatTagError(type_sitter::Error<'s>),
}

impl<'s> InkDocument<'s> {
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

fn update_ink_document_contents(document: &mut InkDocument, ink_parser: &mut InkParser) {
    document.ink_tree = ink_parser.parser.parse_with_options(
        &mut |byte: usize, position: Point| document.feeder(byte, position),
        document.ink_tree.as_ref(),
        None,
    );
}

fn update_deep_ink_document_contents(
    document: &mut InkDocument,
    deep_ink_parser: &mut DeepInkParser,
) {
    if let Some(tree) = document.ink_tree.as_ref() {
        let mut cursor = type_sitter::QueryCursor::new();
        let query = crate::ink_type_sitter::queries::DeepInkContent;
        let ropey_text_provider =
            crate::ropey_text_provider::RopeyTextProvider::new(document.contents.clone());
        let ink_node = ink_type_sitter::nodes::Ink::try_from_raw(tree.root_node()).unwrap();
        let mut matches = cursor.matches(&query, ink_node, ropey_text_provider);
        let mut ranges = vec![];
        let byte_len = document.contents.len_bytes();
        while let Some(mtch) = matches.next() {
            if let Some(tag) = mtch.tag() {
                let tag_range = tag.range();
                ranges.push(tag_range.clone());
                let tag_contents = document
                    .contents
                    .byte_slice(tag_range.start_byte..tag_range.end_byte)
                    .to_string();
            }
            if let Some(content) = mtch.content() {
                let content_range = content.range();
                ranges.push(content_range);
            }
            if let Some(end_of_line) = mtch.end_of_line() {
                let mut eol_range = end_of_line.range();
                eprintln!("EOL RANGE {:?}", eol_range);
                if eol_range.start_byte + 1 < byte_len {
                    eol_range.end_byte = eol_range.start_byte + 1;
                    eol_range.end_point.row = eol_range.start_point.row + 1;
                    eol_range.end_point.column = 0;
                }
                ranges.push(eol_range);
            }
            if let Some(choice_only) = mtch.choice_only() {
                let choice_only_range = choice_only.range();
                ranges.push(choice_only_range);
            }
        }
        ranges.sort_by(|x, y| x.start_byte.cmp(&y.start_byte));
        let mut resultant_ranges: Vec<tree_sitter::Range> = vec![];
        for r in ranges {
            if let Some(prev) = resultant_ranges.last_mut() {
                if r.start_byte < prev.end_byte {
                    if r.end_byte > prev.end_byte {
                        prev.end_point = r.end_point;
                        prev.end_byte = r.end_byte;
                    }
                    continue;
                }
            }
            resultant_ranges.push(r);
        }

        deep_ink_parser
            .parser
            .set_included_ranges(resultant_ranges.as_slice())
            .expect("VALID RANGES");
        document.deep_ink_tree = deep_ink_parser.parser.parse_with_options(
            &mut |byte: usize, position: Point| document.feeder(byte, position),
            document.deep_ink_tree.as_ref(),
            None,
        );
    }
}

fn create_ink_document<'s>(
    open_ink_document: OpenInkDocument,
    ink_parser: &mut InkParser,
    deep_ink_parser: &mut DeepInkParser,
) -> InkDocument<'s> {
    let rope = Rope::from_str(&open_ink_document.contents);
    let mut document = InkDocument {
        uri: open_ink_document.uri.clone(),
        version: open_ink_document.version,
        contents: rope.clone(),
        ink_tree: None,
        deep_ink_tree: None,
        deep_ink_content: HashSet::new(),
    };
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
    let mut dirty = HashSet::new();
    for update in updates {
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
        let edit = InputEdit {
            start_byte: start_byte,
            old_end_byte: old_end_byte,
            new_end_byte: new_end_byte,
            start_position: Point::new(start.line as usize, start.character as usize),
            old_end_position: Point::new(end.line as usize, end.character as usize),
            new_end_position,
        };
        if let Some(ink_tree) = document.ink_tree.as_mut() {
            ink_tree.edit(&edit);
        };
        if let Some(deep_ink_tree) = document.deep_ink_tree.as_mut() {
            deep_ink_tree.edit(&edit);
        };
        document.contents = rope.clone();

        document.version = update.version;
        dirty.insert(document.uri.clone());
    }
    for d in dirty {
        let document = documents
            .get_mut(&d)
            .expect("Dirty should never be unable to find the new document");
        update_ink_document_contents(document, ink_parser);
        update_deep_ink_document_contents(document, deep_ink_parser)
    }
}

pub async fn start_database_service(
    mut db_message_receiver: Receiver<DbMessage>,
    use_deep_ink: bool,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        eprintln!("STARTING DATABASE SERVICE");
        let mut documents: HashMap<Uri, InkDocument> = HashMap::new();
        let mut ink_parser = create_ink_parser();
        let mut deep_ink_parser = create_deep_ink_parser();
        while let Some(cmd) = db_message_receiver.blocking_recv() {
            let mut diagnostics_message_sender: Option<Sender<DiagnosticsMessage>> = None;
            match cmd {
                DbMessage::Open(open_ink_document, sender) => {
                    diagnostics_message_sender = Some(sender);
                    eprintln!("OPENED INK DOCUMENT");
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
                }
            }
            if let Some(sender) = diagnostics_message_sender {
                let diagnostics = analyze_documents(&mut documents, use_deep_ink);
                match sender.blocking_send(DiagnosticsMessage(diagnostics)) {
                    Ok(_) => {}
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
) -> InkDiagnostic {
    let severity: Option<DiagnosticSeverity> = Some(DiagnosticSeverity::ERROR);
    let message: String;
    let range;
    if capture.index == 1 {
        let node = capture.node.parent().unwrap_or_else(|| capture.node);
        range = node.range();
        let kind = node.kind();
        message = format!("Missing closing character for {}", kind).into();
    } else {
        let node = capture.node.parent().unwrap_or_else(|| capture.node);
        range = node.range();
        let kind = node.kind();
        message = format!("Syntax error for {}", kind).into()
    }
    InkDiagnostic(Diagnostic {
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
    })
}

pub fn raise_deep_ink_syntax_errors(document: &InkDocument) -> Vec<InkDiagnostic> {
    let mut result = vec![];
    if let Some(tree) = document.deep_ink_tree.as_ref() {
        let error_query = Query::new(
            &tree_sitter_ink::LANGUAGE.into(),
            &"
            (ERROR) @ink_errors
            (MISSING) @missing_errors
        ",
        )
        .expect("Valid Query");
        let mut cursor = tree_sitter::QueryCursor::new();
        let ropey_text_provider =
            crate::ropey_text_provider::RopeyTextProvider::new(document.contents.clone());
        let capture_names = error_query.capture_names();
        let mut matches = cursor.matches(&error_query, tree.root_node(), ropey_text_provider);
        while let Some(mtch) = matches.next() {
            for capture in mtch.captures {
                result.push(diagnostic_from_error_or_missing_capture(
                    capture,
                    capture_names,
                ));
            }
        }
    }
    result
}

pub fn raise_ink_syntax_errors(document: &InkDocument) -> Vec<InkDiagnostic> {
    let mut result = vec![];
    if let Some(tree) = document.ink_tree.as_ref() {
        let error_query = Query::new(
            &tree_sitter_ink::LANGUAGE.into(),
            &"
            (ERROR) @ink_errors
            (_ (MISSING)) @missing_errors
        ",
        )
        .expect("Valid Query");
        let mut cursor = tree_sitter::QueryCursor::new();
        let ropey_text_provider =
            crate::ropey_text_provider::RopeyTextProvider::new(document.contents.clone());
        let capture_names = error_query.capture_names();
        let mut matches = cursor.matches(&error_query, tree.root_node(), ropey_text_provider);
        while let Some(mtch) = matches.next() {
            for capture in mtch.captures {
                result.push(diagnostic_from_error_or_missing_capture(
                    capture,
                    capture_names,
                ));
            }
        }
    };
    result
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InkDiagnostic(pub Diagnostic);

impl std::hash::Hash for InkDiagnostic {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.range.hash(state);
        self.0.message.hash(state);
    }
}

pub fn analyze_documents(
    documents: &mut HashMap<Uri, InkDocument>,
    use_deep_ink: bool,
) -> Multimap<(Uri, i32), InkDiagnostic> {
    let mut diagnostic_map: Multimap<(Uri, i32), InkDiagnostic> = Multimap::new();
    for document in documents.values() {
        diagnostic_map = diagnostic_map.add_range(
            (document.uri.clone(), document.version),
            raise_ink_syntax_errors(&document),
        );
    }
    if use_deep_ink {
        for k in documents.keys().cloned().collect::<Vec<Uri>>() {
            let document = documents.get_mut(&k).unwrap();
            diagnostic_map = diagnostic_map.add_range(
                (document.uri.clone(), document.version),
                raise_deep_ink_syntax_errors(&document),
            );
        }
    };
    diagnostic_map
}
