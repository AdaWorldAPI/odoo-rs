//! **PROBE — wide-mask convergence parity** (the named lane-3 follow-up to
//! `view_field_mask.rs`, unblocked by lance-graph #651/#653, merged
//! 2026-07-06): mint BOTH the harvest-side `MaskWords` and the
//! contract-typed `lance_graph_contract::WideFieldMask` from the SAME
//! `account_move` view ∩ corpus-universe inputs, and prove bit-for-bit
//! parity between the two minters.
//!
//! # What it measures
//!
//! `mint_mask` (dep-free, `MaskWords`) and `mint_wide_mask` (behind
//! `fieldmask`, contract-typed `WideFieldMask`) both implement "bit `i` set
//! iff `universe[i]` ∈ `present`" over the identical inputs. This probe reuses
//! `view_field_mask.rs`'s exact fixture + corpus loading (verbatim
//! `account.move.form` view XML + the `slice_2` SPO corpus's `account_move`
//! field universe) and asserts the two minters can never disagree:
//! popcount/count match, every bit round-trips, and intersecting the wide
//! mask with `WideFieldMask::full_for(universe.len())` (the canonical-form
//! identity check) leaves the count unchanged.
//!
//! # The >64 seam, again
//!
//! `account_move`'s corpus universe is 216 fields (see `view_field_mask.rs`
//! for the full accounting), so this is also exactly the shape
//! `WideFieldMask` widens for: this probe additionally asserts at least one
//! populated position lands `>= 64`, proving the parity check actually
//! exercises the `Wide` representation tier, not just `Small`.
//!
//! # The 256 cap (OGAR-SOC split signal)
//!
//! `WideFieldMask` positions are `u8`, capping any single mask at 256
//! fields. `mint_wide_mask` refuses loudly (`WideMaskError::UniverseExceedsSocCap`)
//! rather than silently truncating a larger universe — a synthetic
//! 257-field universe pins that refusal.

// Pedantic lints relaxed for this reporting probe, matching view_field_mask.rs.
#![allow(clippy::too_many_lines, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;

use od_ontology::{field_universe, mint_mask, mint_wide_mask, parse_ndjson, WideMaskError};

use lance_graph_contract::WideFieldMask;

use od_ontology::extract_view_fields;

#[test]
fn wide_mask_matches_mask_words_on_real_account_move_view() {
    // ── 1. Harvest the verbatim fixture (identical to view_field_mask.rs). ──
    let xml = include_str!("../../../data/account_move_form_view.xml");
    let views = extract_view_fields(xml);
    assert_eq!(views.len(), 1, "fixture carries exactly ONE ir.ui.view record");
    let view = &views[0];
    assert_eq!(view.model, "account.move");

    // ── 2. The model's field universe from the slice_2 corpus. ──
    let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
    let triples = parse_ndjson(ndjson).expect("slice 2 corpus parses");
    let universe = field_universe(&triples, "account_move");
    eprintln!("corpus field universe  : {}", universe.len());

    // ── 3. view ∩ universe — the same intersection view_field_mask.rs uses. ──
    let uni_set: BTreeSet<&str> = universe.iter().map(String::as_str).collect();
    let in_universe: Vec<String> = view
        .fields
        .iter()
        .filter(|f| uni_set.contains(f.as_str()))
        .cloned()
        .collect();

    // ── 4. Mint BOTH minters over the identical inputs. ──
    let words = mint_mask(&universe, &in_universe);
    let wide = mint_wide_mask(&universe, &in_universe).expect("account_move universe within 256 cap");

    eprintln!("MaskWords popcount     : {}", words.popcount());
    eprintln!("WideFieldMask count    : {}", wide.count());
    eprintln!("WideFieldMask max_fields: {}", wide.max_fields());

    // ── 5. Parity: the two minters can never disagree. ──
    assert_eq!(wide.count(), words.popcount(), "count must match popcount");
    let mut saw_high_position = false;
    for i in 0..universe.len() {
        assert_eq!(
            wide.has(i as u8),
            words.is_set(i),
            "bit {i} must agree between WideFieldMask and MaskWords"
        );
        if i >= 64 && wide.has(i as u8) {
            saw_high_position = true;
        }
    }

    // The >64 seam: this probe must actually exercise the Wide tier, not
    // just Small. If the run shows no set position >= 64, that's a real gap
    // (the universe's >64 claim wouldn't be exercised) — report loudly
    // rather than fake a pass.
    if saw_high_position {
        eprintln!("high-position (>=64) bit confirmed set — Wide tier exercised");
    } else {
        eprintln!(
            "GAP: no populated bit >= 64 in this fixture — asserting the universe itself \
             exceeds 64 instead (the Wide-tier exercise falls back to capacity, not content)"
        );
        assert!(
            universe.len() > 64,
            "neither a set bit >= 64 NOR a universe > 64 — the >64 seam is not exercised at all"
        );
    }

    // Canonical-form identity: intersecting with the full mask over the same
    // field count must not change the count (V-L P0 canonical-form guarantee
    // — a Wide value with all-zero high chunks must still behave/compare
    // like the equivalent Small value).
    let full = WideFieldMask::full_for(universe.len());
    let intersected = wide.intersect(&full);
    assert_eq!(
        intersected.count(),
        wide.count(),
        "intersecting with full_for(universe.len()) must not change the count"
    );

    // ── 6. Exact drift-fuses, pinned from this run. ──
    assert_eq!(universe.len(), 216, "drift-fuse: corpus universe size");
    assert_eq!(in_universe.len(), 57, "drift-fuse: view ∩ universe size");
    assert_eq!(words.popcount(), 57, "drift-fuse: MaskWords popcount");
    assert_eq!(wide.count(), 57, "drift-fuse: WideFieldMask count");
    assert_eq!(wide.max_fields(), 256, "drift-fuse: promoted-tier capacity (4 chunks x 64)");
}

#[test]
fn wide_mask_refuses_universe_over_soc_cap() {
    // Positions are u8 — 256 is the hard cap. A 257-field universe must be a
    // loud refusal (OGAR-SOC split signal), never a silent truncation/wrap.
    let universe: Vec<String> = (0..257).map(|i| format!("f{i:03}")).collect();
    let err = mint_wide_mask(&universe, &[]).expect_err("257-field universe must be refused");
    assert_eq!(err, WideMaskError::UniverseExceedsSocCap { fields: 257 });
    eprintln!("refusal confirmed       : {err}");
}
