//! Emit the `account.move` ontology slice as SurrealQL DDL.
//!
//! ```bash
//! cargo run -p od-ontology --example emit_account_move
//! ```
//!
//! Prints the full `DEFINE TABLE / FIELD / FUNCTION / EVENT` projection of the
//! real 1 647-triple `account.move` fixture, plus a one-line summary of the
//! reactive surface it captured.

use od_ontology::{corpus_to_schema, parse_ndjson, ToSql};

fn main() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/account_move.spo.ndjson"
    );
    let ndjson = std::fs::read_to_string(path).expect("account_move fixture present");
    let triples = parse_ndjson(&ndjson).expect("fixture parses");
    let schema = corpus_to_schema(&triples, Some(&["account_move"]));
    let ddl = schema.to_sql();

    println!("{ddl}");
    eprintln!(
        "\n-- summary: {} fields, {} functions (deferred bodies), {} events \
         (reactive + guards) from {} triples",
        ddl.matches("DEFINE FIELD").count(),
        ddl.matches("DEFINE FUNCTION").count(),
        ddl.matches("DEFINE EVENT").count(),
        triples.len(),
    );
}
