//! End-to-end "pull OGAR via class": from an SPO corpus to canonical classids
//! and annotated `SurrealQL` DDL — the classid stamped into the `DEFINE TABLE`
//! `COMMENT` clause, so it survives into `SurrealDB`'s own catalog.
//!
//! ```bash
//! cargo run -p od-ontology --example classid_pull --features ogar-emit
//! ```
//!
//! This is the consumer-migration target spelled out (lance-graph #589): the
//! canonical id is pulled for each Odoo model straight from `OdooPort` — no
//! bridge, no registry, no hydration. `account_analytic_line` resolves to the
//! SAME `BILLABLE_WORK_ENTRY` id WoA/SMB `Stundenzettel` and `OpenProject` /
//! `Redmine` `TimeEntry` resolve to — planner times align with billable hours
//! through one codebook lookup.

use od_ontology::{
    concept_classid, corpus_to_schema, emit_via_ogar_annotated, parse_ndjson, render_classid,
    schema_classids,
};

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

    // 1. The table → canonical OGAR classid map (concept low-u16 + full render u32).
    println!("== canonical classids (table → concept / render) ==");
    for (table, concept) in schema_classids(&schema) {
        match concept {
            Some(lo) => {
                let render = render_classid(&table).expect("render present when concept present");
                println!("  {table:<22} concept=0x{lo:04X}  render=0x{render:08X}");
            }
            None => println!("  {table:<22} (uncodified — no canonical analogue)"),
        }
    }

    // 2. The planner↔ERP convergence pin (a pure codebook lookup — needs no
    //    corpus, so it resolves even though account.analytic.line isn't in the
    //    slice-2 focus set above).
    if let Some(id) = concept_classid("account_analytic_line") {
        println!(
            "\n== convergence pin ==\n  \
             account_analytic_line → 0x{id:04X} (BILLABLE_WORK_ENTRY)\n  \
             ≡ WoA/SMB Stundenzettel ≡ OpenProject/Redmine TimeEntry"
        );
    }

    // 3. The annotated DDL — the render classid rides into the DEFINE TABLE
    //    COMMENT clause (queryable later via `INFO FOR TABLE`).
    println!("\n== annotated SurrealQL DDL (classid in COMMENT) ==");
    print!("{}", emit_via_ogar_annotated(&schema));
}
