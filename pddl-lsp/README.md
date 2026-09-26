# VAL PDDL LSP

A PDDL/HDDL language server manufactured on `lsp-max` with an ontology-rendered capability manifest.

## Bound state

- **Overall:** `ALIVE` on the nightly Rust rail. The ontology projection, Rust test suite, and clippy with `-D warnings` execute successfully in GitHub Actions.
- **LSP 3.18:** all 73 server-handler routes are wired through `lsp-max`; PDDL semantics override synchronization, diagnostics, hover, completion, navigation, symbols, folding, semantic tokens, formatting, and commands. Route-level ontology states intentionally remain `CANDIDATE`/`WIRED` unless that specific behavior has its own executable receipt.
- **LSIF 0.6:** the exporter covers all 24 vertex labels and 21 edge labels, then adds PDDL symbols, diagnostics, monikers, and receipts. The vocabulary-closure behavioral test is executable and green.
- **ggen:** `ontology/pddl-lsp.ttl` is the source of truth. `ggen.toml`, SPARQL queries, and Tera templates render `src/generated/capability_manifest.rs`; CI refuses projection drift.
- **VAL:** this crate lives with VAL and exposes `pddl.validate`; a native VAL process adapter can replace the built-in structural validator without changing the LSP surface.

## HDDL + FOND semantic court

`src/semantic.rs` (hand-written; not an ontology projection) admits HDDL and FOND documents:

- `(:task name :parameters (...))` is indexed as `SymbolKind::Task` with its arity.
- Problem `(:htn ...)` networks and `:subtasks`/`:ordered-subtasks`/`:tasks`/`:ordered-tasks`/
  `:ordering` are recognized; `oneof` and the `:non-deterministic` requirement are known.
- `HDDL_UNDEFINED_TASK`: a method `:task` naming no declared compound task (or naming a
  primitive action), or a subtask naming no declared task/action.
- `HDDL_METHOD_ARITY_MISMATCH` / `HDDL_SUBTASK_ARITY_MISMATCH`: argument count differs from the
  declared `:parameters`.
- `FOND_ONEOF_OUTSIDE_EFFECT`: `oneof` anywhere other than inside an action `:effect`.
- `DocumentModel::check_problem_against_domain` checks a separate problem file's `(:htn ...)`
  network against a domain document.

Fixtures in `tests/fixtures/`: an IPC-style HDDL transport domain/problem (zero issues), a
FOND `oneof` tireworld domain (zero issues), and a SHOP-shaped `(defproblem ...)` document
mirroring ggen_igniter `semantic-jira-pack/templates/plan.hddl.eex` output, which is refused
with `PDDL_DEFINE_REQUIRED` plus `HDDL_UNDEFINED_TASK` for its undeclared tasks.

## Subprocess oracle: `val-pddl-check`

A non-LSP binary over the same court, for callers that cannot hold an LSP session
(ash_pplan, ferroplan, beam4pm, wasm4pm-powl-to-hddl). Logic lives in `src/check.rs`.

```bash
cargo +nightly run --manifest-path pddl-lsp/Cargo.toml --bin val-pddl-check -- \
  --domain pddl-lsp/tests/fixtures/ipc-transport-domain.hddl \
  pddl-lsp/tests/fixtures/ipc-transport-problem.hddl
```

- `val-pddl-check [--domain DOMAIN_FILE] FILE...`; `FILE` may be `-` for stdin.
- stdout is one JSON object (`schema: "val-pddl-check/v1"`): `admitted`, `files[]`
  (`path`, `role`, `domain_name`, `problem_name`, `checked_against`, `issue_count`) and
  `issues[]` (`file`, `check`, `code`, `message`, `span`, `severity`).
- Exit `0` = admitted (zero issues), `1` = refused (any issue), `2` = usage/I-O error
  (JSON `error` object with `CLI_USAGE` or `CLI_IO_ERROR`).
- Every input gets `DocumentModel::analyze` (`check: "document"`). A problem's `(:htn ...)`
  network is checked with `check_problem_against_domain` (`check: "problem-against-domain"`)
  against `--domain`, else against an input declaring the problem's `(:domain X)`; if neither
  resolves, it is refused with `CLI_DOMAIN_UNRESOLVED` (`check: "domain-resolution"`) rather
  than admitted unjudged.
- `tests/check_cli.rs` spawns the built binary on every fixture and asserts its issue codes
  equal the library court's, alone and against every domain fixture.

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
