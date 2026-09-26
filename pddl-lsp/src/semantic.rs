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
    ":non-deterministic",
];

pub const KEYWORDS: &[&str] = &[
    "define", "domain", "problem", ":domain", ":requirements", ":types", ":constants",
    ":predicates", ":functions", ":action", ":durative-action", ":parameters",
    ":precondition", ":condition", ":effect", ":duration", ":derived", ":objects",
    ":init", ":goal", ":metric", ":constraints", ":method", ":task", ":tasks",
    ":ordered-tasks", ":htn", ":subtasks", ":ordered-subtasks", ":ordering", "oneof", "<",
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
    Task,
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
        let mut offset = 0usize;
        for (current_line, segment) in self.text.split_inclusive('\n').enumerate() {
            if current_line as u32 == line {
                return offset + usize::min(character as usize, segment.trim_end_matches('\n').len());
            }
            offset += segment.len();
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
            ":task" if items.len() > 1 && atom(&items[1]).is_some() => self.add_named(&items[1], SymbolKind::Task, container, Some(signature(items))),
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

    /// Indexes the declared names of a typed list (`a b - t c`). The atom after
    /// `-` is a supertype *reference*, not a declaration, and is not indexed.
    fn add_flat(&mut self, items: &[SExpr], kind: SymbolKind, container: Option<&str>) {
        let mut after_dash = false;
        for item in items.iter().skip(1) {
            let is_supertype = std::mem::replace(&mut after_dash, atom(item) == Some("-"));
            if is_supertype { continue; }
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
        self.collect_hddl_issues();
        self.collect_fond_issues();
    }

    /// Declared arity of every task (`SymbolKind::Task`) and primitive action,
    /// keyed by normalized name. Arity = number of `?variables` in `:parameters`.
    pub fn operator_arities(&self) -> BTreeMap<String, (SymbolKind, usize)> {
        let mut arities = BTreeMap::new();
        for root in &self.parse.roots { collect_operator_arities(root, &mut arities); }
        arities
    }

    /// HDDL reference court over methods declared in this document: a method's
    /// `:task` must name a declared compound task with matching arity, and every
    /// subtask must name a declared task or primitive action with matching arity.
    /// Problem-level `(:htn ...)` networks are checked only when the same
    /// document declares a domain; cross-file problems use [`Self::check_problem_against_domain`].
    fn collect_hddl_issues(&mut self) {
        let arities = self.operator_arities();
        let declares_domain = self.domain_name.is_some();
        let mut issues = Vec::new();
        for root in &self.parse.roots {
            visit_lists(root, &mut |items| {
                match head(items).as_deref() {
                    Some(":method") => check_method(items, &arities, &mut issues),
                    Some(":htn") if declares_domain => {
                        for task in network_tasks(items) { check_task_reference(task, &arities, "subtask", &mut issues); }
                    }
                    _ => {}
                }
            });
        }
        self.issues.extend(issues);
    }

    /// FOND court: `oneof` is a non-deterministic *effect* and is admitted only
    /// inside the value of an action's `:effect`.
    fn collect_fond_issues(&mut self) {
        let mut issues = Vec::new();
        for root in &self.parse.roots { check_oneof(root, false, &mut issues); }
        self.issues.extend(issues);
    }

    /// Checks this document's problem-level `(:htn ...)` task network against
    /// the tasks and actions declared by a separately analyzed domain document.
    pub fn check_problem_against_domain(&self, domain: &DocumentModel) -> Vec<SemanticIssue> {
        let arities = domain.operator_arities();
        let mut issues = Vec::new();
        for root in &self.parse.roots {
            visit_lists(root, &mut |items| {
                if head(items).as_deref() == Some(":htn") {
                    for task in network_tasks(items) { check_task_reference(task, &arities, "subtask", &mut issues); }
                }
            });
        }
        issues
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

fn head(items: &[SExpr]) -> Option<String> { items.first().and_then(atom).map(normalize) }

/// Pre-order visit of every list node.
fn visit_lists(expr: &SExpr, f: &mut dyn FnMut(&[SExpr])) {
    let Some(items) = list(expr) else { return; };
    f(items);
    for child in items { visit_lists(child, f); }
}

/// Value following a `:keyword` inside a list (e.g. `:parameters (...)`).
fn keyword_value<'a>(items: &'a [SExpr], keyword: &str) -> Option<&'a SExpr> {
    items.iter().position(|item| atom(item).map(normalize).as_deref() == Some(keyword)).and_then(|i| items.get(i + 1))
}

fn parameter_count(items: &[SExpr]) -> usize {
    keyword_value(items, ":parameters")
        .and_then(list)
        .map(|params| params.iter().filter_map(atom).filter(|v| v.starts_with('?')).count())
        .unwrap_or(0)
}

fn collect_operator_arities(expr: &SExpr, arities: &mut BTreeMap<String, (SymbolKind, usize)>) {
    let Some(items) = list(expr) else { return; };
    let kind = match head(items).as_deref() {
        Some(":task") => Some(SymbolKind::Task),
        Some(":action") | Some(":durative-action") => Some(SymbolKind::Action),
        _ => None,
    };
    if let (Some(kind), Some(name)) = (kind, items.get(1).and_then(atom)) {
        arities.entry(normalize(name)).or_insert((kind, parameter_count(items)));
        return;
    }
    for child in items { collect_operator_arities(child, arities); }
}

/// Task atoms of a task network value: `(and (t1 (a ?x)) (b ?y))`, a single
/// `(t1 (a ?x))`, a single `(a ?x)`, `()`, or a SHOP-style bare sequence.
fn network_entries(value: &SExpr) -> Vec<&SExpr> {
    let Some(items) = list(value) else { return Vec::new(); };
    if items.is_empty() { return Vec::new(); }
    if head(items).as_deref() == Some("and") {
        return items.iter().skip(1).filter_map(network_entry).collect();
    }
    // SHOP-style bare sequence `((a ?x) (b ?y))`: accepted so its references are still checked.
    if list(&items[0]).is_some() {
        return items.iter().filter_map(network_entry).collect();
    }
    network_entry(value).into_iter().collect()
}

fn network_entry(entry: &SExpr) -> Option<&SExpr> {
    let items = list(entry)?;
    match items {
        [label, task] if atom(label).is_some() && list(task).is_some() => Some(task),
        [first, ..] if atom(first).is_some() => Some(entry),
        _ => None,
    }
}

fn network_tasks(items: &[SExpr]) -> Vec<&SExpr> {
    const NETWORK_KEYWORDS: &[&str] = &[":subtasks", ":ordered-subtasks", ":tasks", ":ordered-tasks"];
    let mut tasks = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if atom(item).map(normalize).is_some_and(|k| NETWORK_KEYWORDS.contains(&k.as_str())) {
            if let Some(value) = items.get(i + 1) { tasks.extend(network_entries(value)); }
        }
    }
    tasks
}

fn check_method(items: &[SExpr], arities: &BTreeMap<String, (SymbolKind, usize)>, issues: &mut Vec<SemanticIssue>) {
    let method = items.get(1).and_then(atom).unwrap_or("<anonymous>").to_string();
    if let Some(task) = keyword_value(items, ":task") {
        match list(task).and_then(|parts| parts.first().and_then(atom).map(|name| (name, parts.len() - 1))) {
            None => issues.push(SemanticIssue { code: "HDDL_UNDEFINED_TASK", message: format!("method '{method}' :task must be a task atom '(name args...)'"), span: task.span, severity: 1 }),
            Some((name, given)) => match arities.get(&normalize(name)) {
                Some((SymbolKind::Task, declared)) if *declared != given => issues.push(SemanticIssue {
                    code: "HDDL_METHOD_ARITY_MISMATCH",
                    message: format!("method '{method}' decomposes '{name}' with {given} argument(s); task declares {declared}"),
                    span: task.span,
                    severity: 1,
                }),
                Some((SymbolKind::Task, _)) => {}
                Some(_) => issues.push(SemanticIssue { code: "HDDL_UNDEFINED_TASK", message: format!("method '{method}' :task '{name}' names a primitive action; methods decompose declared compound tasks"), span: task.span, severity: 1 }),
                None => issues.push(SemanticIssue { code: "HDDL_UNDEFINED_TASK", message: format!("method '{method}' :task '{name}' names no declared task"), span: task.span, severity: 1 }),
            },
        }
    }
    for task in network_tasks(items) { check_task_reference(task, arities, "subtask", issues); }
}

fn check_task_reference(task: &SExpr, arities: &BTreeMap<String, (SymbolKind, usize)>, role: &str, issues: &mut Vec<SemanticIssue>) {
    let Some(parts) = list(task) else { return; };
    let Some(name) = parts.first().and_then(atom) else { return; };
    let given = parts.len() - 1;
    match arities.get(&normalize(name)) {
        None => issues.push(SemanticIssue { code: "HDDL_UNDEFINED_TASK", message: format!("{role} '{name}' names no declared task or action"), span: task.span, severity: 1 }),
        Some((kind, declared)) if *declared != given => issues.push(SemanticIssue {
            code: "HDDL_SUBTASK_ARITY_MISMATCH",
            message: format!("{role} '{name}' has {given} argument(s); {} declares {declared}", if *kind == SymbolKind::Task { "task" } else { "action" }),
            span: task.span,
            severity: 1,
        }),
        Some(_) => {}
    }
}

fn check_oneof(expr: &SExpr, in_effect: bool, issues: &mut Vec<SemanticIssue>) {
    let Some(items) = list(expr) else { return; };
    if head(items).as_deref() == Some("oneof") && !in_effect {
        issues.push(SemanticIssue { code: "FOND_ONEOF_OUTSIDE_EFFECT", message: "'oneof' is a non-deterministic effect and is only admitted inside an action :effect".to_string(), span: expr.span, severity: 1 });
    }
    let mut effect_value = false;
    for item in items {
        check_oneof(item, in_effect || effect_value, issues);
        effect_value = atom(item).map(normalize).as_deref() == Some(":effect");
    }
}

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
    fn typed_list_supertype_is_a_reference_not_a_declaration() {
        let model = DocumentModel::analyze("(define (domain d) (:types location locatable - object vehicle package - locatable))");
        assert!(model.issues.is_empty(), "{:?}", model.issues);
        let types: Vec<&str> = model.symbols.iter().filter(|s| s.kind == SymbolKind::Type).map(|s| s.name.as_str()).collect();
        assert_eq!(types, vec!["location", "locatable", "vehicle", "package"]);
    }

    #[test]
    fn unknown_requirement_is_diagnostic() {
        let model = DocumentModel::analyze("(define (domain d) (:requirements :telepathy))");
        assert!(model.issues.iter().any(|issue| issue.code == "PDDL_UNKNOWN_REQUIREMENT"));
    }
}
