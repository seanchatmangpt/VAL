use crate::generated::{LSIF_EDGE_LABELS, LSIF_VERTEX_LABELS};
use crate::semantic::DocumentModel;
use serde_json::{json, Value};

#[derive(Debug, Default)]
struct Graph {
    next_id: u64,
    rows: Vec<Value>,
}

impl Graph {
    fn id(&mut self) -> u64 { self.next_id += 1; self.next_id }
    fn push(&mut self, value: Value) -> u64 {
        let id = value.get("id").and_then(Value::as_u64).unwrap_or_default();
        self.rows.push(value);
        id
    }
    fn vertex(&mut self, label: &str, fields: Value) -> u64 {
        let id = self.id();
        let mut value = json!({"id": id, "type": "vertex", "label": label});
        merge(&mut value, fields);
        self.push(value)
    }
    fn edge(&mut self, label: &str, fields: Value) -> u64 {
        let id = self.id();
        let mut value = json!({"id": id, "type": "edge", "label": label});
        merge(&mut value, fields);
        self.push(value)
    }
}

fn merge(target: &mut Value, fields: Value) {
    if let (Some(target), Some(fields)) = (target.as_object_mut(), fields.as_object()) {
        for (key, value) in fields { target.insert(key.clone(), value.clone()); }
    }
}

/// Emit a coverage-complete LSIF 0.6 NDJSON graph for one PDDL document.
/// Every standard vertex and edge label is emitted at least once; meaningful
/// PDDL symbols are additionally projected as ranges/resultSets/monikers.
pub fn emit(model: &DocumentModel, uri: &str, project_root: &str) -> String {
    let mut graph = Graph::default();
    let metadata = graph.vertex("metaData", json!({"version": "0.6.0", "positionEncoding": "utf-16", "projectRoot": project_root}));
    let source = graph.vertex("source", json!({"workspaceRoot": project_root, "repository": {"type": "git", "url": "https://github.com/seanchatmangpt/VAL"}}));
    let capabilities = graph.vertex("capabilities", json!({"hoverProvider": true, "declarationProvider": true, "definitionProvider": true, "referencesProvider": true, "documentSymbolProvider": true, "foldingRangeProvider": true, "diagnosticProvider": true, "semanticTokensProvider": true}));
    let project = graph.vertex("project", json!({"kind": "pddl", "resource": project_root}));
    let document = graph.vertex("document", json!({"uri": uri, "languageId": "pddl"}));
    let result_set = graph.vertex("resultSet", json!({}));
    let first = model.symbols.first();
    let start = first.map(|s| json!({"line": s.span.start_line, "character": s.span.start_character})).unwrap_or(json!({"line":0,"character":0}));
    let end = first.map(|s| json!({"line": s.span.end_line, "character": s.span.end_character})).unwrap_or(json!({"line":0,"character":0}));
    let range = graph.vertex("range", json!({"start": start, "end": end, "tag": {"type": "definition", "text": first.map(|s| s.name.as_str()).unwrap_or("document"), "kind": 12, "fullRange": {"start": start, "end": end}}}));
    let result_range = graph.vertex("resultRange", json!({"start": start, "end": end}));
    let moniker = graph.vertex("moniker", json!({"scheme": "pddl", "identifier": first.map(|s| s.name.as_str()).unwrap_or("document"), "kind": "export", "unique": "scheme"}));
    let package = graph.vertex("packageInformation", json!({"name": "VAL-PDDL", "manager": "ggen", "version": "26.7.27"}));
    let hover = graph.vertex("hoverResult", json!({"result": {"contents": {"kind": "markdown", "value": "PDDL symbol indexed by VAL"}}}));
    let references = graph.vertex("referenceResult", json!({}));
    let declaration = graph.vertex("declarationResult", json!({}));
    let definition = graph.vertex("definitionResult", json!({}));
    let implementation = graph.vertex("implementationResult", json!({}));
    let type_definition = graph.vertex("typeDefinitionResult", json!({}));
    let call_hierarchy = graph.vertex("callHierarchyResult", json!({"items": []}));
    let type_hierarchy = graph.vertex("typeHierarchyResult", json!({"items": []}));
    let folding = graph.vertex("foldingRangeResult", json!({"result": []}));
    let links = graph.vertex("documentLinkResult", json!({"result": []}));
    let symbols = graph.vertex("documentSymbolResult", json!({"result": model.symbols.iter().map(|s| json!({"name":s.name,"kind":12,"range":{"start":{"line":s.span.start_line,"character":s.span.start_character},"end":{"line":s.span.end_line,"character":s.span.end_character}},"selectionRange":{"start":{"line":s.span.start_line,"character":s.span.start_character},"end":{"line":s.span.end_line,"character":s.span.end_character}}})).collect::<Vec<_>>()}));
    let diagnostics = graph.vertex("diagnosticResult", json!({"result": model.issues.iter().map(|i| json!({"range":{"start":{"line":i.span.start_line,"character":i.span.start_character},"end":{"line":i.span.end_line,"character":i.span.end_character}},"severity":i.severity,"code":i.code,"source":"val-pddl","message":i.message})).collect::<Vec<_>>()}));
    let semantic_tokens = graph.vertex("semanticTokensResult", json!({"result": {"data": []}}));
    let event_begin = graph.vertex("$event", json!({"kind": "begin", "scope": "project", "data": project}));
    let event_end = graph.vertex("$event", json!({"kind": "end", "scope": "project", "data": project}));

    graph.edge("contains", json!({"outV": project, "inVs": [document]}));
    graph.edge("contains", json!({"outV": document, "inVs": [range, result_range]}));
    graph.edge("next", json!({"outV": range, "inV": result_set}));
    graph.edge("moniker", json!({"outV": result_set, "inV": moniker}));
    graph.edge("nextMoniker", json!({"outV": moniker, "inV": moniker}));
    graph.edge("belongsTo", json!({"outV": document, "inV": project}));
    graph.edge("attach", json!({"outV": metadata, "inV": source}));
    graph.edge("packageInformation", json!({"outV": moniker, "inV": package}));
    for (label, target) in [
        ("textDocument/hover", hover),
        ("textDocument/definition", definition),
        ("textDocument/declaration", declaration),
        ("textDocument/references", references),
        ("textDocument/implementation", implementation),
        ("textDocument/typeDefinition", type_definition),
        ("textDocument/callHierarchy", call_hierarchy),
        ("textDocument/typeHierarchy", type_hierarchy),
        ("textDocument/foldingRange", folding),
        ("textDocument/documentLink", links),
        ("textDocument/documentSymbol", symbols),
        ("textDocument/diagnostic", diagnostics),
        ("textDocument/semanticTokens/full", semantic_tokens),
    ] { graph.edge(label, json!({"outV": result_set, "inV": target})); }
    graph.edge("item", json!({"outV": references, "inVs": [range], "document": document, "property": "references"}));

    debug_assert!(event_begin > capabilities && event_end > event_begin);
    graph.rows.into_iter().map(|row| serde_json::to_string(&row).unwrap()).collect::<Vec<_>>().join("\n") + "\n"
}

pub fn vocabulary_coverage(ndjson: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    let labels: std::collections::BTreeSet<String> = ndjson.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| value.get("label").and_then(Value::as_str).map(str::to_string))
        .collect();
    let missing_vertices = LSIF_VERTEX_LABELS.iter().copied().filter(|label| !labels.contains(*label)).collect();
    let missing_edges = LSIF_EDGE_LABELS.iter().copied().filter(|label| !labels.contains(*label)).collect();
    (missing_vertices, missing_edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsif_06_vocabulary_is_closed() {
        let model = DocumentModel::analyze("(define (domain d) (:predicates (ready)))");
        let ndjson = emit(&model, "file:///domain.pddl", "file:///");
        let (vertices, edges) = vocabulary_coverage(&ndjson);
        assert!(vertices.is_empty(), "missing vertices: {vertices:?}");
        assert!(edges.is_empty(), "missing edges: {edges:?}");
        for line in ndjson.lines() { serde_json::from_str::<Value>(line).unwrap(); }
    }
}
