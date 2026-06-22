//! `od-codegen` — read an SPO-triple ndjson corpus + optional RelationMap and
//! write the rendered SurrealQL schema for an Odoo model focus.
//!
//! # Usage
//!
//! ```sh
//! # Slice 1: account.move alone
//! od-codegen data/account_move.spo.ndjson --focus account_move
//!
//! # Slice 2 + typed lift: 4 models, RelationMap overrides
//! od-codegen data/slice_2.spo.ndjson \
//!   --focus account_move,account_move_line,res_partner,res_company \
//!   --relations data/slice_2.relations.ndjson \
//!   -o /tmp/slice_2.surql --stats
//!
//! # Stdin pipeline
//! cat data/slice_2.spo.ndjson | od-codegen - -f account_move
//! ```
//!
//! # End-to-end pipeline
//!
//! ```text
//!   odoo/addons/  ─►  ruff_python_dto_check + odoo-blueprint-extractor (Python)
//!                  ─►  triples.ndjson (22 245 SPO triples)
//!                  ─►  od-codegen (THIS BINARY)
//!                  ─►  schema.surql (DEFINE TABLE / FIELD / FUNCTION / EVENT)
//!                  ─►  surrealdb
//! ```
//!
//! # Exit codes — `lance-graph#512` convention
//!
//! Following `lance-graph#512`'s degenerate-input-vs-generic-error split so
//! wrapper scripts can react to the cause:
//!
//! - `0` — schema rendered successfully.
//! - `1` — argument / I/O error (message on stderr).
//! - `2` — degenerate input (empty / malformed ndjson, zero triples, zero
//!   tables — the upstream extractor never produced anything meaningful).
//!
//! Pattern parity with `AdaWorldAPI/openproject-nexgen-rs#31` so the two
//! CLIs behave identically across language fronts.
//!
//! # Deferred — `--validate` flag
//!
//! Reserved as a no-op stub. When wired, it will route the emitted DDL
//! through `surrealdb_core::syn::parse` (path-dep on the AdaWorldAPI
//! `surrealdb` fork) and exit 2 on parse failure — proving the projection
//! emits valid SurrealQL, not just plausible-looking strings. Currently
//! gated on disk-space headroom for the fork build; the CLI carries the
//! flag so the hook lands when the dep wires in.

#![cfg(feature = "cli")]

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process;

use od_ontology::{corpus_to_schema, parse_ndjson, RelationMap, Schema, ToSql};

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

    // Degenerate-input guard (#512): an empty triple stream silently emits
    // an empty SurrealQL file, making downstream pipelines fail far from
    // the cause. Exit 2 with a directive message naming the upstream fix.
    if triples.is_empty() {
        eprintln!(
            "error: input contains zero triples — run the upstream extractor first \
             (e.g. `python3 -m odoo_blueprint_extractor --addons /home/user/odoo/addons \
             --addon account` then emit_ontology over the parquet)"
        );
        process::exit(2);
    }

    // ── 2. Load optional RelationMap ──
    let relations: Option<RelationMap> = match &parsed.relations {
        Some(path) => match fs::read_to_string(path) {
            Ok(s) => match RelationMap::from_ndjson(&s) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("error parsing relations ndjson: {e}");
                    process::exit(2);
                }
            },
            Err(e) => {
                eprintln!("error reading relations file `{}`: {e}", path.display());
                process::exit(1);
            }
        },
        None => None,
    };

    if parsed.stats {
        eprintln!(
            "loaded {} triples, {} relation overrides",
            triples.len(),
            relations.as_ref().map_or(0, RelationMap::len),
        );
    }

    // ── 3. Project ──
    let focus_owned: Vec<String> = parsed.focus.clone();
    let focus_refs: Vec<&str> = focus_owned.iter().map(String::as_str).collect();
    let focus_opt: Option<&[&str]> = if focus_refs.is_empty() {
        None
    } else {
        Some(&focus_refs)
    };

    let schema = corpus_to_schema(&triples, focus_opt, relations.as_ref());

    // Degenerate-output guard (#512): zero tables means the focus set
    // didn't intersect any `(*, rdf:type, ogit:ObjectType)` rows. Rather
    // than ship a no-op `.surql`, fail loudly with the cause.
    if schema.tables.is_empty() {
        eprintln!(
            "error: projection emitted zero tables — focus set {:?} did not match any \
             `(*, rdf:type, ogit:ObjectType)` row in the {} input triples",
            focus_owned,
            triples.len(),
        );
        process::exit(2);
    }

    // ── 4. `--classids` — print table → OGAR render classid map ──
    if parsed.classids {
        #[cfg(feature = "ogar-emit")]
        {
            for table in &schema.tables {
                match od_ontology::render_classid(&table.name) {
                    Some(id) => println!("{}\t0x{id:08X}", table.name),
                    None => println!("{}\t(uncodified)", table.name),
                }
            }
            return;
        }
        #[cfg(not(feature = "ogar-emit"))]
        {
            eprintln!(
                "error: --classids requires the `ogar-emit` feature \
                 (rebuild: cargo build -p od-ontology --features cli,ogar-emit)"
            );
            process::exit(1);
        }
    }

    // ── 4b. `--actions` — print the behavioral-arm lowering (ActionDef) ──
    if parsed.actions {
        #[cfg(feature = "ogar-emit")]
        {
            for (model, predicate, kind, detail) in od_ontology::corpus_action_rows(&triples) {
                // Respect --focus: only the focused models, when given.
                if !focus_refs.is_empty() && !focus_refs.iter().any(|&f| f == model) {
                    continue;
                }
                println!("{model}.{predicate}\t{kind}\t{detail}");
            }
            return;
        }
        #[cfg(not(feature = "ogar-emit"))]
        {
            eprintln!(
                "error: --actions requires the `ogar-emit` feature \
                 (rebuild: cargo build -p od-ontology --features cli,ogar-emit)"
            );
            process::exit(1);
        }
    }

    let sql = schema.to_sql();

    // ── 5. `--validate` (deferred stub) ──
    if parsed.validate {
        eprintln!(
            "warning: --validate is a deferred stub (surrealdb-core parser not wired yet); \
             passing through unverified"
        );
    }

    // ── 6. Write ──
    if let Err(e) = write_output(&parsed.output, &sql) {
        eprintln!("error writing output: {e}");
        process::exit(1);
    }

    if parsed.stats {
        print_stats(&schema, triples.len());
    }
}

#[derive(Debug)]
struct ParsedArgs {
    input: Input,
    output: Output,
    focus: Vec<String>,
    relations: Option<PathBuf>,
    stats: bool,
    validate: bool,
    classids: bool,
    actions: bool,
    help: bool,
}

#[derive(Debug)]
enum Input {
    Stdin,
    Path(PathBuf),
}

#[derive(Debug)]
enum Output {
    Stdout,
    Path(PathBuf),
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut input: Option<Input> = None;
    let mut output: Option<Output> = None;
    let mut focus: Vec<String> = Vec::new();
    let mut relations: Option<PathBuf> = None;
    let mut stats = false;
    let mut validate = false;
    let mut classids = false;
    let mut actions = false;
    let mut help = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => help = true,
            "--stats" => stats = true,
            "--validate" => validate = true,
            "--classids" => classids = true,
            "--actions" => actions = true,
            "-f" | "--focus" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    "-f/--focus requires a comma-separated model list".to_string()
                })?;
                focus = value.split(',').map(|s| s.trim().to_string()).collect();
                focus.retain(|s| !s.is_empty());
            }
            "-r" | "--relations" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "-r/--relations requires a path argument".to_string())?;
                relations = Some(PathBuf::from(value));
            }
            "-o" | "--output" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "-o/--output requires a path argument".to_string())?;
                output = Some(if value == "-" {
                    Output::Stdout
                } else {
                    Output::Path(PathBuf::from(value))
                });
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
        output: output.unwrap_or(Output::Stdout),
        focus,
        relations,
        stats,
        validate,
        classids,
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

fn write_output(output: &Output, content: &str) -> io::Result<()> {
    match output {
        Output::Stdout => io::stdout().write_all(content.as_bytes()),
        Output::Path(path) => fs::write(path, content),
    }
}

fn print_stats(schema: &Schema, triple_count: usize) {
    let tables = schema.tables.len();
    let fields: usize = schema.tables.iter().map(|t| t.fields.len()).sum();
    let indices: usize = schema.tables.iter().map(|t| t.indices.len()).sum();
    let functions = schema.functions.len();
    let events = schema.events.len();
    let sql = schema.to_sql();
    let unresolved = sql.matches("(child UNRESOLVED").count();
    let via_typed = sql.matches("via typed-lift").count();
    let via_conv = sql.matches("via convention").count();
    eprintln!(
        "schema: {tables} tables, {fields} fields, {indices} indices, \
         {functions} functions (deferred bodies), {events} events \
         ({via_typed} typed-lift / {via_conv} convention / {unresolved} unresolved) \
         from {triple_count} triples",
    );
}

const USAGE: &str = "\
od-codegen — render SurrealQL schema from Odoo SPO-triple ndjson

USAGE:
    od-codegen [INPUT] [-f MODELS] [-r RELATIONS] [-o OUTPUT] [--stats] [--validate]

ARGS:
    INPUT                       Path to SPO ndjson file, or `-` for stdin (default: stdin).

OPTIONS:
    -f, --focus MODELS          Comma-separated focus models
                                (e.g. `account_move,res_partner,res_company`).
                                Omit to project the entire corpus.
    -r, --relations PATH        Optional RelationMap ndjson (typed-lift overrides
                                for `OdooField.target` ground truth).
    -o, --output PATH           Write SurrealQL to PATH instead of stdout. Use `-` for stdout.
    --stats                     Print schema feature counts (tables / fields / events /
                                typed-lift vs convention vs unresolved) to stderr.
    --validate                  Reserved — when wired, routes the emitted DDL through
                                `surrealdb_core::syn::parse` and exits 2 on syntax error.
                                Currently a no-op stub (warns and passes through).
    --classids                  Print the `table → canonical OGAR render classid (0xAABBCCDD)`
                                map instead of DDL. Requires the `ogar-emit` feature.
    --actions                   Print the behavioral-arm lowering — one
                                `model.method <TAB> kind <TAB> detail` row per ActionDef
                                (kind = depends|guard). Respects --focus. Requires `ogar-emit`.
    -h, --help                  Show this help.

EXIT CODES (lance-graph#512 convention):
    0  schema rendered successfully
    1  argument / I/O error (message on stderr)
    2  degenerate input (malformed ndjson, zero triples, zero tables)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_default_is_stdin_stdout_no_focus() {
        let p = parse_args(&[]).unwrap();
        assert!(matches!(p.input, Input::Stdin));
        assert!(matches!(p.output, Output::Stdout));
        assert!(p.focus.is_empty());
        assert!(p.relations.is_none());
        assert!(!p.stats);
        assert!(!p.validate);
        assert!(!p.classids);
        assert!(!p.actions);
    }

    #[test]
    fn parse_args_classids_flag() {
        let p = parse_args(&["--classids".into()]).unwrap();
        assert!(p.classids);
    }

    #[test]
    fn parse_args_actions_flag() {
        let p = parse_args(&["--actions".into()]).unwrap();
        assert!(p.actions);
        assert!(!p.classids);
    }

    #[test]
    fn parse_args_focus_splits_on_comma_and_trims() {
        let p =
            parse_args(&["-f".into(), "account_move, res_partner ,res_company".into()]).unwrap();
        assert_eq!(p.focus, vec!["account_move", "res_partner", "res_company"],);
    }

    #[test]
    fn parse_args_relations_long_and_short() {
        let p1 = parse_args(&["-r".into(), "/tmp/r.ndjson".into()]).unwrap();
        let p2 = parse_args(&["--relations".into(), "/tmp/r.ndjson".into()]).unwrap();
        assert_eq!(
            p1.relations.as_deref(),
            Some(std::path::Path::new("/tmp/r.ndjson"))
        );
        assert_eq!(p1.relations, p2.relations);
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
}
