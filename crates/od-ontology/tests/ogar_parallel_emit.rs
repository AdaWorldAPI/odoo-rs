//! **Stage 1 convergence proof** (feature `ogar-emit`).
//!
//! Emits the real `account.move` slice TWO ways from the same `Schema`:
//! the native bespoke [`ToSql`] path and the canonical
//! `ogar-adapter-surrealql` path (via [`emit_via_ogar`]). Asserts what
//! *converges* (DEFINE TABLE + scalar/`Many2one` DEFINE FIELD, and — since the
//! OGAR Stage-A bump — One2many/Many2many `array<record<…>>`) and pins the
//! remaining Stage-2 *gaps* (computed `VALUE`/`READONLY`,
//! `DEFINE FUNCTION`/`DEFINE EVENT`) so they can't silently change.
//!
//! Run with: `cargo test -p od-ontology --features ogar-emit`.
//!
//! [`ToSql`]: od_ontology::ToSql
//! [`emit_via_ogar`]: od_ontology::emit_via_ogar
#![cfg(feature = "ogar-emit")]

use std::collections::BTreeSet;

use od_ontology::{corpus_to_schema, emit_via_ogar, parse_ndjson, ToSql};

fn fixture() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/account_move.spo.ndjson"
    );
    std::fs::read_to_string(path).expect("account_move fixture present")
}

fn schemas() -> (String, String) {
    let triples = parse_ndjson(&fixture()).expect("fixture parses");
    let schema = corpus_to_schema(&triples, Some(&["account_move"]), None);
    (schema.to_sql(), emit_via_ogar(&schema))
}

/// `DEFINE FIELD <name> ON <table> …` → the set of `<name>`s for `table`.
fn fields_on<'a>(ddl: &'a str, table: &str) -> BTreeSet<&'a str> {
    ddl.lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("DEFINE FIELD ")?;
            let mut parts = rest.splitn(2, " ON ");
            let name = parts.next()?.trim();
            let tbl = parts.next()?.split_whitespace().next()?;
            (tbl == table).then_some(name)
        })
        .collect()
}

#[test]
fn ogar_path_emits_the_account_move_table() {
    let (_native, ogar) = schemas();
    assert!(
        ogar.contains("DEFINE TABLE account_move SCHEMAFULL"),
        "OGAR path did not emit the account_move table:\n{ogar}"
    );
}

/// The OGAR path invents nothing: every field it emits also exists in the
/// native emit. (It may emit *fewer* — the documented gaps below.)
#[test]
fn ogar_fields_are_a_subset_of_native_fields() {
    let (native, ogar) = schemas();
    let native_fields = fields_on(&native, "account_move");
    let ogar_fields = fields_on(&ogar, "account_move");

    assert!(
        ogar_fields.is_subset(&native_fields),
        "OGAR emitted fields absent from the native emit: {:?}",
        ogar_fields.difference(&native_fields).collect::<Vec<_>>()
    );
    assert!(
        ogar_fields.len() > 10,
        "expected the OGAR path to cover the bulk of account_move scalar / \
         Many2one fields, only got {}",
        ogar_fields.len()
    );
}

/// **OGAR Stage A landed (the array<record> gap is closed).** The One2many /
/// Many2many collections the OGAR path used to *drop* (a `HasMany` comment, no
/// column) now converge as `array<record<…>>` fields — the same shape the
/// native emit produces. This test was authored to flip exactly when Stage A
/// lands; it now pins the convergence: nothing array-shaped is dropped, and the
/// stale `HasMany` comment marker is gone.
#[test]
fn array_collections_now_converge_not_dropped() {
    let (native, ogar) = schemas();
    let native_fields = fields_on(&native, "account_move");
    let ogar_fields = fields_on(&ogar, "account_move");

    // The shared adapter now emits the collections as array<record<…>>.
    assert!(
        ogar.contains("TYPE array<record<"),
        "expected array<record<…>> for the One2many/Many2many collections after \
         Stage A:\n{ogar}"
    );
    // …so every native `array<…>` collection field is now ALSO on the OGAR path
    // (no longer dropped). Any non-array field still missing would be a real
    // regression, surfaced by the difference set below.
    for f in native_fields.difference(&ogar_fields) {
        let line = native
            .lines()
            .find(|l| {
                l.trim_start()
                    .starts_with(&format!("DEFINE FIELD {f} ON account_move"))
            })
            .unwrap_or_default();
        assert!(
            !line.contains("array<"),
            "array collection `{f}` is still dropped by the OGAR path after \
             Stage A (native line: {line})"
        );
    }
    // The old HasMany comment marker is gone — the column is real now.
    assert!(
        !ogar.contains("HasMany"),
        "stale HasMany comment marker present after Stage A:\n{ogar}"
    );
}

/// At least one `Many2one` converges as a `record<…>` field on both sides.
#[test]
fn many2one_record_fields_converge() {
    let (native, ogar) = schemas();
    assert!(native.contains("record<"), "native has no record field?");
    assert!(
        ogar.contains("TYPE record<") || ogar.contains("TYPE option<record<"),
        "OGAR path emitted no record<…> field:\n{ogar}"
    );
}

/// **The documented Stage-2 gaps.** The reactive / behavioural layer the
/// native emit produces has no equivalent in the DEFINE TABLE/FIELD-only
/// shared emitter *yet*. If the emitter grows to cover any of these, this
/// test flips and tells us to advance the convergence stage.
#[test]
fn reactive_layer_is_the_known_gap() {
    let (native, ogar) = schemas();

    // DEFINE FUNCTION (compute / action stubs)
    assert!(
        native.contains("DEFINE FUNCTION"),
        "native lost its functions"
    );
    assert!(
        !ogar.contains("DEFINE FUNCTION"),
        "OGAR emitter now emits functions — advance the convergence stage"
    );

    // DEFINE EVENT (reactive recompute + @api.constrains guards)
    assert!(native.contains("DEFINE EVENT"), "native lost its events");
    assert!(
        !ogar.contains("DEFINE EVENT"),
        "OGAR emitter now emits events — advance the convergence stage"
    );

    // Computed VALUE + READONLY (the field type converges; its reactivity does not)
    assert!(
        native.contains("VALUE fn::") && native.contains("READONLY"),
        "native lost its computed VALUE/READONLY"
    );
    assert!(
        !ogar.contains("VALUE fn::") && !ogar.contains("READONLY"),
        "OGAR emitter now emits VALUE/READONLY — advance the convergence stage"
    );
}

#[test]
fn ogar_emit_is_deterministic() {
    let triples = parse_ndjson(&fixture()).expect("parses");
    let schema = corpus_to_schema(&triples, Some(&["account_move"]), None);
    assert_eq!(
        emit_via_ogar(&schema),
        emit_via_ogar(&schema),
        "OGAR emit not stable"
    );
}
