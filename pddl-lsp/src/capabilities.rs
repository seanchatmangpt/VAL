use lsp_max::lsp_types_max::ServerCapabilities;
use serde_json::json;

/// Build the entire LSP 3.18 server-capability surface. The lsp-max router owns
/// transport/lifecycle coverage; PDDL-specific handlers override the routes with
/// semantic behavior while all remaining routes retain lawful framework defaults.
pub fn server_capabilities() -> ServerCapabilities {
    let value = json!({
        "positionEncoding": "utf-16",
        "textDocumentSync": {"openClose": true, "change": 2, "willSave": true, "willSaveWaitUntil": true, "save": {"includeText": true}},
        "selectionRangeProvider": true,
        "hoverProvider": true,
        "completionProvider": {"resolveProvider": true, "triggerCharacters": [":", "?", "-", "("]},
        "signatureHelpProvider": {"triggerCharacters": ["(", " "]},
        "declarationProvider": true,
        "definitionProvider": true,
        "typeDefinitionProvider": true,
        "implementationProvider": true,
        "referencesProvider": true,
        "documentHighlightProvider": true,
        "documentSymbolProvider": {"label": "PDDL"},
        "workspaceSymbolProvider": {"resolveProvider": true},
        "codeActionProvider": {"resolveProvider": true, "codeActionKinds": ["quickfix", "refactor.rewrite", "source.fixAll"]},
        "codeLensProvider": {"resolveProvider": true},
        "documentFormattingProvider": true,
        "documentRangeFormattingProvider": true,
        "documentRangesFormattingProvider": true,
        "documentOnTypeFormattingProvider": {"firstTriggerCharacter": ")", "moreTriggerCharacter": ["\n"]},
        "renameProvider": {"prepareProvider": true},
        "documentLinkProvider": {"resolveProvider": true},
        "colorProvider": true,
        "foldingRangeProvider": true,
        "executeCommandProvider": {"commands": ["pddl.validate", "pddl.exportLsif", "pddl.coverage", "pddl.renderOntology"]},
        "workspace": {
            "workspaceFolders": {"supported": true, "changeNotifications": true},
            "fileOperations": {
                "didCreate": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]},
                "willCreate": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]},
                "didRename": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]},
                "willRename": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]},
                "didDelete": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]},
                "willDelete": {"filters": [{"scheme": "file", "pattern": {"glob": "**/*.{pddl,hddl}"}}]}
            }
        },
        "callHierarchyProvider": true,
        "semanticTokensProvider": {
            "legend": {
                "tokenTypes": ["namespace", "type", "class", "function", "method", "variable", "parameter", "keyword", "number", "operator", "comment"],
                "tokenModifiers": ["declaration", "definition", "readonly", "deprecated"]
            },
            "range": true,
            "full": {"delta": true}
        },
        "monikerProvider": true,
        "linkedEditingRangeProvider": true,
        "inlineValueProvider": true,
        "inlayHintProvider": {"resolveProvider": true},
        "diagnosticProvider": {"identifier": "val-pddl", "interFileDependencies": true, "workspaceDiagnostics": true},
        "textDocumentContentProvider": {"schemes": ["file", "untitled"]},
        "inlineCompletionProvider": true,
        "experimental": {
            "pddl": {"dialects": ["PDDL 1.2", "PDDL 2.1", "PDDL 2.2", "PDDL 3.0", "PDDL 3.1", "HDDL"], "validator": "VAL"},
            "lspMax": {"lawState": true, "receipts": "blake3"},
            "lsif": {"version": "0.6.0", "completeVocabulary": true},
            "ggen": {"ontology": "ontology/pddl-lsp.ttl", "config": "ggen.toml"}
        }
    });
    serde_json::from_value(value).expect("static LSP 3.18 capability manifest must deserialize")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_capability_manifest_deserializes() {
        let value = serde_json::to_value(server_capabilities()).unwrap();
        assert_eq!(value["positionEncoding"], "utf-16");
        assert!(value.get("semanticTokensProvider").is_some());
        assert!(value.get("documentRangesFormattingProvider").is_some());
        assert!(value.get("textDocumentContentProvider").is_some());
    }
}
