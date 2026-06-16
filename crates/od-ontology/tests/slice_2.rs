//! Slice-2 proving test: four real models projected together.
//!
//! Fixture: every triple whose subject is one of
//! `{account_move, account_move_line, res_partner, res_company}` —
//! **2 739 real rows** of the 22 245-triple corpus. This is the smallest
//! meaningful expansion past slice 1, because it surfaces three shapes the
//! single-model slice can't:
//!
//! 1. **Back-ref resolution within the focus set** — `account_move.line_ids`
//!    must resolve to the child table `account_move_line` (via the
//!    `<parent>_<stem>` Odoo convention), not the placeholder
//!    `account_move__line_ids` used at slice 1.
//! 2. **`res_*` resolution via the namespace convention** —
//!    `account_move.partner_id` must lower to `option<record<res_partner>>`,
//!    not `option<record<partner>>`.
//! 3. **Bi-directional reactivity** — `account_move_line.move_id` is the
//!    back-reference; events emitted on `account_move_line` must fire UPDATE
//!    on `account_move` through it (`LET $parent = $after.move_id`).

use od_ontology::{corpus_to_schema, parse_ndjson, ToSql};

const FOCUS: &[&str] = &[
    "account_move",
    "account_move_line",
    "res_partner",
    "res_company",
];

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/slice_2.spo.ndjson");
    std::fs::read_to_string(path).expect("slice_2 fixture present")
}

fn slice_2_ddl() -> String {
    let triples = parse_ndjson(&fixture()).expect("fixture parses");
    corpus_to_schema(&triples, Some(FOCUS)).to_sql()
}

#[test]
fn fixture_parses() {
    let triples = parse_ndjson(&fixture()).expect("parses");
    assert!(
        triples.len() > 2_500,
        "expected ~2 739 triples, got {}",
        triples.len()
    );
}

#[test]
fn all_four_tables_emitted_schemafull() {
    let ddl = slice_2_ddl();
    for m in FOCUS {
        let expect = format!("DEFINE TABLE {m} SCHEMAFULL TYPE NORMAL;");
        assert!(ddl.contains(&expect), "missing table for {m}");
    }
}

#[test]
fn partner_id_resolves_to_res_partner() {
    // `account_move.partner_id` is `Many2one('res.partner')`. With res_partner
    // in the focus set, the `res_<stem>` convention must resolve. The field
    // is computed (`emitted_by _inverse_partner_id`), so the trailing slot
    // carries `VALUE … READONLY` — assert the resolved TYPE substring.
    let ddl = slice_2_ddl();
    assert!(
        ddl.contains("DEFINE FIELD partner_id ON account_move TYPE option<record<res_partner>>"),
        "partner_id did not resolve to record<res_partner>:\n{}",
        grep(&ddl, "partner_id ON account_move ")
    );
}

#[test]
fn company_id_resolves_to_res_company() {
    let ddl = slice_2_ddl();
    assert!(
        ddl.contains("DEFINE FIELD company_id ON account_move TYPE option<record<res_company>>"),
        "company_id did not resolve to record<res_company>:\n{}",
        grep(&ddl, "company_id ON account_move ")
    );
}

#[test]
fn invoice_line_ids_resolution_is_an_acknowledged_deferred_gap() {
    // Real-corpus oddity worth pinning: `account.move.invoice_line_ids` is
    // semantically `One2many('account.move.line', 'move_id', domain=[…])` —
    // i.e. it points at `account_move_line`, NOT a non-existent
    // `account_move_invoice_line` table. The corpus carries only the field
    // name, so the `<parent>_<stem>` convention misses (stem = `invoice_line`,
    // not `line`), and the bare stem falls through to `record<invoice_line>`.
    //
    // This is the typed `OdooEntity::Many2one`/`One2many` first-argument
    // resolution that the lance-graph-ontology `odoo_blueprint` lift would
    // wire up — the **explicitly deferred** bit. This test PINS the current
    // honest output so a future lift removing this fallback is visible.
    let ddl = slice_2_ddl();
    assert!(
        ddl.contains("DEFINE FIELD invoice_line_ids ON account_move TYPE option<array<record<invoice_line>>>"),
        "expected the deferred-lift fallback `record<invoice_line>` for invoice_line_ids:\n{}",
        grep(&ddl, "invoice_line_ids ON account_move ")
    );
}

#[test]
fn line_ids_is_not_declared_as_property_on_account_move_in_the_corpus() {
    // Honesty pin: the extractor that produced odoo_ontology.spo.ndjson does
    // NOT declare `account_move.line_ids` as `ogit:Property` — only
    // `invoice_line_ids`. The cross-record event for `line_ids` is still
    // emitted because the `depends_on` path-rows trigger it (151 of them);
    // it just doesn't get a DEFINE FIELD of its own. This is a data
    // completeness signal pointing back at the corpus, NOT a projection bug.
    let ddl = slice_2_ddl();
    assert!(
        !ddl.contains("DEFINE FIELD line_ids ON account_move "),
        "the corpus does not declare line_ids; emitting one would be inventing"
    );
}

#[test]
fn cross_record_event_targets_child_table_not_placeholder() {
    // The slice-1 placeholder was `account_move__line_ids`. With the child in
    // focus, the event MUST fire on `account_move_line`.
    let ddl = slice_2_ddl();
    assert!(
        ddl.contains("DEFINE EVENT recompute_account_move_via_line_ids ON account_move_line"),
        "cross-record event for line_ids did not target account_move_line:\n{}",
        grep(&ddl, "recompute_account_move_via_line_ids")
    );
    assert!(
        !ddl.contains("account_move__line_ids"),
        "stale placeholder account_move__line_ids leaked into output"
    );
}

#[test]
fn cross_record_event_uses_back_ref_to_lookup_parent() {
    // The resolved cross-record event must `LET $parent = $after.move_id`
    // (the Odoo One2many inverse) before UPDATEing the parent.
    let ddl = slice_2_ddl();
    let block = grep(&ddl, "recompute_account_move_via_line_ids");
    assert!(
        block.contains("LET $parent = $after.move_id"),
        "back-ref lookup missing from resolved event:\n{block}"
    );
}

#[test]
fn unresolved_cross_record_event_audits_inline() {
    // Some `account_move` deps walk relations whose targets are NOT in the
    // focus set (e.g. `journal_id.fiscal_country_id` → res.country, not in
    // focus). Those must surface as audit-noted TODO events, not silent
    // fallthroughs.
    let ddl = slice_2_ddl();
    assert!(
        ddl.contains("(child UNRESOLVED — not in focus set)"),
        "expected at least one UNRESOLVED audit note for out-of-focus child"
    );
    assert!(
        ddl.contains("/* TODO: resolve child→parent back-ref"),
        "expected TODO audit on the THEN-clause for unresolved events"
    );
}

#[test]
fn focus_isolation_holds_no_out_of_set_subjects_leak_in() {
    // No DEFINE TABLE for any model outside the focus set, even though the
    // corpus contains rows for many other Odoo models.
    let ddl = slice_2_ddl();
    let table_count = ddl.matches("DEFINE TABLE ").count();
    assert_eq!(
        table_count, 4,
        "expected exactly 4 tables, got {table_count}"
    );
}

#[test]
fn output_is_deterministic() {
    assert_eq!(slice_2_ddl(), slice_2_ddl(), "projection not stable");
}

/// The lines of `ddl` mentioning `needle`, joined — for readable failure msgs.
fn grep(ddl: &str, needle: &str) -> String {
    ddl.lines()
        .filter(|l| l.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}
