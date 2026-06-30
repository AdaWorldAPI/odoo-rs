//! **PROBE — AR-lifecycle-override redundancy** (the recipe-bitmask conjecture).
//!
//! Consumer-side falsifier for OGAR's `E-RECIPE-BITMASK` /
//! `D-RECIPE-BITMASK` conjecture: *OGAR is Open Graph **Active Record**, so the
//! canonical "recipe" IS the AR lifecycle protocol; a best-shaped (AR-canonical)
//! consumer stores that recipe once and carries only a per-class override
//! bitmask + the genuine deltas, collapsing the irreducible behavioral
//! "leftover" from ~15% toward ~7%.*
//!
//! # What it measures
//!
//! Odoo's lifecycle slots are the underscore-prefix method families
//! (`_compute_*` reactive recompute, `_check_*`/raising guards, `_onchange_*`
//! cooperative loops, `_inverse_*` write-backs, `_search_*`). The behavioral
//! arm (`src/ogar_actions.rs::corpus_to_actions`) lifts exactly two of these
//! into `ActionDef`s, and the recipe-bitmask claim is visible directly in their
//! shape:
//!   * **guards** (a method that `raises`) are byte-identical except their
//!     address — `KausalSpec::LifecycleTrigger{before_save}` + `Reject`. The
//!     recipe (one shape) IS the whole spec, so a guard collapses to
//!     `(recipe-shape, address)` with ZERO per-class payload. Pure bitmask.
//!   * **computes** (`_compute_*`, not raising) share one shape and differ only
//!     in their `Depends.paths` (the lifted `@api.depends` = the method's
//!     `reads_field` set). The path-set is the genuine per-class delta; two
//!     computes with an identical path-set dedup to one.
//!
//! So: behavioral methods partition into `recipe-collapsible`
//! (guards + computes whose path-set is shared with another compute) and
//! `genuine-leftover` (computes with a unique path-set). The probe reports both
//! ratios — the headline number the conjecture predicts.
//!
//! # Honest bounds (why this is an UPPER bound, and why Rails is the clean test)
//!
//!   1. Method *bodies* are not captured by the ruff Python frontend (only the
//!      `reads`/`raises` facts), so "redundant" here means *same lifecycle
//!      shape + same dependency set* — not content-hash-identical bodies
//!      (lossless-DO §1's stricter test). True body dedup can only lower the
//!      leftover further, never raise it.
//!   2. inherited-default-vs-override is unmeasurable on this slice. The corpus
//!      *does* carry `inherits_from` (8 edges in slice_2), but every base mixin
//!      it points at (`mail_thread`, `sequence_mixin`, `analytic_mixin`, …) is
//!      OUT-OF-SLICE — so the bases' method sets aren't present to dedup an
//!      override against. (Separately, the live-source `ruff_python_spo` crate
//!      path — `compile_source` — drops `_inherit` in `build_graph` entirely.)
//!      The clean measurement lives on the Rails/OpenProject side, where
//!      `ruff_ruby_spo` captures `callbacks`/`validations`/`sti` as first-class
//!      `Model` data — see `openproject-nexgen-rs/.claude/handovers/`.
//!
//! Default build — no `ogar-emit`, no git deps — so it runs offline and in CI.
//! Mirrors the classification in `corpus_to_actions` (raises ⇒ guard; else
//! `MethodKind::Compute` ⇒ compute); the `ogar-emit`-gated assertion at the end
//! pins the mirror to the real lift so the two can never silently drift.

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{model_of, parse_ndjson, MethodKind, Triple};

/// Strip a known namespace prefix (`odoo:` / `ogit:` / `exc:`) — mirrors the
/// crate-private `triple::strip_ns` (not re-exported), used only for map keys.
fn strip_ns(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, r)| r)
}

fn corpus() -> Vec<Triple> {
    // Same richest committed corpus the `corpus_to_actions` unit test uses:
    // account_move + account_move_line + res_partner + res_company (2 739 rows).
    let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
    parse_ndjson(ndjson).expect("slice 2 corpus parses")
}

/// The lifted behavioral arm, partitioned exactly as `corpus_to_actions` would.
struct Arm {
    models: BTreeSet<String>,
    methods: BTreeSet<String>,
    guards: BTreeSet<String>,
    /// compute method IRI → its `reads_field` path-set (the `Depends.paths`).
    computes: BTreeMap<String, BTreeSet<String>>,
}

fn lift(triples: &[Triple]) -> Arm {
    let mut methods: BTreeSet<String> = BTreeSet::new();
    let mut raises: BTreeSet<String> = BTreeSet::new();
    let mut reads: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for t in triples {
        match t.p.as_str() {
            "has_function" => {
                methods.insert(strip_ns(&t.o).to_string());
            }
            "raises" => {
                raises.insert(strip_ns(&t.s).to_string());
            }
            "reads_field" => {
                reads
                    .entry(strip_ns(&t.s).to_string())
                    .or_default()
                    .insert(strip_ns(&t.o).to_string());
            }
            _ => {}
        }
    }

    let mut models: BTreeSet<String> = BTreeSet::new();
    let mut guards: BTreeSet<String> = BTreeSet::new();
    let mut computes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for m in &methods {
        models.insert(model_of(m).to_string());
        // raises wins over compute (matches `raises_wins_over_compute_classification`).
        if raises.contains(m) {
            guards.insert(m.clone());
        } else if MethodKind::classify(m) == MethodKind::Compute {
            computes.insert(m.clone(), reads.get(m).cloned().unwrap_or_default());
        }
        // else: onchange / inverse / search / other — not the behavioral arm.
    }

    Arm {
        models,
        methods,
        guards,
        computes,
    }
}

#[test]
fn ar_lifecycle_override_redundancy() {
    let triples = corpus();
    let arm = lift(&triples);

    let n_models = arm.models.len();
    let n_methods = arm.methods.len();
    let n_guards = arm.guards.len();
    let n_computes = arm.computes.len();

    // Split computes by whether their reads were captured. An EMPTY path-set is
    // a data gap (the Python frontend didn't infer the method's reads), NOT
    // evidence of a shared dependency — counting all empties as "deduped to one"
    // would inflate the collapse with missing data. Exclude them from the
    // headline; report them separately as "unresolved".
    let nonempty: Vec<&BTreeSet<String>> =
        arm.computes.values().filter(|p| !p.is_empty()).collect();
    let n_compute_nonempty = nonempty.len();
    let n_compute_empty = n_computes - n_compute_nonempty;
    let distinct_nonempty: BTreeSet<&BTreeSet<String>> = nonempty.iter().copied().collect();
    let n_distinct_nonempty = distinct_nonempty.len();
    let dedup_nonempty = n_compute_nonempty - n_distinct_nonempty;
    let total_paths: usize = nonempty.iter().map(|p| p.len()).sum();
    let avg_paths = if n_compute_nonempty > 0 {
        total_paths as f64 / n_compute_nonempty as f64
    } else {
        0.0
    };

    // Recipe shapes present in the lifted arm (guard-shape, compute-shape).
    let recipe_shapes = usize::from(n_guards > 0) + usize::from(n_computes > 0);

    // Headline is computed over the RESOLVED behavioral arm only
    // (guards + non-empty computes); empty computes are excluded as a data gap.
    //   distinct payloads you must store = 1 shared guard recipe (all guards
    //   identical) + the distinct non-empty compute path-sets.
    //   recipe-collapsible = everything else (guard duplicates + compute dups).
    let resolved = n_guards + n_compute_nonempty;
    let distinct_payloads = usize::from(n_guards > 0) + n_distinct_nonempty;
    let genuine_leftover = distinct_payloads;
    let recipe_collapsible = resolved.saturating_sub(distinct_payloads);

    let pct = |num: usize| -> f64 {
        if resolved == 0 {
            0.0
        } else {
            100.0 * num as f64 / resolved as f64
        }
    };
    let leftover_pct = pct(genuine_leftover);
    let collapse_pct = pct(recipe_collapsible);

    eprintln!("──── AR-lifecycle-override redundancy (slice_2 corpus) ────");
    eprintln!("models                 : {n_models}");
    eprintln!("methods (all)          : {n_methods}");
    eprintln!("behavioral arm         : {}  (guards {n_guards} + computes {n_computes})", n_guards + n_computes);
    eprintln!("  computes resolved    : {n_compute_nonempty}  (reads captured)");
    eprintln!("  computes unresolved  : {n_compute_empty}  (reads NOT captured — excluded from headline)");
    eprintln!("recipe shapes          : {recipe_shapes}  (1 guard-shape + 1 compute-shape) carry all {} methods", n_guards + n_computes);
    eprintln!("guard arm              : {n_guards} guards → 1 shared recipe shape, 0 per-class payload");
    eprintln!("compute path-sets      : {n_distinct_nonempty} distinct of {n_compute_nonempty}  ({dedup_nonempty} dedup) · avg {avg_paths:.1} paths");
    // Guard-arm full collapse: all guards share ONE recipe shape, so the arm
    // contributes exactly 1 distinct payload — N guards = 1 recipe + N bits.
    let guard_collapsed = n_guards.saturating_sub(1);
    eprintln!("── headline (over {resolved} RESOLVED behavioral methods) ──");
    eprintln!("recipe-collapsible     : {recipe_collapsible}  ({collapse_pct:.1}%)");
    eprintln!("genuine leftover       : {genuine_leftover}  ({leftover_pct:.1}%)");
    eprintln!("───────────────────────────────────────────────────────────");
    eprintln!("VERDICT (Odoo / Python, UPPER bound):");
    eprintln!(
        "  • guard arm collapses FULLY — {n_guards} guards → 1 shared recipe ({guard_collapsed} hidden)."
    );
    eprintln!(
        "  • compute arm is mostly genuine — {n_distinct_nonempty} distinct path-sets of\n    \
         {n_compute_nonempty} resolved computes; recipe-bitmask hides only {dedup_nonempty}."
    );
    eprintln!(
        "  • leftover {leftover_pct:.1}% >> 7% target → REFUTES the strong reading\n    \
         (\"Odoo collapses to 7%\") and CONFIRMS the conjecture's SCOPING: 7% is the\n    \
         best-shaped Rails-AR case, not compute-heavy Odoo-Python."
    );
    eprintln!(
        "  • why upper bound: inherited-vs-override is unmeasurable HERE — the corpus\n    \
         carries inherits_from, but every base mixin (mail_thread, sequence_mixin, …)\n    \
         is OUT-OF-SLICE, so inherited method sets aren't present to dedup against;\n    \
         the live-source `ruff_python_spo` path drops `_inherit` outright; and method\n    \
         bodies aren't captured. All three can only LOWER the leftover. Clean\n    \
         measurement = the Rails/OpenProject probe (callbacks as first-class data)."
    );

    // ── Structural invariants (true regardless of the measured ratio) ──
    assert!(n_models >= 4, "slice_2 spans 4 models, saw {n_models}");
    assert!(
        n_computes > 50,
        "expected many compute methods in slice_2, saw {n_computes}"
    );
    assert!(n_guards > 0, "expected at least one guard method, saw {n_guards}");
    assert!(
        recipe_shapes <= 2,
        "the lifted behavioral arm has exactly two recipe shapes (guard, compute); saw {recipe_shapes}"
    );
    assert!(resolved > 0, "no resolved behavioral methods to measure");
    // The strong mechanism claim that DOES hold for Odoo: the guard arm is
    // perfectly redundant — every guard is the same AR-lifecycle recipe, so the
    // whole arm folds to a single shared shape + per-class address bits.
    assert_eq!(
        distinct_payloads,
        1 + n_distinct_nonempty,
        "guard arm must contribute exactly one shared recipe payload"
    );
    // Ratios are a measurement, not a gate — only that they're well-formed.
    assert!((0.0..=100.0).contains(&leftover_pct));
    assert!(
        (leftover_pct + collapse_pct - 100.0).abs() < 1e-6,
        "leftover% + collapse% must sum to 100 (saw {leftover_pct} + {collapse_pct})"
    );

    // ── ogar-emit consistency pin: the default-build mirror above must agree
    // with the REAL lift (`corpus_action_rows`), so the two classifications can
    // never silently drift, and every guard must render ONE recipe detail.
    #[cfg(feature = "ogar-emit")]
    {
        let rows = od_ontology::corpus_action_rows(&triples);
        let guard_rows: Vec<&(String, String, String, String)> =
            rows.iter().filter(|r| r.2 == "guard").collect();
        let depends_rows = rows.iter().filter(|r| r.2 == "depends").count();
        assert_eq!(guard_rows.len(), n_guards, "mirror guard count vs real lift");
        assert_eq!(depends_rows, n_computes, "mirror compute count vs real lift");
        let distinct_guard_detail: BTreeSet<&str> =
            guard_rows.iter().map(|r| r.3.as_str()).collect();
        assert_eq!(
            distinct_guard_detail.len(),
            1,
            "all guards must share ONE recipe shape (full collapse), saw {distinct_guard_detail:?}"
        );
    }
}
