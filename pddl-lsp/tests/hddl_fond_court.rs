//! HDDL + FOND semantic court over real fixture files (Chicago style: real
//! parser, real analyzer, real files on disk; assertions on returned issues).

use std::path::PathBuf;
use val_pddl_lsp::semantic::{DocumentModel, SemanticIssue, SymbolKind};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn codes(issues: &[SemanticIssue]) -> Vec<&'static str> {
    issues.iter().map(|i| i.code).collect()
}

fn count(issues: &[SemanticIssue], code: &str) -> usize {
    issues.iter().filter(|i| i.code == code).count()
}

#[test]
fn ipc_hddl_domain_is_admitted_with_zero_issues() {
    let model = DocumentModel::analyze(fixture("ipc-transport-domain.hddl"));
    assert!(model.issues.is_empty(), "unexpected issues: {:?}", model.issues);
}

#[test]
fn ipc_hddl_problem_is_admitted_alone_and_against_its_domain() {
    let domain = DocumentModel::analyze(fixture("ipc-transport-domain.hddl"));
    let problem = DocumentModel::analyze(fixture("ipc-transport-problem.hddl"));
    assert!(problem.issues.is_empty(), "unexpected issues: {:?}", problem.issues);
    let cross = problem.check_problem_against_domain(&domain);
    assert!(cross.is_empty(), "unexpected cross issues: {cross:?}");
    assert_eq!(problem.problem_name.as_deref(), Some("pfile01"));
}

#[test]
fn tasks_are_indexed_as_task_symbols_with_arity() {
    let model = DocumentModel::analyze(fixture("ipc-transport-domain.hddl"));
    let tasks: Vec<&str> = model.symbols.iter().filter(|s| s.kind == SymbolKind::Task).map(|s| s.name.as_str()).collect();
    assert_eq!(tasks, vec!["deliver", "get-to", "load", "unload"]);
    let deliver = model.definition("deliver").expect("deliver indexed");
    assert_eq!(deliver.kind, SymbolKind::Task);
    let arities = model.operator_arities();
    assert_eq!(arities.get("deliver"), Some(&(SymbolKind::Task, 2)));
    assert_eq!(arities.get("drop"), Some(&(SymbolKind::Action, 3)));
}

#[test]
fn method_task_naming_undeclared_task_is_refused() {
    let text = fixture("ipc-transport-domain.hddl").replace(":task (deliver ?p ?l2)", ":task (teleport ?p ?l2)");
    let model = DocumentModel::analyze(text);
    assert_eq!(count(&model.issues, "HDDL_UNDEFINED_TASK"), 1, "{:?}", model.issues);
    let issue = model.issues.iter().find(|i| i.code == "HDDL_UNDEFINED_TASK").unwrap();
    assert!(issue.message.contains("teleport"), "{}", issue.message);
}

#[test]
fn method_task_naming_primitive_action_is_refused() {
    let text = fixture("ipc-transport-domain.hddl").replace(":task (unload ?v ?l ?p)", ":task (drop ?v ?l ?p)");
    let model = DocumentModel::analyze(text);
    assert_eq!(codes(&model.issues), vec!["HDDL_UNDEFINED_TASK"]);
}

#[test]
fn subtask_naming_undeclared_operator_is_refused() {
    let text = fixture("ipc-transport-domain.hddl").replace("(t2 (load ?v ?l1 ?p))", "(t2 (lift ?v ?l1 ?p))");
    let model = DocumentModel::analyze(text);
    assert_eq!(codes(&model.issues), vec!["HDDL_UNDEFINED_TASK"]);
}

#[test]
fn method_arity_mismatch_is_refused() {
    let text = fixture("ipc-transport-domain.hddl").replace(":task (deliver ?p ?l2)", ":task (deliver ?p)");
    let model = DocumentModel::analyze(text);
    assert_eq!(codes(&model.issues), vec!["HDDL_METHOD_ARITY_MISMATCH"]);
}

#[test]
fn subtask_arity_mismatch_is_refused() {
    let text = fixture("ipc-transport-domain.hddl").replace(":subtasks (drive ?v ?l1 ?l2)", ":subtasks (drive ?v ?l2)");
    let model = DocumentModel::analyze(text);
    assert_eq!(codes(&model.issues), vec!["HDDL_SUBTASK_ARITY_MISMATCH"]);
}

#[test]
fn problem_htn_checked_against_separate_domain() {
    let domain = DocumentModel::analyze(fixture("ipc-transport-domain.hddl"));
    let problem = DocumentModel::analyze(fixture("ipc-transport-problem.hddl").replace("(task1 (deliver package-1 city-loc-2))", "(task1 (ship package-1 city-loc-2))"));
    // The problem alone does not declare a domain, so it cannot judge references.
    assert!(problem.issues.is_empty(), "{:?}", problem.issues);
    assert_eq!(codes(&problem.check_problem_against_domain(&domain)), vec!["HDDL_UNDEFINED_TASK"]);
}

#[test]
fn fond_oneof_domain_is_admitted_with_zero_issues() {
    let model = DocumentModel::analyze(fixture("fond-tireworld-domain.pddl"));
    assert!(model.issues.is_empty(), "unexpected issues: {:?}", model.issues);
    assert!(model.symbols.iter().any(|s| s.kind == SymbolKind::Requirement && s.name == ":non-deterministic"));
}

#[test]
fn oneof_in_precondition_is_refused() {
    let text = fixture("fond-tireworld-domain.pddl").replace(
        ":precondition (and (spare-in ?loc) (vehicle-at ?loc))",
        ":precondition (oneof (spare-in ?loc) (vehicle-at ?loc))",
    );
    let model = DocumentModel::analyze(text);
    assert_eq!(codes(&model.issues), vec!["FOND_ONEOF_OUTSIDE_EFFECT"]);
}

#[test]
fn oneof_in_problem_goal_is_refused() {
    let model = DocumentModel::analyze("(define (problem p) (:domain tireworld) (:objects a b - location) (:init (vehicle-at a)) (:goal (oneof (vehicle-at a) (vehicle-at b))))");
    assert_eq!(codes(&model.issues), vec!["FOND_ONEOF_OUTSIDE_EFFECT"]);
}

#[test]
fn nested_oneof_under_conditional_effect_is_admitted() {
    let model = DocumentModel::analyze("(define (domain d) (:requirements :non-deterministic :conditional-effects) (:predicates (p) (q)) (:action a :parameters () :precondition (p) :effect (when (p) (oneof (q) (not (q))))))");
    assert!(model.issues.is_empty(), "{:?}", model.issues);
}

#[test]
fn semantic_jira_shop_defproblem_is_refused_as_non_hddl() {
    let model = DocumentModel::analyze(fixture("semantic-jira-shop-defproblem.hddl"));
    assert!(model.parse.issues.is_empty(), "fixture must be balanced: {:?}", model.parse.issues);
    assert_eq!(count(&model.issues, "PDDL_DEFINE_REQUIRED"), 1, "{:?}", model.issues);
    // The SHOP form declares no HDDL tasks, so the method's :task and all four
    // subtasks are unresolved references.
    assert_eq!(count(&model.issues, "HDDL_UNDEFINED_TASK"), 5, "{:?}", model.issues);
    assert!(model.symbols.iter().all(|s| s.kind != SymbolKind::Task));
}

#[test]
fn hddl_and_fond_vocabulary_is_completable() {
    let words = DocumentModel::analyze("(define (domain d))").completion_words();
    for keyword in [":htn", ":subtasks", ":ordered-subtasks", ":ordering", ":tasks", "oneof", ":non-deterministic"] {
        assert!(words.iter().any(|w| w == keyword), "missing completion {keyword}");
    }
}
