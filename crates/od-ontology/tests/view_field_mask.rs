//! **PROBE — view-stratum seam** (E-MIRROR-EXTERNALIZATION, mirror of
//! D-AR-3.5): mint `FieldMask`-ready projections from a REAL Odoo
//! `ir.ui.view` record.
//!
//! # What it measures
//!
//! Odoo externalizes its views as `ir.ui.view` XML; a view is a
//! field-projection of a model — the exact shape lance-graph's
//! `ClassView × FieldMask` expresses. This probe harvests the PRIMARY
//! `account.move` form view (`view_move_form`, copied verbatim into
//! `data/account_move_form_view.xml` from
//! `/home/user/odoo/addons/account/views/account_move_views.xml`), splits
//! its fields into this-model fields vs comodel `relation_hops`, and mints
//! the projection against the slice_2 corpus's `account_move` field
//! universe as plain LSB-first `Vec<u64>` bit-words (`MaskWords` — the
//! harvest artifact the `WideFieldMask` widening (lance-graph #651, merged)
//! consumes; deliberately NOT the contract type).
//!
//! # The >64 seam
//!
//! `account.move` is WHY the widening exists: the briefing cites ~109
//! fields on the full model; the slice_2 corpus universe actually carries
//! **216** `ogit:Property` members for `account_move` (the corpus slice
//! includes extension-module fields), so the mask needs 4 words — well past
//! the 64-bit boundary either way.
//!
//! # Honest gap
//!
//! Fields present on the view but missing from the corpus universe are a
//! harvest-gap metric (mostly framework/UI-support fields like `state`,
//! `id`, `ref` that the Property extraction didn't cover) — reported
//! loudly, never hidden: the mask's popcount counts only the
//! view ∩ universe intersection.

// Pedantic lints relaxed for this reporting probe: it prints a detailed
// human-facing report and computes display percentages from small counts.
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, clippy::doc_markdown)]

use std::collections::BTreeSet;

use od_ontology::{extract_view_fields, field_universe, mint_mask, parse_ndjson};

#[test]
fn view_field_mask_from_real_ir_ui_view() {
    // ── 1. Harvest the verbatim fixture (ONE ir.ui.view record). ──
    let xml = include_str!("../../../data/account_move_form_view.xml");
    let views = extract_view_fields(xml);
    assert_eq!(views.len(), 1, "fixture carries exactly ONE ir.ui.view record");
    let view = &views[0];
    assert_eq!(view.model, "account.move");
    assert_eq!(view.view_name, "account.move.form");

    eprintln!("──── view-stratum seam (view_move_form → FieldMask words) ────");
    eprintln!("view_name              : {}", view.view_name);
    eprintln!("model                  : {}", view.model);
    eprintln!("top-level fields       : {}", view.fields.len());
    eprintln!("relation hops          : {}", view.relation_hops.len());
    eprintln!("first 10 fields        : {:?}", &view.fields[..10.min(view.fields.len())]);

    // Spot-pins — verified against the actual fixture content.
    for must in ["partner_id", "invoice_date", "journal_id", "invoice_line_ids", "line_ids"] {
        assert!(
            view.fields.iter().any(|f| f == must),
            "expected top-level field `{must}` on view_move_form"
        );
    }
    // Nested fields belong to the comodel, NOT account.move — they must be
    // hops, not fields.
    let hop = |o: &str, i: &str| (o.to_string(), i.to_string());
    assert!(view.relation_hops.contains(&hop("invoice_line_ids", "quantity")));
    assert!(view.relation_hops.contains(&hop("invoice_line_ids", "price_unit")));
    assert!(view.relation_hops.contains(&hop("line_ids", "debit")));
    assert!(
        !view.fields.iter().any(|f| f == "quantity"),
        "comodel field `quantity` must NOT leak into the model's field list"
    );
    // The ir.ui.view metadata / arch wrapper never count as fields.
    assert!(!view.fields.iter().any(|f| f == "arch"));

    // ── 2. The model's field universe from the slice_2 corpus. ──
    let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
    let triples = parse_ndjson(ndjson).expect("slice 2 corpus parses");
    let universe = field_universe(&triples, "account_move");
    eprintln!("corpus field universe  : {} (account_move ogit:Property members)", universe.len());
    assert!(
        universe.len() > 64,
        "the >64 widening motivation: account_move universe must exceed one word (saw {})",
        universe.len()
    );
    // Briefing cites ~109 fields on the full model; the slice_2 corpus
    // carries MORE (extension modules in-slice). Report, don't hide.
    eprintln!(
        "  (briefing cited ~109 fields; corpus slice carries {} — superset, extension modules in-slice)",
        universe.len()
    );

    // ── 3. Mint the mask over view ∩ universe. ──
    // The view's field names use Odoo's dotted-model field spelling already
    // (plain member names), so the intersection is a straight name match.
    let uni_set: BTreeSet<&str> = universe.iter().map(String::as_str).collect();
    let in_universe: Vec<String> = view
        .fields
        .iter()
        .filter(|f| uni_set.contains(f.as_str()))
        .cloned()
        .collect();
    let gap: Vec<&String> = view
        .fields
        .iter()
        .filter(|f| !uni_set.contains(f.as_str()))
        .collect();
    let coverage = 100.0 * in_universe.len() as f64 / view.fields.len() as f64;

    let mask = mint_mask(&universe, &in_universe);
    eprintln!("mask words             : {} (universe {} → div_ceil(64))", mask.0.len(), universe.len());
    eprintln!("popcount               : {}", mask.popcount());
    eprintln!(
        "coverage               : {}/{} view fields in-universe ({coverage:.1}%)",
        in_universe.len(),
        view.fields.len()
    );
    eprintln!(
        "harvest gap            : {} view fields missing from corpus universe: {gap:?}",
        gap.len()
    );

    assert_eq!(mask.0.len(), universe.len().div_ceil(64));
    assert!(
        mask.0.len() >= 2,
        "the >64 seam: mask must span multiple words (saw {})",
        mask.0.len()
    );
    assert_eq!(mask.popcount() as usize, in_universe.len());
    // Every bit round-trips: set iff the universe field is on the view.
    for (i, field) in universe.iter().enumerate() {
        assert_eq!(
            mask.is_set(i),
            in_universe.iter().any(|f| f == field),
            "bit {i} ({field}) must round-trip"
        );
    }
    assert!(!mask.is_set(universe.len()), "out-of-range bits read unset");

    // ── 4. Exact drift-fuses (pinned from this fixture + this corpus). ──
    // If the fixture or the corpus slice changes, these move — deliberately
    // loud, so the seam never drifts silently.
    assert_eq!(view.fields.len(), 86, "drift-fuse: top-level field count");
    assert_eq!(view.relation_hops.len(), 44, "drift-fuse: deduped hop count");
    assert_eq!(universe.len(), 216, "drift-fuse: corpus universe size");
    assert_eq!(mask.0.len(), 4, "drift-fuse: mask word count");
    assert_eq!(mask.popcount(), 57, "drift-fuse: view ∩ universe popcount");
    assert_eq!(gap.len(), 29, "drift-fuse: harvest-gap size");
}
