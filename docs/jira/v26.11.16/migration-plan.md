# VAL migration plan v26.11.16

Standing: UNVERIFIED

## Identity
- Path: /Users/sac/VAL
- Remote: https://github.com/seanchatmangpt/VAL (fork of KCL-Planning/VAL)
- Branch: master@1aad5f4 (tracks origin/master, behind 5)
- Last commit: 2026-08-04 — "Merge PR #1: ontology-rendered PDDL LSP"
- Commits: 94; size: 29 MB
- Languages/build: C++ (CMakeLists.txt), Lean 4 (lakefile.toml, lean-toolchain), pddl-lsp/
- Nested repos: none

## Observed state
- Dirty: 0 tracked. Untracked: 10 paths — `VAL.lean`, `VAL/` (Lean sources), `lakefile.toml`,
  `lake-manifest.json`, `lean-toolchain`, `docs/` (jira dir). A Lean-4 shim was started and
  never committed.
- Ahead 0 / behind 5 of origin/master.
- Stash: 0.
- Unpushed local branches (2): `v26926/val-pddl-check-cli-subprocess-oracle` (8b2f304),
  `v26926/val-pddl-lsp-hddl-fond-semantic-court` (d1ad3fa) — no upstream.
- Build dirs: none present (no build/, no .lake/). pddl-lsp/ is 132K source.
- Submodules: none.

## What it is
Fork of the VAL PDDL plan-validation toolkit (C++), extended with an ontology-rendered PDDL
language server (`pddl-lsp/`) and an uncommitted Lean 4 harness. Upstream work lives on
origin/master; the v26.9.26 LSP branches are local-only.

## Classification
WARM (census agrees: last touch Aug 2026, two unpushed feature branches).

## Migration risk
- Lost if not copied: 2 unpushed branches (HDDL/FOND semantic court, subprocess oracle), the
  untracked Lean 4 shim (VAL.lean, VAL/, lakefile.toml, lean-toolchain, lake-manifest.json).
- No secrets or data files observed.
- Rating: MEDIUM.

## Recommended action
COMMIT-PUSH

1. `cd /Users/sac/VAL && git status --porcelain` — confirm the Lean files are the only untracked.
2. DECISION: keep the Lean 4 shim? If yes: `git checkout -b feat/lean4-shim && git add VAL.lean VAL lakefile.toml lake-manifest.json lean-toolchain docs && git commit -m "feat(lean): VAL Lean 4 shim"`.
3. `git push -u origin feat/lean4-shim`.
4. `git push -u origin v26926/val-pddl-check-cli-subprocess-oracle v26926/val-pddl-lsp-hddl-fond-semantic-court`.
5. `git fetch && git log --oneline master..origin/master` — review the 5 upstream commits; then
   `git merge --ff-only origin/master`.
6. `git branch -vv` — verify every branch shows an upstream and no `ahead`.
7. On the new machine: `git clone https://github.com/seanchatmangpt/VAL`.

## First commands on the new machine
- `cmake -S . -B build && cmake --build build` (C++ validator)
- `lake build` (Lean shim, if kept)
- `cd pddl-lsp && <its documented check>` — see pddl-lsp/README

## See Also
- ../v26.9.19/
- /Users/sac/docs/jira/v26.11.16/README.md
