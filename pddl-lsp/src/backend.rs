use crate::capabilities::server_capabilities;
use crate::document::{lsp_range, WorkspaceState};
use crate::lsif;
use crate::receipt;
use crate::semantic::{DocumentModel, SymbolKind};
use lsp_max::jsonrpc::Result;
use lsp_max::lsp_types_max::*;
use lsp_max::{Client, LanguageServer};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct Backend {
    client: Client,
    state: Arc<RwLock<WorkspaceState>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self { client, state: Arc::new(RwLock::new(WorkspaceState::default())) }
    }

    fn model(&self, uri: &Uri) -> Option<DocumentModel> {
        self.state.read().ok()?.get(uri.as_str()).map(|doc| doc.model.clone())
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("server-owned LSP response must match lsp-types-max")
}

fn diagnostic_json(model: &DocumentModel) -> Vec<Value> {
    model.issues.iter().map(|issue| json!({
        "range": range_json(issue.span),
        "severity": issue.severity,
        "code": issue.code,
        "source": "val-pddl",
        "message": issue.message,
        "data": {"lawState": if issue.severity == 1 {"REFUSED"} else {"UNKNOWN"}}
    })).collect()
}

fn range_json(span: crate::parser::Span) -> Value {
    json!({"start":{"line":span.start_line,"character":span.start_character},"end":{"line":span.end_line,"character":span.end_character}})
}

#[lsp_max::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult { capabilities: server_capabilities(), ..Default::default() })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "VAL PDDL LSP: LSP 3.18 + LSIF 0.6 + ggen ontology").await;
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Ok(mut state) = self.state.write() {
            state.open(params.text_document.uri.as_str(), params.text_document.version, params.text_document.text);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.as_str().to_string();
        let mut text = self.state.read().ok().and_then(|state| state.get(&uri).map(|doc| doc.model.text.clone())).unwrap_or_default();
        for change in params.content_changes {
            if let Some(range) = change.range {
                let model = DocumentModel::analyze(text.clone());
                let start = model.offset(range.start.line, range.start.character);
                let end = model.offset(range.end.line, range.end.character);
                if start <= end && end <= text.len() { text.replace_range(start..end, &change.text); }
            } else { text = change.text; }
        }
        if let Ok(mut state) = self.state.write() { state.change(&uri, params.text_document.version, text); }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Ok(mut state) = self.state.write() { state.close(params.text_document.uri.as_str()); }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(model) = self.model(uri) else { return Ok(None); };
        let position = params.text_document_position_params.position;
        let offset = model.offset(position.line, position.character);
        let Some(token) = model.token_at_offset(offset) else { return Ok(None); };
        let symbol = model.definition(&token.text);
        let value = if let Some(symbol) = symbol {
            format!("### `{}`\n\n- kind: `{:?}`\n- container: `{}`\n- signature: `{}`", symbol.name, symbol.kind, symbol.container.as_deref().unwrap_or("document"), symbol.signature.as_deref().unwrap_or("n/a"))
        } else if token.text.starts_with(':') {
            format!("PDDL keyword or requirement `{}`", token.text)
        } else { return Ok(None); };
        Ok(Some(decode(json!({"contents":{"kind":"markdown","value":value},"range":range_json(token.span)}))))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(model) = self.model(uri) else { return Ok(Some(CompletionResponse::Array(Vec::new()))); };
        let items = model.completion_words().into_iter().map(|label| CompletionItem::new_simple(label.clone(), format!("PDDL completion: {label}"))).collect();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(model) = self.model(uri) else { return Ok(None); };
        let pos = params.text_document_position_params.position;
        let offset = model.offset(pos.line, pos.character);
        let Some(token) = model.token_at_offset(offset) else { return Ok(None); };
        let Some(symbol) = model.definition(&token.text) else { return Ok(None); };
        Ok(Some(GotoDefinitionResponse::Scalar(Location::new(uri.clone(), lsp_range(symbol.span)))))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let Some(model) = self.model(uri) else { return Ok(None); };
        let pos = params.text_document_position.position;
        let offset = model.offset(pos.line, pos.character);
        let Some(token) = model.token_at_offset(offset) else { return Ok(None); };
        Ok(Some(model.references(&token.text).into_iter().map(|span| Location::new(uri.clone(), lsp_range(span))).collect()))
    }

    async fn document_highlight(&self, params: DocumentHighlightParams) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(model) = self.model(uri) else { return Ok(None); };
        let pos = params.text_document_position_params.position;
        let offset = model.offset(pos.line, pos.character);
        let Some(token) = model.token_at_offset(offset) else { return Ok(None); };
        Ok(Some(model.references(&token.text).into_iter().map(|span| decode(json!({"range":range_json(span),"kind":1}))).collect()))
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let Some(model) = self.model(&params.text_document.uri) else { return Ok(None); };
        let symbols = model.symbols.iter().filter(|symbol| symbol.kind != SymbolKind::Parameter).map(|symbol| json!({
            "name": symbol.name,
            "detail": symbol.signature,
            "kind": match symbol.kind { SymbolKind::Domain | SymbolKind::Problem => 2, SymbolKind::Type => 5, SymbolKind::Predicate | SymbolKind::Function => 12, SymbolKind::Action | SymbolKind::Method => 6, _ => 13 },
            "range": range_json(symbol.span),
            "selectionRange": range_json(symbol.span),
            "children": []
        })).collect::<Vec<_>>();
        Ok(Some(decode(Value::Array(symbols))))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let Some(model) = self.model(&params.text_document.uri) else { return Ok(None); };
        let ranges = model.parse.roots.iter().filter(|root| root.span.end_line > root.span.start_line).map(|root| decode(json!({"startLine":root.span.start_line,"startCharacter":root.span.start_character,"endLine":root.span.end_line,"endCharacter":root.span.end_character,"kind":"region"}))).collect();
        Ok(Some(ranges))
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let Some(model) = self.model(&params.text_document.uri) else { return Ok(None); };
        let mut data = Vec::<u32>::new();
        let mut previous_line = 0u32;
        let mut previous_char = 0u32;
        for token in model.parse.tokens.iter().filter(|token| !matches!(token.kind, crate::parser::TokenKind::LeftParen | crate::parser::TokenKind::RightParen)) {
            let delta_line = token.span.start_line - previous_line;
            let delta_start = if delta_line == 0 { token.span.start_character - previous_char } else { token.span.start_character };
            let token_type = if matches!(token.kind, crate::parser::TokenKind::Comment) { 10 } else if token.text.starts_with('?') { 6 } else if token.text.starts_with(':') || crate::semantic::KEYWORDS.contains(&token.text.to_ascii_lowercase().as_str()) { 7 } else if token.text.parse::<f64>().is_ok() { 8 } else { 5 };
            data.extend([delta_line, delta_start, (token.span.end - token.span.start) as u32, token_type, 0]);
            previous_line = token.span.start_line;
            previous_char = token.span.start_character;
        }
        Ok(Some(decode(json!({"data":data}))))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let Some(model) = self.model(&params.text_document.uri) else { return Ok(None); };
        let formatted = format_pddl(&model.text);
        let end_line = model.text.lines().count() as u32 + 1;
        Ok(Some(vec![decode(json!({"range":{"start":{"line":0,"character":0},"end":{"line":end_line,"character":0}},"newText":formatted}))]))
    }

    async fn diagnostic(&self, params: DocumentDiagnosticParams) -> Result<DocumentDiagnosticReportResult> {
        let items = self.model(&params.text_document.uri).map(|model| diagnostic_json(&model)).unwrap_or_default();
        Ok(decode(json!({"kind":"full","items":items})))
    }

    async fn workspace_diagnostic(&self, _params: WorkspaceDiagnosticParams) -> Result<WorkspaceDiagnosticReportResult> {
        let items = self.state.read().ok().map(|state| state.iter().map(|(uri, doc)| json!({"uri":uri,"version":doc.version,"kind":"full","items":diagnostic_json(&doc.model)})).collect::<Vec<_>>()).unwrap_or_default();
        Ok(decode(json!({"items":items})))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        let uri = params.arguments.first().and_then(Value::as_str);
        match params.command.as_str() {
            "pddl.coverage" => {
                let lsp = lsp_max::coverage::lsp_coverage();
                let lsif = lsp_max::lsif::lsif_coverage();
                Ok(Some(json!({"lsp":lsp.summary(),"lsif":lsif.summary(),"ontologyMethods":crate::generated::LSP_METHODS.len(),"lsifVertices":crate::generated::LSIF_VERTEX_LABELS.len(),"lsifEdges":crate::generated::LSIF_EDGE_LABELS.len()})))
            }
            "pddl.validate" => Ok(Some(json!({"diagnostics": uri.and_then(|uri| self.state.read().ok()?.get(uri).map(|doc| diagnostic_json(&doc.model))).unwrap_or_default()}))),
            "pddl.exportLsif" => {
                let Some(uri) = uri else { return Ok(Some(json!({"error":"first argument must be a document URI"}))); };
                let Some(model) = self.state.read().ok().and_then(|state| state.get(uri).map(|doc| doc.model.clone())) else { return Ok(Some(json!({"error":"document not open"}))); };
                let ndjson = lsif::emit(&model, uri, "file:///");
                Ok(Some(json!({"ndjson":ndjson,"receipt":receipt::receipt(format!("lsif:{uri}"), ndjson.as_bytes())})))
            }
            "pddl.renderOntology" => Ok(Some(json!({"command":"ggen sync --config pddl-lsp/ggen.toml","source":"pddl-lsp/ontology/pddl-lsp.ttl"}))),
            _ => Ok(None),
        }
    }
}

fn format_pddl(text: &str) -> String {
    let mut out = String::new();
    let mut indent = 0usize;
    let mut token = String::new();
    let flush = |out: &mut String, token: &mut String| {
        if !token.is_empty() {
            if !(out.ends_with(' ') || out.ends_with('\n') || out.ends_with('(')) { out.push(' '); }
            out.push_str(token);
            token.clear();
        }
    };
    for ch in text.chars() {
        match ch {
            ';' => { flush(&mut out, &mut token); if !out.ends_with('\n') { out.push(' '); } out.push(';'); },
            '(' => { flush(&mut out, &mut token); if !out.is_empty() && !(out.ends_with('\n') || out.ends_with('(') || out.ends_with(' ')) { out.push('\n'); } out.push_str(&"  ".repeat(indent)); out.push('('); indent += 1; },
            ')' => { flush(&mut out, &mut token); indent = indent.saturating_sub(1); out.push(')'); },
            '\n' | '\r' | '\t' | ' ' => flush(&mut out, &mut token),
            _ => token.push(ch),
        }
    }
    flush(&mut out, &mut token);
    if !out.ends_with('\n') { out.push('\n'); }
    out
}
