//! Consumer-side OGAR classid pins for `od-ontology`.
//!
//! **Rehomed** from the deleted `tests/odoo_ogar_convergence.rs` (the
//! `SurrealQL`-fork delete, operator ruling 2026-07-06: "`SurrealQL` is
//! absolutely deprecated. OGAR V3 for transpile substrate, lance-graph V3 for
//! database."). These are the classid-identity pins from that file —
//! `concept_classid` / `render_classid`, driven off the codebook, never off
//! the deleted `Schema`/`TableDefinition` DDL types. The one test that WAS
//! `Schema`-typed (`schema_classids_full_surface_in_order`) is dropped along
//! with `schema_classids` itself; identity resolution is now demonstrated
//! per-model via `concept_classid`/`render_classid` directly.
//!
//! This is the odoo-rs CONSUMER mirror of OGAR's own
//! `billable_work_entry_converges_across_all_five_ports` pin. It guards that
//! odoo-rs's view through `concept_classid` / `render_classid` stays
//! bit-identical to the OGAR codebook constants re-exported via
//! `od_ontology::class_ids`.
//!
//! Every test here is a boundary contract: if it breaks, the OGAR codebook
//! and the odoo-rs consumer have diverged and ALL downstream consumers
//! (`WoA`, `SMB`, `OpenProject`, `lance-graph-planner`) see a broken pin.
//!
//! # No feature gate
//! OGAR is always compiled into the binary (operator ruling 2026-07-06: no
//! bridges — consumers consume `ogar-vocab` directly; the former `ogar-emit`
//! opt-in is retired).

use od_ontology::{class_ids, concept_classid, render_classid};

// ---------------------------------------------------------------------------
// Individual concept-classid pins (symbol-bound, not hex literals)
// ---------------------------------------------------------------------------

#[test]
fn account_analytic_line_pins_billable_work_entry() {
    // This is the planner↔ERP pin: the SAME class_ids::BILLABLE_WORK_ENTRY
    // constant is what WoA/SMB Stundenzettel AND OpenProject/Redmine TimeEntry
    // resolve to.  If OGAR moves the constant this test breaks — that is the
    // intended behaviour; any move must be co-ordinated across all consumers.
    assert_eq!(
        concept_classid("account_analytic_line"),
        Some(class_ids::BILLABLE_WORK_ENTRY),
    );
}

#[test]
fn account_move_pins_commercial_document() {
    assert_eq!(
        concept_classid("account_move"),
        Some(class_ids::COMMERCIAL_DOCUMENT),
    );
}

#[test]
fn account_move_line_pins_commercial_line_item() {
    assert_eq!(
        concept_classid("account_move_line"),
        Some(class_ids::COMMERCIAL_LINE_ITEM),
    );
}

#[test]
fn res_partner_pins_billing_party() {
    assert_eq!(
        concept_classid("res_partner"),
        Some(class_ids::BILLING_PARTY),
    );
}

// ---------------------------------------------------------------------------
// Full-surface render_classid pin (lens + concept in one u32)
// ---------------------------------------------------------------------------

#[test]
fn render_classid_stamps_odoo_lens_over_concept() {
    // Canon-high (OGAR D-CLASSID-CANON-HIGH-FLIP, 2026-07-02): render_classid
    // encodes (concept_classid << 16) | odoo_lens. The HIGH u16 is the shared
    // concept (cross-app RBAC + ontology); the LOW u16 (0x0002) is the Odoo
    // render lens. Both fields must be stable together.
    assert_eq!(render_classid("account_analytic_line"), Some(0x0103_0002));
    assert_eq!(render_classid("account_move"), Some(0x0202_0002));

    // Assert the lens independently so a bit-shift bug is caught separately
    // from a codebook-value bug — the lens lives in the LOW u16 under canon-high.
    assert_eq!(
        render_classid("account_move").map(|id| id & 0xFFFF),
        Some(0x0002),
        "low u16 of render_classid must be the Odoo lens (0x0002)",
    );
}

// ---------------------------------------------------------------------------
// Negative pin (R4 requirement)
// ---------------------------------------------------------------------------

#[test]
fn uncodified_model_resolves_none() {
    // `ir_cron` is deliberately outside the canonical-identity codebook.
    // Structural lowering used to have a DDL projection covering such a
    // table; that projection is gone (`SurrealQL` fork deleted), but the
    // canonical-identity pull remains codebook-gated, and that gating must
    // stay observable — resolving to Some(...) here would silently hide a
    // codebook expansion that hasn't been reviewed.
    assert_eq!(
        concept_classid("ir_cron"),
        None,
        "ir_cron must NOT resolve to a concept classid; it is not in the OGAR codebook",
    );
    assert_eq!(
        render_classid("ir_cron"),
        None,
        "ir_cron must NOT resolve to a render classid; it is not in the OGAR codebook",
    );
}
