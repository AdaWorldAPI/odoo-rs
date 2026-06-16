//! Slice-2 with the typed-lift bridge: the `OdooField.target` ground truth
//! overrides the name-heuristic ladder and **closes the deferred gap**.
//!
//! These tests run the same projection over the same 2 739-triple fixture as
//! `slice_2.rs`, but with a hand-crafted `slice_2.relations.ndjson` standing in
//! for what the `od-ontology-bridge` binary will later extract from
//! `lance-graph-ontology::odoo_blueprint::ENTITIES` verbatim.
//!
//! The proof: every claim `slice_2.rs` pins as "deferred / acknowledged gap"
//! flips to a positive resolution here, demonstrating that swapping the
//! convention ladder for the typed truth is the right next-rung move.

use od_ontology::{corpus_to_schema, parse_ndjson, RelationMap, ToSql};

const FOCUS: &[&str] = &[
    "account_move",
    "account_move_line",
    "res_partner",
    "res_company",
];

fn triples() -> Vec<od_ontology::Triple> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/slice_2.spo.ndjson");
    let ndjson = std::fs::read_to_string(path).expect("slice_2 fixture present");
    parse_ndjson(&ndjson).expect("fixture parses")
}

fn relations() -> RelationMap {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/slice_2.relations.ndjson"
    );
    let ndjson = std::fs::read_to_string(path).expect("slice_2 relations present");
    RelationMap::from_ndjson(&ndjson).expect("relations parse")
}

fn ddl_with_lift() -> String {
    corpus_to_schema(&triples(), Some(FOCUS), Some(&relations())).to_sql()
}

fn ddl_without_lift() -> String {
    corpus_to_schema(&triples(), Some(FOCUS), None).to_sql()
}

#[test]
fn relations_fixture_loads() {
    let m = relations();
    assert!(m.len() >= 10, "expected ~14 overrides, got {}", m.len());
    // Spot-check the one the convention can't reach.
    assert_eq!(
        m.target("account_move", "invoice_line_ids"),
        Some("account_move_line")
    );
    assert_eq!(m.inverse("account_move", "line_ids"), Some("move_id"));
}

#[test]
fn typed_lift_closes_the_invoice_line_ids_deferred_gap() {
    // The slice_2 `acknowledged_deferred_gap` pin: without the lift, the
    // convention misses (stem = "invoice_line"; no "invoice_line",
    // "res_invoice_line", or "account_move_invoice_line" in focus) and falls
    // back to `record<invoice_line>`. With the lift, the projection produces
    // the canonical `record<account_move_line>` — the Odoo decorator's first
    // positional arg.
    let with_lift = ddl_with_lift();
    let without = ddl_without_lift();
    assert!(
        without.contains(
            "DEFINE FIELD invoice_line_ids ON account_move TYPE option<array<record<invoice_line>>>"
        ),
        "expected the heuristic to MISS without the lift"
    );
    assert!(
        with_lift.contains(
            "DEFINE FIELD invoice_line_ids ON account_move TYPE option<array<record<account_move_line>>>"
        ),
        "typed lift did not close the gap:\n{}",
        grep(&with_lift, "invoice_line_ids ON account_move ")
    );
}

#[test]
fn typed_lift_resolves_bank_partner_id_to_res_partner() {
    // Another conviction-missing case: `account_move.bank_partner_id` is
    // `Many2one('res.partner')`. The heuristic strips `_id` → `bank_partner`,
    // tries `res_bank_partner` (no), `account_move_bank_partner` (no) → bare
    // fallback `record<bank_partner>`. The lift carries the truth.
    let with_lift = ddl_with_lift();
    let without = ddl_without_lift();
    assert!(
        without.contains(
            "DEFINE FIELD bank_partner_id ON account_move TYPE option<record<bank_partner>>"
        ),
        "expected heuristic miss on bank_partner_id"
    );
    assert!(
        with_lift.contains(
            "DEFINE FIELD bank_partner_id ON account_move TYPE option<record<res_partner>>"
        ),
        "lift did not resolve bank_partner_id to res_partner"
    );
}

#[test]
fn cross_record_event_provenance_audit_shifts_from_convention_to_typed_lift() {
    // The event comment carries `(via convention)` without the lift and
    // `(via typed-lift)` with it — visible audit-trail of which path emitted
    // each event, so a reader can tell at a glance which relations have been
    // grounded.
    let with_lift = ddl_with_lift();
    let without = ddl_without_lift();
    assert!(
        without.contains("(child=account_move_line via convention"),
        "expected 'via convention' audit without lift"
    );
    assert!(
        with_lift.contains("(child=account_move_line via typed-lift"),
        "expected 'via typed-lift' audit with lift"
    );
}

#[test]
fn invoice_line_ids_cross_record_event_targets_account_move_line_only_with_lift() {
    // Without the lift the event would fire on
    // `account_move_invoice_line_ids` (the unresolved placeholder), or — given
    // the depends_on rows for this rel — drop to UNRESOLVED. With the lift it
    // targets the canonical child `account_move_line` exactly like `line_ids`.
    let with_lift = ddl_with_lift();
    // Either both `line_ids` and `invoice_line_ids` events name `account_move_line`,
    // or there's exactly one such event because the recompute payloads dedupe —
    // both are healthy. What MUST NOT happen is an `invoice_line_ids` event
    // firing on a placeholder or an unresolved table.
    let invoice_event_block = grep(&with_lift, "recompute_account_move_via_invoice_line_ids");
    assert!(
        invoice_event_block.contains("ON account_move_line"),
        "invoice_line_ids cross-record event should target account_move_line:\n{invoice_event_block}"
    );
    assert!(
        !with_lift.contains(
            "recompute_account_move_via_invoice_line_ids ON account_move__invoice_line_ids"
        ),
        "placeholder leaked"
    );
}

#[test]
fn back_ref_resolution_uses_typed_inverse_when_supplied() {
    // The relations override carries `inverse: "move_id"` for line_ids; the
    // event must `LET $parent = $after.move_id` (matches the convention here
    // by happy accident — but the test would also catch a divergence such as
    // an Odoo model that declares a non-conventional inverse).
    let with_lift = ddl_with_lift();
    let block = grep(&with_lift, "recompute_account_move_via_line_ids");
    assert!(
        block.contains("LET $parent = $after.move_id"),
        "back-ref did not use the typed inverse:\n{block}"
    );
}

#[test]
fn output_is_deterministic_with_lift() {
    assert_eq!(
        ddl_with_lift(),
        ddl_with_lift(),
        "lift projection not stable"
    );
}

/// The lines of `ddl` mentioning `needle`, joined — for readable failure msgs.
fn grep(ddl: &str, needle: &str) -> String {
    ddl.lines()
        .filter(|l| l.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}
