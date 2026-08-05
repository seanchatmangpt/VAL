#!/usr/bin/env python3
"""Deterministic independent verifier for the ggen-rendered Rust manifest."""
from __future__ import annotations
import argparse, hashlib, json, re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ONTOLOGY = ROOT / "ontology" / "pddl-lsp.ttl"
OUTPUT = ROOT / "src" / "generated" / "capability_manifest.rs"
RECEIPT = ROOT / "receipts" / "pddl-lsp-generation.json"

def parse() -> tuple[list[str], list[str], list[str]]:
    text = ONTOLOGY.read_text()
    methods = re.findall(r'lsp:methodName\s+"([^"]+)"', text)
    vertices = re.findall(r'a\s+pddl:LsifVertex\s*;\s*rdfs:label\s+"([^"]+)"', text)
    edges = re.findall(r'a\s+pddl:LsifEdge\s*;\s*rdfs:label\s+"([^"]+)"', text)
    return sorted(methods), sorted(vertices), sorted(edges)

def render() -> str:
    methods, vertices, edges = parse()
    def arr(name: str, values: list[str]) -> str:
        body = "\n".join(f'    {json.dumps(value)},' for value in values)
        return f"pub const {name}: &[&str] = &[\n{body}\n];\n"
    return "// @generated from ontology/pddl-lsp.ttl; do not edit.\n\n" + arr("LSP_METHODS", methods) + "\n" + arr("LSIF_VERTEX_LABELS", vertices) + "\n" + arr("LSIF_EDGE_LABELS", edges)

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    output = render()
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != output:
            raise SystemExit("generated manifest drift: run scripts/render_ontology.py")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(output)
    ontology_bytes = ONTOLOGY.read_bytes()
    generated_bytes = output.encode()
    payload = {
        "status": "PARTIAL_ALIVE",
        "generator": "ggen ontology projection independent verifier",
        "hash_algorithm": "SHA-256",
        "source": str(ONTOLOGY.relative_to(ROOT)),
        "artifact": str(OUTPUT.relative_to(ROOT)),
        "source_sha256": hashlib.sha256(ontology_bytes).hexdigest(),
        "artifact_sha256": hashlib.sha256(generated_bytes).hexdigest(),
        "lsp_methods": len(parse()[0]),
        "lsif_vertices": len(parse()[1]),
        "lsif_edges": len(parse()[2]),
    }
    RECEIPT.parent.mkdir(parents=True, exist_ok=True)
    RECEIPT.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return 0
if __name__ == "__main__": raise SystemExit(main())
