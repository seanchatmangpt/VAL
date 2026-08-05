#!/usr/bin/env python3
from pathlib import Path
import re
ROOT=Path(__file__).resolve().parents[1]
text=(ROOT/'ontology/pddl-lsp.ttl').read_text()
methods=re.findall(r'lsp:methodName\s+"([^"]+)"', text)
vertices=re.findall(r'a\s+pddl:LsifVertex\s*;\s*rdfs:label\s+"([^"]+)"', text)
edges=re.findall(r'a\s+pddl:LsifEdge\s*;\s*rdfs:label\s+"([^"]+)"', text)
assert len(methods)==73, f"expected 73 lsp-max server routes, got {len(methods)}"
assert len(vertices)==24, f"expected 24 LSIF vertices, got {len(vertices)}"
assert len(edges)==21, f"expected 21 LSIF edges, got {len(edges)}"
assert len(set(methods))==len(methods)
assert len(set(vertices))==len(vertices)
assert len(set(edges))==len(edges)
print(f"PARTIAL_ALIVE lsp={len(methods)} lsif_vertices={len(vertices)} lsif_edges={len(edges)}")
