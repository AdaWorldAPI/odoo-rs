//! Slice-1 proving test: the real `account.move` triple fixture (1 647 rows
//! extracted from the 22 245-triple corpus) projects to faithful SurrealQL DDL.
//!
//! These assertions pin the *ontology-shape contract*, not incidental output:
//! a computed field carries its `VALUE` + `READONLY`, its same-record
//! `@api.depends` set is audited inline, guards become `THROW` events, and
//! cross-record deps become recompute events. If the projection ever stops
//! emitting one of these, the corresponding Odoo semantic was silently lost.

use od_ontology::{corpus_to_schema, parse_ndjson, ToSql};

fn fixture() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/account_move.spo.ndjson"
    );
    std::fs::read_to_string(path).expect("account_move fixture present")
}

fn account_move_ddl() -> String {
    let triples = parse_ndjson(&fixture()).expect("fixture parses");
    let schema = corpus_to_schema(&triples, Some("account_move"));
    schema.to_sql()
}

#[test]
fn fixture_parses_to_expected_corpus_size() {
    let triples = parse_ndjson(&fixture()).expect("parses");
    // The whole fixture is account_move-subject; sanity-check the real size.
    assert!(
        triples.len() > 1_500,
        "expected the full account_move slice, got {}",
        triples.len()
    );
}

#[test]
fn emits_the_table_schemafull() {
    let ddl = account_move_ddl();
    assert!(
        ddl.contains("DEFINE TABLE account_move SCHEMAFULL TYPE NORMAL;"),
        "table definition missing"
    );
}

#[test]
fn computed_field_carries_value_and_readonly() {
    // `amount_residual emitted_by _compute_amount` in the real corpus.
    let ddl = account_move_ddl();
    assert!(
        ddl.contains(
            "DEFINE FIELD amount_residual ON account_move TYPE option<any> \
             VALUE fn::account_move::_compute_amount($this) READONLY;"
        ),
        "computed field amount_residual did not lower to VALUE + READONLY:\n{}",
        grep(&ddl, "amount_residual")
    );
}

#[test]
fn same_record_depends_audited_inline() {
    // `abnormal_amount_warning depends_on partner_id` (same-record, no dot) must
    // surface as the `-- reactive over:` audit comment rendered immediately
    // ABOVE the field's DEFINE line.
    let ddl = account_move_ddl();
    let comment = comment_above(&ddl, "DEFINE FIELD abnormal_amount_warning ON account_move")
        .expect("abnormal_amount_warning field emitted with an audit comment");
    assert!(
        comment.starts_with("-- reactive over:") && comment.contains("partner_id"),
        "same-record depends_on not audited above the field:\n{comment}"
    );
}

#[test]
fn relation_fields_typed_as_record() {
    // `*_id` → record<…>, `*_ids` → array<record<…>>. Assert the lowering
    // pattern (some field of each shape exists) rather than betting on one
    // field name that may be computed or inherited.
    let ddl = account_move_ddl();
    assert!(
        ddl.contains("TYPE option<record<"),
        "no *_id field lowered to option<record<…>>"
    );
    assert!(
        ddl.contains("TYPE option<array<record<"),
        "no *_ids field lowered to option<array<record<…>>>"
    );
}

#[test]
fn compute_methods_emitted_as_deferred_stubs() {
    let ddl = account_move_ddl();
    assert!(
        ddl.contains(
            "DEFINE FUNCTION fn::account_move::_compute_amount($this: record<account_move>) \
             { /* deferred: port from Python */ RETURN NONE; };"
        ),
        "compute method stub missing:\n{}",
        grep(&ddl, "_compute_amount(")
    );
}

#[test]
fn constrains_guard_becomes_throw_event() {
    // `_check_invoice_currency_rate raises ValidationError`.
    let ddl = account_move_ddl();
    let block = grep(&ddl, "check_invoice_currency_rate");
    assert!(
        block.contains("DEFINE EVENT")
            && block.contains("THROW")
            && block.contains("ValidationError"),
        "guard did not lower to a THROW event:\n{block}"
    );
}

#[test]
fn cross_record_depends_becomes_recompute_event() {
    // The fixture has 130 dotted cross-record deps (e.g. company_id.vat).
    let ddl = account_move_ddl();
    assert!(
        ddl.contains("-- cross-record @api.depends:") && ddl.contains("DEFINE EVENT recompute_"),
        "cross-record reactive events missing"
    );
}

#[test]
fn output_is_deterministic() {
    assert_eq!(
        account_move_ddl(),
        account_move_ddl(),
        "projection not stable"
    );
}

/// The lines of `ddl` mentioning `needle`, joined — for readable failure msgs.
fn grep(ddl: &str, needle: &str) -> String {
    ddl.lines()
        .filter(|l| l.contains(needle))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The line immediately preceding the first line that starts with `prefix`
/// (the field's audit comment renders directly above its `DEFINE FIELD`).
fn comment_above<'a>(ddl: &'a str, prefix: &str) -> Option<&'a str> {
    let lines: Vec<&str> = ddl.lines().collect();
    let i = lines.iter().position(|l| l.starts_with(prefix))?;
    i.checked_sub(1).map(|j| lines[j])
}
