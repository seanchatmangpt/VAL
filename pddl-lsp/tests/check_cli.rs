//! `val-pddl-check` as a real subprocess oracle (Chicago style: the built
//! binary is spawned on real fixture files; assertions are on its exit code
//! and the JSON it prints, cross-checked against the library court).

use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use val_pddl_lsp::semantic::DocumentModel;

const BIN: &str = env!("CARGO_BIN_EXE_val-pddl-check");

fn fixture_path(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name).to_string_lossy().into_owned()
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

struct Run {
    code: i32,
    json: Value,
}

fn run_with_stdin(args: &[&str], stdin: Option<&str>) -> Run {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn val-pddl-check");
    if let Some(text) = stdin {
        child.stdin.take().expect("stdin").write_all(text.as_bytes()).expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait val-pddl-check");
    let stdout = String::from_utf8(out.stdout).expect("utf-8 stdout");
    let json: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}) for {args:?}:\n{stdout}"));
    Run { code: out.status.code().expect("exit code"), json }
}

fn run(args: &[&str]) -> Run { run_with_stdin(args, None) }

fn codes(run: &Run) -> Vec<String> {
    run.json["issues"].as_array().expect("issues array").iter().map(|i| i["code"].as_str().expect("code").to_string()).collect()
}

fn assert_report_shape(run: &Run) {
    assert_eq!(run.json["schema"], "val-pddl-check/v1");
    let admitted = run.json["admitted"].as_bool().expect("admitted bool");
    assert_eq!(admitted, run.code == 0, "admitted must agree with exit code {}: {}", run.code, run.json);
    for issue in run.json["issues"].as_array().expect("issues") {
        for key in ["file", "check", "code", "message", "severity"] {
            assert!(!issue[key].is_null(), "issue lacks {key}: {issue}");
        }
        for key in ["start", "end", "start_line", "start_character", "end_line", "end_character"] {
            assert!(issue["span"][key].is_u64(), "span lacks {key}: {issue}");
        }
    }
}

#[test]
fn ipc_transport_pair_is_admitted_with_explicit_domain() {
    let r = run(&["--domain", &fixture_path("ipc-transport-domain.hddl"), &fixture_path("ipc-transport-problem.hddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 0, "{}", r.json);
    assert!(codes(&r).is_empty());
    assert_eq!(r.json["files"][1]["checked_against"], fixture_path("ipc-transport-domain.hddl"));
    assert_eq!(r.json["files"][1]["problem_name"], "pfile01");
}

#[test]
fn ipc_transport_pair_is_admitted_by_domain_name_resolution() {
    let r = run(&[&fixture_path("ipc-transport-problem.hddl"), &fixture_path("ipc-transport-domain.hddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 0, "{}", r.json);
    assert_eq!(r.json["files"][0]["checked_against"], fixture_path("ipc-transport-domain.hddl"));
}

#[test]
fn problem_with_undeclared_task_is_refused_against_its_domain() {
    for args in [
        vec!["--domain".to_string(), fixture_path("ipc-transport-domain.hddl"), fixture_path("ipc-transport-problem-undeclared-task.hddl")],
        vec![fixture_path("ipc-transport-domain.hddl"), fixture_path("ipc-transport-problem-undeclared-task.hddl")],
    ] {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = run(&args);
        assert_report_shape(&r);
        assert_eq!(r.code, 1, "{}", r.json);
        assert_eq!(codes(&r), vec!["HDDL_UNDEFINED_TASK"]);
        let issue = &r.json["issues"][0];
        assert_eq!(issue["check"], "problem-against-domain");
        assert_eq!(issue["file"], fixture_path("ipc-transport-problem-undeclared-task.hddl"));
        assert!(issue["message"].as_str().unwrap().contains("ship"), "{issue}");
        assert_eq!(issue["span"]["start_line"], 11, "span must point at the (ship ...) atom line: {issue}");
    }
}

#[test]
fn problem_with_task_network_and_no_domain_is_refused_not_admitted() {
    let r = run(&[&fixture_path("ipc-transport-problem-undeclared-task.hddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1, "{}", r.json);
    assert_eq!(codes(&r), vec!["CLI_DOMAIN_UNRESOLVED"]);
    assert_eq!(r.json["issues"][0]["check"], "domain-resolution");
    assert!(r.json["issues"][0]["message"].as_str().unwrap().contains("transport"));
}

#[test]
fn problem_task_arity_mismatch_is_refused() {
    let r = run(&["--domain", &fixture_path("ipc-transport-domain.hddl"), &fixture_path("ipc-transport-problem-arity-mismatch.hddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    assert_eq!(codes(&r), vec!["HDDL_SUBTASK_ARITY_MISMATCH"]);
}

#[test]
fn domain_issues_refuse_even_when_problem_is_clean() {
    let r = run(&["--domain", &fixture_path("semantic-jira-shop-defproblem.hddl"), &fixture_path("fond-tireworld-domain.pddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    assert_eq!(r.json["files"][0]["role"], "domain");
    assert!(codes(&r).iter().any(|c| c == "PDDL_DEFINE_REQUIRED"), "{}", r.json);
}

#[test]
fn fond_domain_admitted_and_oneof_in_precondition_refused() {
    let ok = run(&[&fixture_path("fond-tireworld-domain.pddl")]);
    assert_report_shape(&ok);
    assert_eq!(ok.code, 0, "{}", ok.json);
    let bad = run(&[&fixture_path("fond-tireworld-oneof-precondition.pddl")]);
    assert_report_shape(&bad);
    assert_eq!(bad.code, 1);
    assert_eq!(codes(&bad), vec!["FOND_ONEOF_OUTSIDE_EFFECT"]);
}

#[test]
fn shop_defproblem_is_refused_with_library_codes() {
    let r = run(&[&fixture_path("semantic-jira-shop-defproblem.hddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    let c = codes(&r);
    assert_eq!(c.iter().filter(|c| *c == "PDDL_DEFINE_REQUIRED").count(), 1);
    assert_eq!(c.iter().filter(|c| *c == "HDDL_UNDEFINED_TASK").count(), 5);
}

#[test]
fn unbalanced_document_is_refused_as_json() {
    let r = run(&[&fixture_path("unbalanced-domain.pddl")]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    assert!(!codes(&r).is_empty());
}

#[test]
fn stdin_input_is_checked() {
    let r = run_with_stdin(&["--domain", &fixture_path("ipc-transport-domain.hddl"), "-"], Some(&fixture_text("ipc-transport-problem-undeclared-task.hddl")));
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    assert_eq!(codes(&r), vec!["HDDL_UNDEFINED_TASK"]);
    assert_eq!(r.json["issues"][0]["file"], "-");
}

#[test]
fn empty_stdin_is_refused() {
    let r = run_with_stdin(&["-"], Some(""));
    assert_report_shape(&r);
    assert_eq!(r.code, 1);
    assert_eq!(codes(&r), vec!["PDDL_EMPTY_DOCUMENT"]);
}

#[test]
fn usage_and_io_errors_exit_2_with_json_error() {
    let missing = fixture_path("does-not-exist.hddl");
    for (args, code) in [
        (vec![], "CLI_USAGE"),
        (vec!["--bogus", "x.pddl"], "CLI_USAGE"),
        (vec!["--domain"], "CLI_USAGE"),
        (vec![missing.as_str()], "CLI_IO_ERROR"),
        (vec!["--domain", missing.as_str(), "x.pddl"], "CLI_IO_ERROR"),
    ] {
        let r = run(&args);
        assert_eq!(r.code, 2, "{args:?}: {}", r.json);
        assert_eq!(r.json["admitted"], false);
        assert_eq!(r.json["error"]["code"], code, "{args:?}: {}", r.json);
    }
}

#[test]
fn help_exits_zero_with_usage_text() {
    let out = Command::new(BIN).arg("--help").output().expect("spawn");
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("usage: val-pddl-check"));
}

/// Falsifier: the CLI's semantic issue codes must equal the library court's on
/// every fixture, alone and against every domain fixture.
#[test]
fn cli_issue_codes_equal_library_codes_on_every_fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("fixtures dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".hddl") || n.ends_with(".pddl"))
        .collect();
    names.sort();
    assert!(names.len() >= 8, "fixture corpus shrank: {names:?}");
    let domains: Vec<&String> = names.iter().filter(|n| DocumentModel::analyze(fixture_text(n)).domain_name.is_some()).collect();
    let mut compared = 0;
    for name in &names {
        let model = DocumentModel::analyze(fixture_text(name));
        // Alone: document codes, plus CLI_DOMAIN_UNRESOLVED only when a task network could not be judged.
        let r = run(&[&fixture_path(name)]);
        assert_report_shape(&r);
        let mut expected: Vec<String> = model.issues.iter().map(|i| i.code.to_string()).collect();
        if model.domain_name.is_none() && val_pddl_lsp::check::has_task_network(&model) {
            expected.push("CLI_DOMAIN_UNRESOLVED".to_string());
        }
        assert_eq!(codes(&r), expected, "alone: {name}");
        compared += 1;
        for domain in &domains {
            let dmodel = DocumentModel::analyze(fixture_text(domain));
            let r = run(&["--domain", &fixture_path(domain), &fixture_path(name)]);
            assert_report_shape(&r);
            let mut expected: Vec<String> = dmodel.issues.iter().map(|i| i.code.to_string()).collect();
            expected.extend(model.issues.iter().map(|i| i.code.to_string()));
            expected.extend(model.check_problem_against_domain(&dmodel).iter().map(|i| i.code.to_string()));
            assert_eq!(codes(&r), expected, "{name} against {domain}");
            assert_eq!(r.code == 0, expected.is_empty(), "{name} against {domain}");
            compared += 1;
        }
    }
    let semantic: BTreeSet<String> = names.iter().flat_map(|n| DocumentModel::analyze(fixture_text(n)).issues.into_iter().map(|i| i.code.to_string())).collect();
    assert!(semantic.iter().all(|c| !c.starts_with("CLI_")), "library emitted a CLI_ code: {semantic:?}");
    assert!(compared >= names.len() * 2, "compared only {compared} runs");
}

/// A real file on disk for inputs derived from fixtures at test time.
struct TempInput(PathBuf);

impl TempInput {
    fn new(name: &str, text: &str) -> Self {
        let path = std::env::temp_dir().join(format!("val-pddl-check-{}-{name}", std::process::id()));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        Self(path)
    }

    fn path(&self) -> &str { self.0.to_str().expect("utf-8 temp path") }
}

impl Drop for TempInput {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

fn nested(depth: usize) -> String { format!("{}{}", "(".repeat(depth), ")".repeat(depth)) }

/// Falsifier for "stdout always carries exactly one JSON document": inputs
/// nested far past any real document used to overflow the recursive parser
/// (exit 134, empty stdout). They must be refused as JSON with exit 1.
#[test]
fn pathologically_deep_input_is_refused_as_json_not_a_crash() {
    for depth in [5_000usize, 50_000, 200_000] {
        let r = run_with_stdin(&["-"], Some(&nested(depth)));
        assert_report_shape(&r);
        assert_eq!(r.code, 1, "depth {depth}: {}", r.json);
        assert!(codes(&r).iter().any(|c| c == "PDDL_NESTING_TOO_DEEP"), "depth {depth}: {:?}", codes(&r));
    }
    let unbalanced = run_with_stdin(&["-"], Some(&"(".repeat(200_000)));
    assert_report_shape(&unbalanced);
    assert_eq!(unbalanced.code, 1);
    let c = codes(&unbalanced);
    assert!(c.iter().any(|c| c == "PDDL_NESTING_TOO_DEEP") && c.iter().any(|c| c == "PDDL_UNCLOSED_LIST"), "{c:?}");
}

#[test]
fn deep_precondition_in_a_real_domain_is_refused_as_json() {
    let deep_and = format!("{}(at ?p ?l){}", "(and ".repeat(6_000), ")".repeat(6_000));
    let domain = fixture_text("ipc-transport-domain.hddl").replacen(
        "(:task deliver :parameters (?p - package ?l - location))",
        &format!("(:task deliver :parameters (?p - package ?l - location))\n  (:action probe :parameters (?p - package ?l - location) :precondition {deep_and} :effect (and))"),
        1,
    );
    assert_ne!(domain, fixture_text("ipc-transport-domain.hddl"), "splice point moved");
    let r = run_with_stdin(&["-"], Some(&domain));
    assert_report_shape(&r);
    assert_eq!(r.code, 1, "{}", r.json);
    assert_eq!(codes(&r), vec!["PDDL_NESTING_TOO_DEEP"]);
    assert_eq!(r.json["files"][0]["domain_name"], "transport", "the rest of the domain is still analyzed");
}

#[test]
fn deep_task_network_against_explicit_domain_is_refused_as_json() {
    let problem = fixture_text("ipc-transport-problem.hddl").replacen(":ordered-subtasks (and", &format!(":ordered-subtasks (and {}", nested(100_000)), 1);
    assert_ne!(problem, fixture_text("ipc-transport-problem.hddl"), "splice point moved");
    let r = run_with_stdin(&["--domain", &fixture_path("ipc-transport-domain.hddl"), "-"], Some(&problem));
    assert_report_shape(&r);
    assert_eq!(r.code, 1, "{}", r.json);
    assert!(codes(&r).iter().any(|c| c == "PDDL_NESTING_TOO_DEEP"), "{:?}", codes(&r));
    assert_eq!(r.json["files"][1]["checked_against"], fixture_path("ipc-transport-domain.hddl"));
}

/// Kills the case-sensitive-resolution mutant: `(:domain TRANSPORT)` must
/// resolve to the input declaring `(domain transport)` (ASCII case-insensitive).
#[test]
fn domain_name_resolution_is_ascii_case_insensitive() {
    let upper = TempInput::new("upper-ok.hddl", &fixture_text("ipc-transport-problem.hddl").replacen("(:domain transport)", "(:domain TRANSPORT)", 1));
    let bad = TempInput::new("upper-bad.hddl", &fixture_text("ipc-transport-problem-undeclared-task.hddl").replacen("(:domain transport)", "(:domain TRANSPORT)", 1));
    let domain = fixture_path("ipc-transport-domain.hddl");
    let ok = run(&[upper.path(), &domain]);
    assert_report_shape(&ok);
    assert_eq!(ok.code, 0, "{}", ok.json);
    assert_eq!(ok.json["files"][0]["checked_against"], domain);
    for args in [[bad.path(), domain.as_str()], [domain.as_str(), bad.path()]] {
        let r = run(&args);
        assert_report_shape(&r);
        assert_eq!(r.code, 1, "{}", r.json);
        assert_eq!(codes(&r), vec!["HDDL_UNDEFINED_TASK"], "{args:?}");
    }
}

/// Pins the documented rule "--domain is used for every input": a document
/// that declares its own domain and carries an (:htn ...) network is judged
/// in-document AND against the explicit domain. Kills the mutant that applies
/// --domain only to inputs lacking a domain.
#[test]
fn explicit_domain_also_judges_a_self_contained_htn_document() {
    let combined = format!(
        "{}\n(define (problem self-contained) (:domain transport) (:htn :ordered-subtasks (and (t0 (deliver p l))))) ",
        fixture_text("ipc-transport-domain.hddl")
    );
    let own = TempInput::new("self-contained.hddl", &combined);
    let alone = run(&[own.path()]);
    assert_report_shape(&alone);
    assert_eq!(alone.code, 0, "in-document judgement admits it: {}", alone.json);
    assert!(alone.json["files"][0]["checked_against"].is_null());
    let r = run(&["--domain", &fixture_path("fond-tireworld-domain.pddl"), own.path()]);
    assert_report_shape(&r);
    assert_eq!(r.code, 1, "{}", r.json);
    assert_eq!(r.json["files"][1]["checked_against"], fixture_path("fond-tireworld-domain.pddl"));
    let against: Vec<&Value> = r.json["issues"].as_array().unwrap().iter().filter(|i| i["check"] == "problem-against-domain").collect();
    assert_eq!(against.len(), 1, "{}", r.json);
    assert_eq!(against[0]["code"], "HDDL_UNDEFINED_TASK");
}
