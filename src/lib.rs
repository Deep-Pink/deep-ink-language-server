use im::hashmap::HashMap;
use tokio::sync::mpsc::Sender;
use tower_lsp_server::ls_types::{Diagnostic, Range, Uri};

use crate::{db::InkDiagnostic, multimap::Multimap};
pub mod db;
pub mod ropey_text_provider;

pub mod deep_ink_type_sitter {
    pub mod nodes {
        include!(concat!(env!("OUT_DIR"), "/deep_ink_nodes.rs"));
    }
    pub mod deep_ink_queries {
        // include!(concat!(env!("OUT_DIR"), "/deep_ink_queries.rs"));
    }
}

pub mod ink_type_sitter {
    pub mod nodes {
        include!(concat!(env!("OUT_DIR"), "/ink_nodes.rs"));
    }
    pub mod queries {
        include!(concat!(env!("OUT_DIR"), "/ink_queries.rs"));
    }
}

pub enum DbMessage {
    RequestDiagnostics(Sender<DiagnosticsMessage>),
    Open(OpenInkDocument, Sender<DiagnosticsMessage>),
    Update(Vec<UpdateInkDocument>, Sender<DiagnosticsMessage>),
    Remove(RemoveInkDocument, Sender<DiagnosticsMessage>),
    Rename(RenameInkDocument, Sender<DiagnosticsMessage>),
}

#[derive(Debug, Clone)]
pub struct DiagnosticsMessage(pub Multimap<(Uri, i32), InkDiagnostic>);

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
