//! `val-pddl-check`: non-LSP subprocess oracle over VAL's PDDL/HDDL/FOND
//! semantic court. See `val_pddl_lsp::check` for the JSON and exit-code contract.

use std::io::Write;

fn main() {
    let (stdout, code) = val_pddl_lsp::check::run(std::env::args().skip(1));
    let mut out = std::io::stdout().lock();
    // A closed stdout (e.g. `| head -0`) must not turn a refusal into a panic;
    // the exit code still carries the verdict.
    let _ = writeln!(out, "{stdout}");
    let _ = out.flush();
    std::process::exit(code);
}
