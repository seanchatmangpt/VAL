# VAL PDDL LSP

A PDDL/HDDL language server manufactured on `lsp-max` with an ontology-rendered capability manifest.

## Bound state

- **Overall:** `ALIVE` on the nightly Rust rail. The ontology projection, Rust test suite, and clippy with `-D warnings` execute successfully in GitHub Actions.
- **LSP 3.18:** all 73 server-handler routes are wired through `lsp-max`; PDDL semantics override synchronization, diagnostics, hover, completion, navigation, symbols, folding, semantic tokens, formatting, and commands. Route-level ontology states intentionally remain `CANDIDATE`/`WIRED` unless that specific behavior has its own executable receipt.
- **LSIF 0.6:** the exporter covers all 24 vertex labels and 21 edge labels, then adds PDDL symbols, diagnostics, monikers, and receipts. The vocabulary-closure behavioral test is executable and green.
- **ggen:** `ontology/pddl-lsp.ttl` is the source of truth. `ggen.toml`, SPARQL queries, and Tera templates render `src/generated/capability_manifest.rs`; CI refuses projection drift.
- **VAL:** this crate lives with VAL and exposes `pddl.validate`; a native VAL process adapter can replace the built-in structural validator without changing the LSP surface.

## Verify

```bash
python3 pddl-lsp/scripts/render_ontology.py --check
python3 pddl-lsp/scripts/verify_ontology.py
cargo +nightly test --manifest-path pddl-lsp/Cargo.toml --all-features
cargo +nightly clippy --manifest-path pddl-lsp/Cargo.toml --all-targets --all-features -- -D warnings
```

## Run

```bash
cargo +nightly run --manifest-path pddl-lsp/Cargo.toml --bin pddl-lsp
```

Configure the client language id as `pddl` for `*.pddl` and `hddl` for `*.hddl`.
