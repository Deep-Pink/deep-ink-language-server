use std::collections::HashSet;

use tower_lsp_server::ls_types::{Range, Uri};

use crate::db::InkDiagnostic;

pub mod deep_ink_nodes {
    include!(concat!(env!("OUT_DIR"), "/deep_ink_nodes.rs"));
}

// mod deep_ink_queries {
//     include!(concat!(env!("OUT_DIR"), "/deep_ink_queries.rs"));
// }

pub mod ink_nodes {
    include!(concat!(env!("OUT_DIR"), "/ink_nodes.rs"));
}

mod ink_queries {
    include!(concat!(env!("OUT_DIR"), "/ink_queries.rs"));
}

pub mod db;
pub mod ropey_text_provider;

pub enum DbMessage {
    Start,
    Open(OpenInkDocument),
    Update(Vec<UpdateInkDocument>),
    Remove(RemoveInkDocument),
    Rename(RenameInkDocument),
}

pub enum LspMessage {
    Start,
    Diagnostics(Vec<InkDiagnostic>),
}

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
    pub range: Option<Range>,
    pub new_text: String,
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
