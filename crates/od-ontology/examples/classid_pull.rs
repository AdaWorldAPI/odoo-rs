//! End-to-end "pull OGAR via class": from an SPO corpus to canonical classids
//! — no bridge, no registry, no hydration, no `SurrealQL`.
//!
//! ```bash
//! cargo run -p od-ontology --example classid_pull
//! ```
//!
//! **`SurrealQL` is deprecated** (operator ruling 2026-07-06: *"`SurrealQL` is
//! absolutely deprecated. OGAR V3 for transpile substrate, lance-graph V3 for
//! database."*). This example used to also print the annotated `DEFINE TABLE
//! … COMMENT` DDL via `emit_via_ogar_annotated`; that emit path is deleted
//! along with the rest of the `SurrealQL` fork (`surreal_ast.rs` / `emit.rs`).
//! What survives — and is demonstrated here — is the classid pull itself:
//! `concept_classid` / `render_classid` resolve straight from `OdooPort`, the
//! consumer-migration target spelled out in lance-graph #589.
//!
//! `account_analytic_line` resolves to the SAME `BILLABLE_WORK_ENTRY` id
//! WoA/SMB `Stundenzettel` and `OpenProject`/`Redmine` `TimeEntry` resolve to
//! — planner times align with billable hours through one codebook lookup.

use od_ontology::{concept_classid, model_of, parse_ndjson, render_classid, Triple};

/// The distinct model names declared as `(*, rdf:type, ogit:ObjectType)`,
/// filtered by `focus` (empty = no filter), sorted for deterministic output.
/// Mirrors `od-codegen`'s `--classids` mode — reads straight off the SPO
/// triples, no bespoke `Schema` AST involved.
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

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/slice_2.spo.ndjson");
    let ndjson = std::fs::read_to_string(path).expect("slice_2 fixture present");
    let triples = parse_ndjson(&ndjson).expect("fixture parses");
    let focus = [
        "account_move",
        "account_move_line",
        "res_partner",
        "res_company",
    ];
    let tables = object_type_tables(&triples, &focus);

    // 1. The table → canonical OGAR classid map (concept low-u16 + full render u32).
    println!("== canonical classids (table → concept / render) ==");
    for table in &tables {
        match concept_classid(table) {
            Some(lo) => {
                let render = render_classid(table).expect("render present when concept present");
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
}
