use crate::semantic::DocumentModel;
use lsp_max::lsp_types_max::{Position, Range};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct WorkspaceState {
    documents: BTreeMap<String, VersionedDocument>,
}

#[derive(Debug, Clone)]
pub struct VersionedDocument {
    pub version: i32,
    pub model: DocumentModel,
}

impl WorkspaceState {
    pub fn open(&mut self, uri: impl Into<String>, version: i32, text: String) {
        self.documents.insert(uri.into(), VersionedDocument { version, model: DocumentModel::analyze(text) });
    }

    pub fn change(&mut self, uri: &str, version: i32, text: String) {
        self.open(uri.to_string(), version, text);
    }

    pub fn close(&mut self, uri: &str) { self.documents.remove(uri); }

    pub fn get(&self, uri: &str) -> Option<&VersionedDocument> { self.documents.get(uri) }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &VersionedDocument)> { self.documents.iter() }
}

pub fn lsp_range(span: crate::parser::Span) -> Range {
    Range::new(
        Position::new(span.start_line, span.start_character),
        Position::new(span.end_line, span.end_character),
    )
}
