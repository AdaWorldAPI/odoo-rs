//! Emit slice-2 (4 models) — `account.move` + `account.move.line` +
//! `res.partner` + `res.company` — as SurrealQL DDL.
//!
//! ```bash
//! cargo run -p od-ontology --example emit_slice_2
//! ```
//!
//! The point of slice 2 over slice 1: within a focus *set*, cross-record
//! `@api.depends` resolves to real child tables (`account_move.line_ids` →
//! `account_move_line`) and `Many2one` targets to namespaced models
//! (`account_move.partner_id` → `res_partner`). Out-of-focus relations are
//! audited inline (`UNRESOLVED — not in focus set`) — never silently dropped.

use od_ontology::{corpus_to_schema, parse_ndjson, ToSql};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/slice_2.spo.ndjson");
    let ndjson = std::fs::read_to_string(path).expect("slice_2 fixture present");
    let triples = parse_ndjson(&ndjson).expect("fixture parses");
    let schema = corpus_to_schema(
        &triples,
        Some(&[
            "account_move",
            "account_move_line",
            "res_partner",
            "res_company",
        ]),
        None,
    );
    let ddl = schema.to_sql();

    println!("{ddl}");
    eprintln!(
        "\n-- summary: {} tables, {} fields, {} functions (deferred bodies), \
         {} events (reactive + guards), {} unresolved-child audit notes — \
         from {} triples",
        ddl.matches("DEFINE TABLE ").count(),
        ddl.matches("DEFINE FIELD ").count(),
        ddl.matches("DEFINE FUNCTION ").count(),
        ddl.matches("DEFINE EVENT ").count(),
        ddl.matches("UNRESOLVED").count(),
        triples.len(),
    );
}
