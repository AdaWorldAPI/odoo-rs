//! **Real-source compile pin** — the V3 substrate over a REAL Odoo model file,
//! not a toy fixture.
//!
//! 5+3 council finding (R5, 2026-07-07): every `compile_source` call site was
//! an inline 4–20-line snippet, so "the transpile is complete" had never been
//! exercised against real Odoo source. This test compiles the VERBATIM
//! `addons/account/models/account_move.py` (7 380 lines, vendored at
//! `data/account_move_real.py` exactly like the `account_move_form_view.xml`
//! fixture precedent) through `compile_source` and pins what comes out.
//!
//! The full-addon sweep lives in `examples/real_corpus_probe.rs` (55 files →
//! 71 models / 1 496 actions / 347 kausal, measured 2026-07-07); this test is
//! the committed, always-running slice of that run.

use od_ontology::compile_source;

#[test]
fn real_account_move_compiles_through_the_v3_substrate() {
    let src = include_str!("../../../data/account_move_real.py");
    assert!(src.lines().count() > 7_000, "the fixture is the real file, not a stub");

    let compiled = compile_source(src);

    // The file defines exactly account.move (account.move.line lives in its
    // own file). Counts below are drift-fuses from the 2026-07-07 run — a
    // change means the frontend's harvest or the vendored fixture moved;
    // re-run, re-pin, and say so in the commit.
    assert_eq!(compiled.len(), 1, "models harvested from the real file");

    let move_cc = compiled
        .iter()
        .find(|cc| cc.class.name == "account_move")
        .expect("account.move is lifted");

    // Identity: the canon-high render classid rides the facet.
    assert_eq!(move_cc.facet.facet_classid(), 0x0202_0002);

    // THINK arm: the real model is field-rich (109 declared fields in the
    // briefing; the frontend's harvest of THIS file's declarations lands
    // attributes + associations — pinned from the run).
    assert_eq!(move_cc.class.attributes.len(), 104, "attrs (2026-07-07 fuse)");
    assert_eq!(move_cc.class.associations.len(), 38, "assocs (2026-07-07 fuse)");
    assert_eq!(move_cc.class.computed_fields.len(), 93, "computed fields (2026-07-07 fuse)");

    // DO arm: hundreds of real methods arrive as ActionDefs, a substantial
    // subset with a causal trigger (depends/constrains/onchange).
    assert_eq!(move_cc.actions.len(), 354, "DO-arm ActionDefs (2026-07-07 fuse)");
    assert_eq!(
        move_cc.actions.iter().filter(|a| a.kausal.is_some()).count(),
        92,
        "kausal-carrying subset (2026-07-07 fuse)"
    );

    // Spot identity: the flagship compute exists with its body facts.
    let amount_compute = move_cc
        .actions
        .iter()
        .find(|a| a.predicate == "_compute_amount")
        .expect("the real _compute_amount arrives as an ActionDef");
    assert!(
        !amount_compute.reads.is_empty() || amount_compute.kausal.is_some(),
        "_compute_amount carries facts (reads or kausal)"
    );
}
