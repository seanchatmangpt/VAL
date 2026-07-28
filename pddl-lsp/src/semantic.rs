use crate::parser::{atom, list, parse, ParseIssue, ParseResult, SExpr, Span, Token, TokenKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const REQUIREMENTS: &[&str] = &[
    ":strips", ":typing", ":negative-preconditions", ":disjunctive-preconditions",
    ":equality", ":existential-preconditions", ":universal-preconditions",
    ":quantified-preconditions", ":conditional-effects", ":fluents", ":numeric-fluents",
    ":adl", ":durative-actions", ":duration-inequalities", ":continuous-effects",
    ":derived-predicates", ":timed-initial-literals", ":preferences", ":constraints",
    ":action-costs", ":goal-utilities", ":method-preconditions", ":hierarchy",
];

pub const KEYWORDS: &[&str] = &[
    "define", "domain", "problem", ":domain", ":requirements", ":types", ":constants",
    ":predicates", ":functions", ":action", ":durative-action", ":parameters",
    ":precondition", ":condition", ":effect", ":duration", ":derived", ":objects",
    ":init", ":goal", ":metric", ":constraints", ":method", ":task", ":tasks",
    "and", "or", "not", "imply", "exists", "forall", "when", "assign", "increase",
    "decrease", "scale-up", "scale-down", "at", "over", "start", "end", "all",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum SymbolKind {
    Domain,
    Problem,
    Requirement,
    Type,
    Constant,
    Object,
    Predicate,
    Function,
    Action,
    Method,
    Parameter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symbol {
    pub name: String,
    pub normalized: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub container: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticIssue {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub severity: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentModel {
    pub text: String,
    pub parse: ParseResult,
    pub symbols: Vec<Symbol>,
    pub issues: Vec<SemanticIssue>,
    pub domain_name: Option<String>,
    pub problem_name: Option<String>,
}

impl DocumentModel {
    pub fn analyze(text: impl Into<String>) -> Self {
        let text = text.into();
        let parse = parse(&text);
        let mut model = Self {
            text,
            parse,
            symbols: Vec::new(),
            issues: Vec::new(),
            domain_name: None,
            problem_name: None,
        };
        model.collect_symbols();
        model.collect_issues();
        model
    }

    pub fn token_at_offset(&self, offset: usize) -> Option<&Token> {
        self.parse.tokens.iter().find(|token| {
            matches!(token.kind, TokenKind::Atom) && token.span.start <= offset && offset <= token.span.end
        })
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<&Symbol> {
        let token = self.token_at_offset(offset)?;
        let normalized = normalize(&token.text);
        self.symbols.iter().find(|symbol| symbol.normalized == normalized)
    }

    pub fn definition(&self, name: &str) -> Option<&Symbol> {
        let normalized = normalize(name);
        self.symbols.iter().find(|symbol| symbol.normalized == normalized)
    }

    pub fn references(&self, name: &str) -> Vec<Span> {
        let normalized = normalize(name);
        self.parse.tokens.iter()
            .filter(|token| matches!(token.kind, TokenKind::Atom) && normalize(&token.text) == normalized)
            .map(|token| token.span)
            .collect()
    }

    pub fn offset(&self, line: u32, character: u32) -> usize {
        let mut current_line = 0u32;
        let mut offset = 0usize;
        for segment in self.text.split_inclusive('\n') {
            if current_line == line {
                return offset + usize::min(character as usize, segment.trim_end_matches('\n').len());
            }
            offset += segment.len();
            current_line += 1;
        }
        self.text.len()
    }

    fn collect_symbols(&mut self) {
        let roots = self.parse.roots.clone();
        for root in &roots {
            self.walk(root, None);
        }
    }

    fn walk(&mut self, expr: &SExpr, container: Option<&str>) {
        let Some(items) = list(expr) else { return; };
        if items.is_empty() { return; }
        let head = atom(&items[0]).map(normalize).unwrap_or_default();
        match head.as_str() {
            "domain" if items.len() > 1 => self.add_named(&items[1], SymbolKind::Domain, container, None),
            "problem" if items.len() > 1 => self.add_named(&items[1], SymbolKind::Problem, container, None),
            ":action" | ":durative-action" if items.len() > 1 => {
                let name = atom(&items[1]).unwrap_or_default().to_string();
                self.add_named(&items[1], SymbolKind::Action, container, Some(signature(items)));
                for child in items.iter().skip(2) { self.walk(child, Some(&name)); }
                return;
            }
            ":method" if items.len() > 1 => self.add_named(&items[1], SymbolKind::Method, container, Some(signature(items))),
            ":predicates" => {
                for child in items.iter().skip(1) {
                    if let Some(parts) = list(child) {
                        if let Some(name) = parts.first() { self.add_named(name, SymbolKind::Predicate, container, Some(signature(parts))); }
                    }
                }
            }
            ":functions" => {
                for child in items.iter().skip(1) {
                    if let Some(parts) = list(child) {
                        if let Some(name) = parts.first() { self.add_named(name, SymbolKind::Function, container, Some(signature(parts))); }
                    }
                }
            }
            ":types" => self.add_flat(items, SymbolKind::Type, container),
            ":constants" => self.add_flat(items, SymbolKind::Constant, container),
            ":objects" => self.add_flat(items, SymbolKind::Object, container),
            ":requirements" => {
                for item in items.iter().skip(1) {
                    if let Some(value) = atom(item) { self.symbols.push(Symbol { name: value.to_string(), normalized: normalize(value), kind: SymbolKind::Requirement, span: item.span, container: container.map(str::to_string), signature: None }); }
                }
            }
            ":parameters" => self.add_parameters(items, container),
            _ => {}
        }
        for child in items { self.walk(child, container); }
    }

    fn add_named(&mut self, expr: &SExpr, kind: SymbolKind, container: Option<&str>, signature: Option<String>) {
        let Some(name) = atom(expr) else { return; };
        if kind == SymbolKind::Domain { self.domain_name = Some(name.to_string()); }
        if kind == SymbolKind::Problem { self.problem_name = Some(name.to_string()); }
        self.symbols.push(Symbol { name: name.to_string(), normalized: normalize(name), kind, span: expr.span, container: container.map(str::to_string), signature });
    }

    fn add_flat(&mut self, items: &[SExpr], kind: SymbolKind, container: Option<&str>) {
        for item in items.iter().skip(1) {
            let Some(value) = atom(item) else { continue; };
            if value == "-" || value.starts_with(':') { continue; }
            self.symbols.push(Symbol { name: value.to_string(), normalized: normalize(value), kind, span: item.span, container: container.map(str::to_string), signature: None });
        }
    }

    fn add_parameters(&mut self, items: &[SExpr], container: Option<&str>) {
        for child in items.iter().skip(1) {
            let source: Vec<&SExpr> = list(child).map(|v| v.iter().collect()).unwrap_or_else(|| vec![child]);
            for item in source {
                if let Some(value) = atom(item) {
                    if value.starts_with('?') {
                        self.symbols.push(Symbol { name: value.to_string(), normalized: normalize(value), kind: SymbolKind::Parameter, span: item.span, container: container.map(str::to_string), signature: None });
                    }
                }
            }
        }
    }

    fn collect_issues(&mut self) {
        for ParseIssue { code, message, span } in self.parse.issues.clone() {
            self.issues.push(SemanticIssue { code, message, span, severity: 1 });
        }
        if self.parse.roots.is_empty() {
            self.issues.push(SemanticIssue { code: "PDDL_EMPTY_DOCUMENT", message: "expected a PDDL define form".to_string(), span: zero_span(), severity: 2 });
        }
        let has_define = self.parse.roots.iter().any(|root| list(root).and_then(|x| x.first()).and_then(atom).map(normalize).as_deref() == Some("define"));
        if !has_define && !self.parse.roots.is_empty() {
            self.issues.push(SemanticIssue { code: "PDDL_DEFINE_REQUIRED", message: "top-level form must be '(define ...)'".to_string(), span: self.parse.roots[0].span, severity: 1 });
        }
        for symbol in self.symbols.iter().filter(|s| s.kind == SymbolKind::Requirement) {
            if !REQUIREMENTS.contains(&symbol.normalized.as_str()) {
                self.issues.push(SemanticIssue { code: "PDDL_UNKNOWN_REQUIREMENT", message: format!("unknown requirement '{}'", symbol.name), span: symbol.span, severity: 2 });
            }
        }
        let mut seen: BTreeMap<(SymbolKind, String), Span> = BTreeMap::new();
        for symbol in &self.symbols {
            if matches!(symbol.kind, SymbolKind::Parameter | SymbolKind::Requirement) { continue; }
            let key = (symbol.kind, symbol.normalized.clone());
            if seen.insert(key, symbol.span).is_some() {
                self.issues.push(SemanticIssue { code: "PDDL_DUPLICATE_SYMBOL", message: format!("duplicate {:?} '{}'", symbol.kind, symbol.name), span: symbol.span, severity: 1 });
            }
        }
    }

    pub fn completion_words(&self) -> Vec<String> {
        let mut words: BTreeSet<String> = KEYWORDS.iter().map(|s| (*s).to_string()).collect();
        words.extend(REQUIREMENTS.iter().map(|s| (*s).to_string()));
        words.extend(self.symbols.iter().map(|s| s.name.clone()));
        words.into_iter().collect()
    }
}

fn signature(items: &[SExpr]) -> String {
    items.iter().filter_map(atom).collect::<Vec<_>>().join(" ")
}

fn normalize(value: &str) -> String { value.to_ascii_lowercase() }

fn zero_span() -> Span { Span { start: 0, end: 0, start_line: 0, start_character: 0, end_line: 0, end_character: 0 } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_index_finds_pddl_symbols() {
        let model = DocumentModel::analyze("(define (domain d) (:requirements :strips) (:predicates (at ?x)) (:action move :parameters (?x) :precondition (at ?x) :effect (not (at ?x))))");
        assert!(model.symbols.iter().any(|s| s.kind == SymbolKind::Domain && s.name == "d"));
        assert!(model.symbols.iter().any(|s| s.kind == SymbolKind::Predicate && s.name == "at"));
        assert!(model.symbols.iter().any(|s| s.kind == SymbolKind::Action && s.name == "move"));
    }

    #[test]
    fn unknown_requirement_is_diagnostic() {
        let model = DocumentModel::analyze("(define (domain d) (:requirements :telepathy))");
        assert!(model.issues.iter().any(|issue| issue.code == "PDDL_UNKNOWN_REQUIREMENT"));
    }
}
