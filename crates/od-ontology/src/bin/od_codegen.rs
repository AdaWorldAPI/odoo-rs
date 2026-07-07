//! `od-codegen` — read an SPO-triple ndjson corpus and print either the
//! table → canonical OGAR classid map (`--classids`) or the behavioral-arm
//! lowering (`--actions`).
//!
//! # Usage
//!
//! ```sh
//! # Slice 1: account.move alone
//! od-codegen data/account_move.spo.ndjson --focus account_move --classids
//!
//! # Slice 2: 4 models, behavioral-arm lowering
//! od-codegen data/slice_2.spo.ndjson \
//!   --focus account_move,account_move_line,res_partner,res_company \
//!   --actions
//!
//! # Stdin pipeline
//! cat data/slice_2.spo.ndjson | od-codegen - -f account_move --classids
//! ```
//!
//! # `SurrealQL` is deprecated
//!
//! **`SurrealQL` is deprecated** (operator ruling 2026-07-06: *"`SurrealQL` is
//! absolutely deprecated. OGAR V3 for transpile substrate, lance-graph V3 for
//! database."*). This binary used to default to rendering a `.surql` schema
//! (`corpus_to_schema` → `Schema::to_sql`); that DDL-emit path — and the
//! `-r/--relations`, `-o/--output`, `--validate`, `--stats` schema-diagnostics
//! flags that only served it — has been deleted along with `surreal_ast.rs` /
//! `emit.rs`. The two surviving modes (`--classids` / `--actions`) never
//! depended on the bespoke DDL AST; they read straight off the SPO triples
//! and the OGAR codebook. See `docs/W3.3-DELETE-GATE-MATRIX.md`.
//!
//! # Exit codes — `lance-graph#512` convention
//!
//! Following `lance-graph#512`'s degenerate-input-vs-generic-error split so
//! wrapper scripts can react to the cause:
//!
//! - `0` — output printed successfully.
//! - `1` — argument / I/O error (message on stderr).
//! - `2` — degenerate input (empty / malformed ndjson, zero triples, zero
//!   tables — the upstream extractor never produced anything meaningful).
//!
//! Pattern parity with `AdaWorldAPI/openproject-nexgen-rs#31` so the two
//! CLIs behave identically across language fronts.

#![cfg(feature = "cli")]

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

use od_ontology::{model_of, parse_ndjson, render_classid, Triple};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let parsed = match parse_args(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            process::exit(1);
        }
    };
    if parsed.help {
        println!("{USAGE}");
        return;
    }

    // ── 1. Load triples ──
    let ndjson = match read_input(&parsed.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading input: {e}");
            process::exit(1);
        }
    };
    let triples = match parse_ndjson(&ndjson) {
        Ok(t) => t,
        Err(e) => {
            // Malformed ndjson — degenerate input, exit 2 per #512.
            eprintln!("error parsing triple ndjson: {e}");
            process::exit(2);
        }
    };

    // Degenerate-input guard (#512): an empty triple stream carries nothing
    // to render either output mode from. Exit 2 with a directive message
    // naming the upstream fix.
    if triples.is_empty() {
        eprintln!(
            "error: input contains zero triples — run the upstream extractor first \
             (e.g. `python3 -m odoo_blueprint_extractor --addons /home/user/odoo/addons \
             --addon account` then emit_ontology over the parquet)"
        );
        process::exit(2);
    }

    let focus_owned: Vec<String> = parsed.focus.clone();
    let focus_refs: Vec<&str> = focus_owned.iter().map(String::as_str).collect();

    // ── 2. `--actions` — print the behavioral-arm lowering (ActionDef) ──
    if parsed.actions {
        // Focus-miss guard (codex P2 on #25): validate the focus set against
        // the table universe BEFORE the per-row filter, exactly like the
        // `--classids` path below — otherwise a mistyped `--focus` exits 0
        // with empty output instead of the documented exit-2 contract that
        // `tests/exit_codes.rs` locks for the default mode.
        let focused_tables = object_type_tables(&triples, &focus_refs);
        if focused_tables.is_empty() {
            eprintln!(
                "error: projection emitted zero tables — focus set {:?} did not match any \
                 `(*, rdf:type, ogit:ObjectType)` row in the {} input triples",
                focus_owned,
                triples.len(),
            );
            process::exit(2);
        }

        // `corpus_action_rows` is deprecated (the DO-arm now lives on OGAR's
        // `CompiledClass.actions` via `compile_source`) but stays the
        // kausal-parity witness for the corpus-side ndjson pipeline this CLI
        // reads — see `ogar_actions.rs`'s module docs.
        #[allow(deprecated)]
        let rows = od_ontology::corpus_action_rows(&triples);
        for (model, predicate, kind, detail) in rows {
            // Respect --focus: only the focused models, when given.
            if !focus_refs.is_empty() && !focus_refs.iter().any(|&f| f == model) {
                continue;
            }
            println!("{model}.{predicate}\t{kind}\t{detail}");
        }
        return;
    }

    // ── 3. `--classids` (default) — print table → OGAR render classid map ──
    let tables = object_type_tables(&triples, &focus_refs);

    // Degenerate-output guard (#512): zero tables means the focus set didn't
    // intersect any `(*, rdf:type, ogit:ObjectType)` rows.
    if tables.is_empty() {
        eprintln!(
            "error: projection emitted zero tables — focus set {:?} did not match any \
             `(*, rdf:type, ogit:ObjectType)` row in the {} input triples",
            focus_owned,
            triples.len(),
        );
        process::exit(2);
    }

    if parsed.stats {
        eprintln!(
            "loaded {} triples, {} tables matched",
            triples.len(),
            tables.len(),
        );
    }

    for table in &tables {
        match render_classid(table) {
            Some(id) => println!("{table}\t0x{id:08X}"),
            None => println!("{table}\t(uncodified)"),
        }
    }
}

/// The distinct model names declared as `(*, rdf:type, ogit:ObjectType)`,
/// filtered by `focus` (empty = no filter), sorted for deterministic output.
fn object_type_tables(triples: &[Triple], focus: &[&str]) -> Vec<String> {
    let mut tables: Vec<String> = triples
        .iter()
        .filter(|t| t.p == "rdf:type" && t.o == "ogit:ObjectType")
        .map(|t| model_of(&t.s).to_string())
        .filter(|m| focus.is_empty() || focus.contains(&m.as_str()))
        .collect();
    tables.sort_unstable();
    tables.dedup();
    tables
}

#[derive(Debug)]
struct ParsedArgs {
    input: Input,
    focus: Vec<String>,
    stats: bool,
    actions: bool,
    help: bool,
}

#[derive(Debug)]
enum Input {
    Stdin,
    Path(PathBuf),
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut input: Option<Input> = None;
    let mut focus: Vec<String> = Vec::new();
    let mut stats = false;
    let mut actions = false;
    let mut help = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => help = true,
            "--stats" => stats = true,
            "--classids" => {} // the default/only non-actions mode; accepted for CLI compat
            "--actions" => actions = true,
            "-f" | "--focus" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    "-f/--focus requires a comma-separated model list".to_string()
                })?;
                focus = value.split(',').map(|s| s.trim().to_string()).collect();
                focus.retain(|s| !s.is_empty());
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag `{other}`"));
            }
            "-" => input = Some(Input::Stdin),
            path => input = Some(Input::Path(PathBuf::from(path))),
        }
        i += 1;
    }
    Ok(ParsedArgs {
        input: input.unwrap_or(Input::Stdin),
        focus,
        stats,
        actions,
        help,
    })
}

fn read_input(input: &Input) -> io::Result<String> {
    match input {
        Input::Stdin => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
        Input::Path(path) => fs::read_to_string(path),
    }
}

const USAGE: &str = "\
od-codegen — pull canonical OGAR classids / behavioral-arm rows from Odoo SPO-triple ndjson

USAGE:
    od-codegen [INPUT] [-f MODELS] [--classids | --actions] [--stats]

ARGS:
    INPUT                       Path to SPO ndjson file, or `-` for stdin (default: stdin).

OPTIONS:
    -f, --focus MODELS          Comma-separated focus models
                                (e.g. `account_move,res_partner,res_company`).
                                Omit to project the entire corpus.
    --stats                     Print triple / table counts to stderr.
    --classids                  Print the `table → canonical OGAR render classid (0xAABBCCDD)`
                                map (the default mode).
    --actions                   Print the behavioral-arm lowering — one
                                `model.method <TAB> kind <TAB> detail` row per ActionDef
                                (kind = depends|guard). Respects --focus.
    -h, --help                  Show this help.

EXIT CODES (lance-graph#512 convention):
    0  output printed successfully
    1  argument / I/O error (message on stderr)
    2  degenerate input (malformed ndjson, zero triples, zero tables)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_default_is_stdin_no_focus() {
        let p = parse_args(&[]).unwrap();
        assert!(matches!(p.input, Input::Stdin));
        assert!(p.focus.is_empty());
        assert!(!p.stats);
        assert!(!p.actions);
    }

    #[test]
    fn parse_args_classids_flag_accepted() {
        let p = parse_args(&["--classids".into()]).unwrap();
        assert!(!p.actions);
    }

    #[test]
    fn parse_args_actions_flag() {
        let p = parse_args(&["--actions".into()]).unwrap();
        assert!(p.actions);
    }

    #[test]
    fn parse_args_focus_splits_on_comma_and_trims() {
        let p =
            parse_args(&["-f".into(), "account_move, res_partner ,res_company".into()]).unwrap();
        assert_eq!(p.focus, vec!["account_move", "res_partner", "res_company"],);
    }

    #[test]
    fn parse_args_unknown_flag_errors() {
        let err = parse_args(&["--bogus".into()]).expect_err("unknown flag must fail");
        assert!(err.contains("--bogus"));
    }

    #[test]
    fn parse_args_focus_missing_value_errors() {
        let err = parse_args(&["-f".into()]).expect_err("missing focus value must fail");
        assert!(err.contains("focus"));
    }

    #[test]
    fn object_type_tables_filters_by_focus_and_dedupes() {
        let triples = vec![
            Triple {
                s: "odoo:account_move".into(),
                p: "rdf:type".into(),
                o: "ogit:ObjectType".into(),
                f: 1.0,
                c: 1.0,
            },
            Triple {
                s: "odoo:account_move.name".into(),
                p: "rdf:type".into(),
                o: "ogit:Property".into(),
                f: 1.0,
                c: 1.0,
            },
            Triple {
                s: "odoo:res_partner".into(),
                p: "rdf:type".into(),
                o: "ogit:ObjectType".into(),
                f: 1.0,
                c: 1.0,
            },
        ];
        assert_eq!(
            object_type_tables(&triples, &[]),
            vec!["account_move".to_string(), "res_partner".to_string()]
        );
        assert_eq!(
            object_type_tables(&triples, &["account_move"]),
            vec!["account_move".to_string()]
        );
        assert!(object_type_tables(&triples, &["nonexistent_model"]).is_empty());
    }
}
