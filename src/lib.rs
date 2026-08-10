use im::hashmap::HashMap;
use tokio::sync::mpsc::Sender;
use tower_lsp_server::ls_types::{Diagnostic, Range, Uri};
pub mod db;
pub mod ropey_text_provider;

pub enum DbMessage {
    RequestDiagnostics(Sender<DiagnosticsMessage>),
    Open(OpenInkDocument, Sender<DiagnosticsMessage>),
    Update(Vec<UpdateInkDocument>, Sender<DiagnosticsMessage>),
    Remove(RemoveInkDocument, Sender<DiagnosticsMessage>),
    Rename(RenameInkDocument, Sender<DiagnosticsMessage>),
}

#[derive(Debug, Clone)]
pub struct DiagnosticsMessage(pub HashMap<(Uri, i32), Vec<Diagnostic>>);

#[derive(Debug)]
pub struct OpenInkDocument {
    pub uri: Uri,
    pub version: i32,
    pub contents: String,
}

#[derive(Debug)]
pub struct UpdateInkDocument {
    pub uri: Uri,
    pub version: i32,
    pub range: UpdateRange,
    pub new_text: String,
}

#[derive(Debug)]
pub enum UpdateRange {
    Range(Range),
    All,
}

#[derive(Debug)]
pub struct RemoveInkDocument {
    pub uri: Uri,
}

#[derive(Debug)]
pub struct RenameInkDocument {
    pub old_uri: Uri,
    pub new_uri: Uri,
}

pub mod multimap;
pub mod reference_map;
