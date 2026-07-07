//! Corpus schema regression — pins the **invariants** of the SPO triple
//! corpus's shape, not specific counts. The counts drift across corpus
//! revisions (lance-graph PR #523 added 935 deep `reads_field` lifts, #526
//! added 166 `inherits_from` + 247 `validation_kind`, …); the invariants
//! below do not.
//!
//! This is the consumer-side pin on the predicate schema documented in
//! `triple.rs` (pulled from `lance_graph::graph::spo::odoo_ontology` per
//! `specs/REPATRIATION-FRAME.md` Phase 1). It catches:
//!
//! - a NEW predicate appearing without being added to the documented set (the
//!   13-predicate vocabulary is the contract; a 14th is a schema drift event);
//! - a truth value falling outside the three documented provenance bands;
//! - structural inconsistencies (`inherits_from` self-loops, `reads_field`
//!   self-loops, `target` carrying an unexpected `odoo:` prefix).
//!
//! Runs against BOTH shipped corpora (`account_move.spo.ndjson` slice-1 and
//! `slice_2.spo.ndjson` — every invariant must hold on both).

use od_ontology::{parse_ndjson, Triple};

const SLICE_1: &str = include_str!("../../../data/account_move.spo.ndjson");
const SLICE_2: &str = include_str!("../../../data/slice_2.spo.ndjson");

/// The 13 predicates documented in `triple.rs`. Any predicate appearing in
/// the corpus that is NOT in this list is a schema-drift event the schema
/// docs must absorb first (or the harvester must stop emitting).
const DOCUMENTED_PREDICATES: &[&str] = &[
    "rdf:type",
    "has_function",
    "emitted_by",
    "depends_on",
    "reads_field",
    "raises",
    "traverses_relation",
    "target",
    "inverse_name",
    "inherits_from",
    "validation_kind",
    "selection_value",
];

/// Tolerance for the truth-value provenance bands. NARS values are stored as
/// `f32`; we compare with a generous epsilon so a future re-encoding doesn't
/// flap the band classification.
const EPS: f32 = 1e-4;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < EPS
}

/// Classify a `(f, c)` pair into one of the four documented provenance bands
/// (see `triple.rs` § "Truth-value provenance bands"). Returns `None` for
/// unrecognized bands — the test fails on `None`.
fn provenance_band(t: &Triple) -> Option<&'static str> {
    if close(t.f, 1.0) && close(t.c, 1.0) {
        Some("structural-declaration")
    } else if close(t.f, 1.0) && close(t.c, 0.95) {
        Some("structural-membership")
    } else if close(t.f, 0.95) && close(t.c, 0.90) {
        Some("decorator/authoritative")
    } else if close(t.f, 0.85) && close(t.c, 0.75) {
        Some("body-inferred")
    } else {
        None
    }
}

fn for_each_corpus(check: impl Fn(&str, &[Triple])) {
    let s1 = parse_ndjson(SLICE_1).expect("slice-1 parses");
    let s2 = parse_ndjson(SLICE_2).expect("slice-2 parses");
    check("slice-1 (account_move)", &s1);
    check("slice-2", &s2);
}

#[test]
fn every_predicate_is_in_the_documented_set() {
    for_each_corpus(|name, triples| {
        let mut undocumented: Vec<&str> = triples
            .iter()
            .map(|t| t.p.as_str())
            .filter(|p| !DOCUMENTED_PREDICATES.contains(p))
            .collect();
        undocumented.sort_unstable();
        undocumented.dedup();
        assert!(
            undocumented.is_empty(),
            "{name}: undocumented predicates appeared in the corpus — the \
             schema docs must absorb them OR the harvester must stop emitting \
             them: {undocumented:?}"
        );
    });
}

#[test]
fn every_triples_truth_falls_in_a_documented_provenance_band() {
    for_each_corpus(|name, triples| {
        let bad: Vec<&Triple> = triples
            .iter()
            .filter(|t| provenance_band(t).is_none())
            .collect();
        assert!(
            bad.is_empty(),
            "{name}: {} triple(s) carry a truth value outside the three \
             documented bands (structural 1.0/1.0, decorator 0.95/0.9, \
             body-inferred 0.85/0.75); first offender: {:?}",
            bad.len(),
            bad.first()
        );
    });
}

#[test]
fn structural_bands_carry_only_structural_predicates() {
    // The two structural bands are `rdf:type` (declaration, 1.0/1.0) and
    // `has_function` (membership, 1.0/0.95). Nothing else should land there.
    for_each_corpus(|name, triples| {
        for t in triples {
            match provenance_band(t) {
                Some("structural-declaration") => assert_eq!(
                    t.p, "rdf:type",
                    "{name}: predicate `{}` carries the structural-declaration \
                     band (1.0/1.0) but only `rdf:type` should",
                    t.p
                ),
                Some("structural-membership") => assert_eq!(
                    t.p, "has_function",
                    "{name}: predicate `{}` carries the structural-membership \
                     band (1.0/0.95) but only `has_function` should",
                    t.p
                ),
                _ => {}
            }
        }
    });
}

#[test]
fn inherits_from_is_never_a_self_loop() {
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "inherits_from") {
            assert_ne!(
                t.s, t.o,
                "{name}: inherits_from self-loop on {} — the Odoo \
                 extend-in-place idiom must be dropped at scan time",
                t.s
            );
        }
    });
}

#[test]
fn reads_field_is_never_a_self_loop() {
    // A method reading a field it itself emits is dropped — it's a recompute
    // recursion artifact, not a real ordering constraint (mirrors the
    // documented `recompute_dag` guard).
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "reads_field") {
            assert_ne!(
                t.s, t.o,
                "{name}: reads_field self-loop ({} → {})",
                t.s, t.o
            );
        }
    });
}

#[test]
fn inherits_from_base_is_well_formed_iri_even_when_extern_to_slice() {
    // The slice corpora reference bases that resolve OUTSIDE the slice
    // ("externs" in the compiler-AST view — `mail_activity_mixin`,
    // `mail_thread`, …): account_move mixes them in but the slice carries
    // only account_move + its direct relatives, not the mixin defs. That's
    // AST-valid — the compiler's symbol table (OGAR `ClassView`) resolves
    // them, not the slice. So the slice-aware invariant is **well-formedness**
    // (every base is a properly-namespaced `odoo:<name>` IRI), not
    // declared-in-this-unit.
    //
    // The full-corpus integrity claim ("every inherits_from base is itself
    // a declared ObjectType in the full corpus") lives in the upstream
    // extractor's own tests, not here.
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "inherits_from") {
            assert!(
                t.o.starts_with("odoo:"),
                "{name}: inherits_from base `{}` is not a properly-namespaced \
                 `odoo:<name>` IRI",
                t.o
            );
            assert!(
                !t.o["odoo:".len()..].is_empty(),
                "{name}: inherits_from base `{}` has empty local name",
                t.o
            );
        }
    });
}

#[test]
fn target_object_is_raw_comodel_not_namespaced_iri() {
    // `target` carries the raw comodel name from the Odoo source
    // (`"account.move.line"`, sometimes a single word like `"website"`),
    // NOT a namespaced IRI (`"odoo:account_move_line"`). This is the
    // documented shape (ruff#18 sibling-triple convention) and the resolver
    // in `RelationMap::from_corpus` depends on it.
    //
    // Single-word comodels DO occur in real Odoo (`fields.Many2one('website')`
    // is shorthand for `website.website` — Odoo accepts both forms), so the
    // invariant is "no `odoo:` prefix," not "must contain a dot."
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "target") {
            assert!(
                !t.o.starts_with("odoo:"),
                "{name}: target object `{}` carries an odoo: prefix but \
                 should be the raw comodel name",
                t.o
            );
            assert!(
                !t.o.is_empty(),
                "{name}: target object is empty for subject `{}`",
                t.s
            );
        }
    });
}

#[test]
fn validation_kind_object_is_in_the_recognised_set() {
    // Per `triple.rs` doc / lance-graph #526: an `@api.constrains` method is
    // classified by AST pattern into one of five recognised kinds.
    const KINDS: &[&str] = &["presence", "uniqueness", "range", "format", "lookup"];
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "validation_kind") {
            assert!(
                KINDS.contains(&t.o.as_str()),
                "{name}: validation_kind `{}` is not in the recognised set \
                 (presence/uniqueness/range/format/lookup)",
                t.o
            );
        }
    });
}

#[test]
fn raises_object_is_namespaced_exception() {
    // `raises` objects are exception types in the `exc:` namespace
    // (`exc:ValidationError`, `exc:UserError`, …). The shape is fixed.
    for_each_corpus(|name, triples| {
        for t in triples.iter().filter(|t| t.p == "raises") {
            assert!(
                t.o.starts_with("exc:"),
                "{name}: raises object `{}` is not in the `exc:` namespace",
                t.o
            );
        }
    });
}

/// **Q1 migration fuse (2026-07-07)** — `parse_ndjson` is now the canonical
/// `ruff_spo_triplet::from_ndjson`, which fail-loud-rejects any predicate
/// outside the shared closed vocabulary. The retired local `parse_ndjson`
/// validated only the JSON *field* shape, so a predicate typo (`depend_on`)
/// used to parse into a `Triple` and vanish silently from downstream
/// `depends_on` queries. This pins the upgrade: the typo must now be an error.
#[test]
fn unknown_predicate_now_fails_loud_after_upstream_consumption() {
    let typo = r#"{"s":"odoo:a.b","p":"depend_on","o":"odoo:a.c","f":0.95,"c":0.9}"#;
    let err = parse_ndjson(typo).expect_err(
        "an out-of-vocabulary predicate must fail loud now that parse_ndjson is \
         the closed-vocab ruff_spo_triplet::from_ndjson",
    );
    assert_eq!(err.line, 1, "the fail-loud error names the offending 1-based line");
}

/// Companion to the fuse above: every predicate the SHIPPED Odoo corpora emit
/// is understood by the shared closed vocabulary — otherwise `parse_ndjson`
/// (fail-loud) would already have rejected the corpus in `for_each_corpus`.
/// This makes the cross-repo contract explicit rather than incidental: if a
/// future corpus regen adds a predicate upstream doesn't know (e.g. the still-
/// deferred `selection_value`), THIS test's `parse_ndjson` calls fail with a
/// named predicate, pointing straight at the ruff-side `Predicate` gap.
#[test]
fn shipped_corpus_predicates_are_all_in_the_shared_vocabulary() {
    // Reaching here at all means both corpora parsed under the closed-vocab
    // validator; assert we actually exercised a non-trivial predicate set so
    // the guarantee isn't vacuous on an empty corpus.
    for_each_corpus(|name, triples| {
        let mut preds: Vec<&str> = triples.iter().map(|t| t.p.as_str()).collect();
        preds.sort_unstable();
        preds.dedup();
        assert!(
            preds.len() >= 5,
            "{name}: expected a rich predicate set (got {preds:?}); a near-empty \
             set would make the shared-vocabulary guarantee vacuous"
        );
    });
}

/// One concrete data pin — the canonical convergence pin for the deep-read
/// enrichment. This is the load-bearing case the wishlist P0 closed on, and
/// it documents WHAT the corpus actually carries for any future session
/// reading this test as a fixture.
#[test]
fn account_move_compute_amount_carries_deep_cross_model_read_in_slice_2() {
    let triples = parse_ndjson(SLICE_2).expect("slice-2 parses");
    let deep = triples.iter().any(|t| {
        t.s == "odoo:account_move._compute_amount"
            && t.p == "reads_field"
            && t.o == "odoo:account_move_line.amount_residual"
    });
    assert!(
        deep,
        "slice-2: the P0 deep-read enrichment must carry \
         `_compute_amount → account_move_line.amount_residual` (the canonical \
         cross-model recompute-ordering edge)"
    );
}
