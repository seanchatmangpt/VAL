//! Non-LSP subprocess oracle over the PDDL/HDDL/FOND semantic court.
//!
//! `val-pddl-check` (src/bin/val-pddl-check.rs) is a thin shell over [`run`].
//! Every input is analyzed with [`DocumentModel::analyze`]; every problem whose
//! `(:htn ...)` network cannot be judged in-document is checked with
//! [`DocumentModel::check_problem_against_domain`] against its domain. The
//! semantic issues reported are exactly the union of those library calls.
//!
//! Domain resolution for a problem with an `(:htn ...)` network:
//! 1. `--domain FILE` is used for every input when given;
//! 2. otherwise an input declaring `(domain X)` is used for every problem whose
//!    `(:domain X)` matches (ASCII case-insensitive);
//! 3. otherwise the problem is refused with `CLI_DOMAIN_UNRESOLVED`: an oracle
//!    must not admit a task network it did not judge.
//!
//! Only `CLI_*` codes originate here; they never collide with semantic codes.
//!
//! Exit-code contract:
//! - [`EXIT_ADMITTED`] (0): every input parsed and zero issues were found.
//! - [`EXIT_REFUSED`] (1): at least one issue (any severity).
//! - [`EXIT_USAGE`] (2): the check could not run (bad arguments, unreadable
//!   input). stdout still carries one JSON object with an `error` member.
//!
//! stdout always carries exactly one JSON document (the `--help` text aside).

use crate::parser::{atom, list, Span, TokenKind};
use crate::semantic::{DocumentModel, SemanticIssue};
use serde::Serialize;

pub const SCHEMA: &str = "val-pddl-check/v1";
pub const EXIT_ADMITTED: i32 = 0;
pub const EXIT_REFUSED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

pub const USAGE: &str = "usage: val-pddl-check [--domain DOMAIN_FILE] FILE...\n\
\n\
Checks PDDL, HDDL and FOND documents with VAL's semantic court and prints one\n\
JSON report on stdout. FILE may be '-' to read stdin. A problem's (:htn ...)\n\
network is checked against --domain, else against an input declaring the\n\
problem's (:domain ...), else it is refused with CLI_DOMAIN_UNRESOLVED.\n\
Exit 0 = admitted, 1 = refused (issues found), 2 = usage or I/O error.";

/// Which court produced an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckKind {
    /// [`DocumentModel::analyze`] on one document.
    Document,
    /// [`DocumentModel::check_problem_against_domain`] against the resolved domain.
    ProblemAgainstDomain,
    /// This module: no domain could be resolved for a problem's task network.
    DomainResolution,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportedIssue {
    pub file: String,
    pub check: CheckKind,
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub severity: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSummary {
    pub path: String,
    pub role: &'static str,
    pub domain_name: Option<String>,
    pub problem_name: Option<String>,
    /// Path of the domain this input's task network was checked against.
    pub checked_against: Option<String>,
    pub issue_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub admitted: bool,
    pub files: Vec<FileSummary>,
    pub issues: Vec<ReportedIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CliError>,
}

impl Report {
    pub fn exit_code(&self) -> i32 {
        match (&self.error, self.admitted) {
            (Some(_), _) => EXIT_USAGE,
            (None, true) => EXIT_ADMITTED,
            (None, false) => EXIT_REFUSED,
        }
    }

    pub fn to_json(&self) -> String {
        // Serializing plain owned data does not fail; the fallback keeps stdout
        // JSON even if that invariant ever breaks.
        serde_json::to_string_pretty(self).unwrap_or_else(|_| {
            format!("{{\"schema\":\"{SCHEMA}\",\"admitted\":false,\"files\":[],\"issues\":[],\"error\":{{\"code\":\"CLI_SERIALIZE\",\"message\":\"report serialization failed\"}}}}")
        })
    }

    fn failure(code: &'static str, message: String) -> Self {
        Self { schema: SCHEMA, admitted: false, files: Vec::new(), issues: Vec::new(), error: Some(CliError { code, message }) }
    }
}

/// Parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Help,
    Check { domain: Option<String>, files: Vec<String> },
}

/// Parses `argv[1..]`. Errors become `CLI_USAGE` reports.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Invocation, String> {
    let mut domain: Option<String> = None;
    let mut files = Vec::new();
    let mut args = args.into_iter();
    let mut positional_only = false;
    let set_domain = |value: String, domain: &mut Option<String>| {
        if domain.replace(value).is_some() { Err("--domain given more than once".to_string()) } else { Ok(()) }
    };
    while let Some(arg) = args.next() {
        if positional_only {
            files.push(arg);
            continue;
        }
        match arg.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "--" => positional_only = true,
            "--domain" | "-d" => {
                let value = args.next().ok_or_else(|| format!("{arg} requires a file argument"))?;
                set_domain(value, &mut domain)?;
            }
            other if other.starts_with("--domain=") => set_domain(other["--domain=".len()..].to_string(), &mut domain)?,
            other if other.starts_with('-') && other != "-" => return Err(format!("unknown option '{other}'")),
            _ => files.push(arg),
        }
    }
    if files.is_empty() && domain.is_none() {
        return Err("no input files".to_string());
    }
    let stdin_uses = files.iter().chain(domain.iter()).filter(|f| *f == "-").count();
    if stdin_uses > 1 {
        return Err("stdin ('-') may be used for at most one input".to_string());
    }
    Ok(Invocation::Check { domain, files })
}

/// Reads a real file, or stdin for `-`.
pub fn read_input(path: &str) -> std::io::Result<String> {
    if path == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
        Ok(text)
    } else {
        std::fs::read_to_string(path)
    }
}

/// The `X` of a problem's `(:domain X)`, if any.
pub fn problem_domain_reference(model: &DocumentModel) -> Option<String> {
    model.parse.roots.iter().filter_map(list).flat_map(|items| items.iter()).find_map(|item| {
        let parts = list(item)?;
        match parts {
            [head, name, ..] if atom(head).is_some_and(|h| h.eq_ignore_ascii_case(":domain")) => atom(name).map(str::to_string),
            _ => None,
        }
    })
}

/// True when the document carries an `(:htn ...)` task network.
pub fn has_task_network(model: &DocumentModel) -> bool {
    model.parse.tokens.iter().any(|t| t.kind == TokenKind::Atom && t.text.eq_ignore_ascii_case(":htn"))
}

fn reported(file: &str, check: CheckKind, issues: Vec<SemanticIssue>) -> Vec<ReportedIssue> {
    issues
        .into_iter()
        .map(|SemanticIssue { code, message, span, severity }| ReportedIssue { file: file.to_string(), check, code, message, span, severity })
        .collect()
}

fn zero_span() -> Span { Span { start: 0, end: 0, start_line: 0, start_character: 0, end_line: 0, end_character: 0 } }

/// Runs the court over already-read inputs `(path, text)`. Never panics on bad input.
pub fn check_texts(domain: Option<(&str, String)>, files: Vec<(&str, String)>) -> Report {
    let explicit = domain.map(|(path, text)| (path, DocumentModel::analyze(text)));
    let inputs: Vec<(&str, DocumentModel)> = files.into_iter().map(|(path, text)| (path, DocumentModel::analyze(text))).collect();
    let mut report = Report { schema: SCHEMA, admitted: true, files: Vec::new(), issues: Vec::new(), error: None };
    let summary = |path: &str, role: &'static str, model: &DocumentModel, checked_against: Option<String>, issue_count: usize| FileSummary {
        path: path.to_string(),
        role,
        domain_name: model.domain_name.clone(),
        problem_name: model.problem_name.clone(),
        checked_against,
        issue_count,
    };

    if let Some((path, model)) = &explicit {
        let issues = reported(path, CheckKind::Document, model.issues.clone());
        report.files.push(summary(path, "domain", model, None, issues.len()));
        report.issues.extend(issues);
    }
    for (path, model) in &inputs {
        let mut issues = reported(path, CheckKind::Document, model.issues.clone());
        let mut checked_against = None;
        // A document that declares its own domain has its :htn judged in-document.
        let needs_domain = model.domain_name.is_none() && has_task_network(model);
        let resolved: Option<(&str, &DocumentModel)> = match &explicit {
            Some((dpath, dmodel)) => Some((dpath, dmodel)),
            None if needs_domain => problem_domain_reference(model).and_then(|wanted| {
                inputs.iter().find(|(_, other)| other.domain_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&wanted))).map(|(p, m)| (*p, m))
            }),
            None => None,
        };
        match resolved {
            Some((dpath, dmodel)) => {
                issues.extend(reported(path, CheckKind::ProblemAgainstDomain, model.check_problem_against_domain(dmodel)));
                checked_against = Some(dpath.to_string());
            }
            None if needs_domain => issues.push(ReportedIssue {
                file: path.to_string(),
                check: CheckKind::DomainResolution,
                code: "CLI_DOMAIN_UNRESOLVED",
                message: match problem_domain_reference(model) {
                    Some(name) => format!("(:htn ...) network not judged: no --domain and no input declares domain '{name}'"),
                    None => "(:htn ...) network not judged: no --domain and the problem names no (:domain ...)".to_string(),
                },
                span: zero_span(),
                severity: 1,
            }),
            None => {}
        }
        let role = if model.domain_name.is_some() { "domain" } else { "document" };
        report.files.push(summary(path, role, model, checked_against, issues.len()));
        report.issues.extend(issues);
    }
    report.admitted = report.issues.is_empty();
    report
}

/// Reads inputs from disk/stdin and runs [`check_texts`].
pub fn check(domain: Option<&str>, files: &[String]) -> Report {
    let domain = match domain {
        None => None,
        Some(path) => match read_input(path) {
            Ok(text) => Some((path, text)),
            Err(e) => return Report::failure("CLI_IO_ERROR", format!("cannot read domain '{path}': {e}")),
        },
    };
    let mut texts = Vec::with_capacity(files.len());
    for path in files {
        match read_input(path) {
            Ok(text) => texts.push((path.as_str(), text)),
            Err(e) => return Report::failure("CLI_IO_ERROR", format!("cannot read '{path}': {e}")),
        }
    }
    check_texts(domain, texts)
}

/// Full CLI entry: argv without the program name -> (stdout text, exit code).
pub fn run<I: IntoIterator<Item = String>>(args: I) -> (String, i32) {
    match parse_args(args) {
        Ok(Invocation::Help) => (USAGE.to_string(), EXIT_ADMITTED),
        Err(message) => {
            let report = Report::failure("CLI_USAGE", format!("{message}\n{USAGE}"));
            (report.to_json(), report.exit_code())
        }
        Ok(Invocation::Check { domain, files }) => {
            let report = check(domain.as_deref(), &files);
            (report.to_json(), report.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn parse_args_accepts_domain_forms_and_positionals() {
        assert_eq!(parse_args(args(&["--domain", "d.hddl", "p.hddl"])), Ok(Invocation::Check { domain: Some("d.hddl".into()), files: args(&["p.hddl"]) }));
        assert_eq!(parse_args(args(&["--domain=d.hddl"])), Ok(Invocation::Check { domain: Some("d.hddl".into()), files: vec![] }));
        assert_eq!(parse_args(args(&["--", "--weird.pddl"])), Ok(Invocation::Check { domain: None, files: args(&["--weird.pddl"]) }));
        assert_eq!(parse_args(args(&["-"])), Ok(Invocation::Check { domain: None, files: args(&["-"]) }));
        assert_eq!(parse_args(args(&["p", "--help"])), Ok(Invocation::Help));
    }

    #[test]
    fn parse_args_refuses_bad_invocations() {
        for bad in [&[][..], &["--domain"], &["--domain", "a", "--domain", "b", "p"], &["--domain=a", "-d", "b"], &["--frobnicate", "p"], &["-", "-"], &["--domain", "-", "-"]] {
            assert!(parse_args(args(bad)).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn problem_domain_reference_reads_the_domain_clause() {
        let model = DocumentModel::analyze("(define (problem p) (:domain Transport) (:htn :ordered-subtasks (and)))");
        assert_eq!(problem_domain_reference(&model).as_deref(), Some("Transport"));
        assert!(has_task_network(&model));
        let plain = DocumentModel::analyze("(define (problem p) (:domain d) (:init) (:goal (and)))");
        assert!(!has_task_network(&plain));
    }
}
