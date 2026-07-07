//! **W3.3 behavioral-arm lowering** — Odoo's reactive lifecycle → the OGAR
//! behavioral arm (`ogar_vocab::ActionDef` + `KausalSpec`), the sibling of the
//! structural lowering [`crate::compile_source`] now performs (formerly
//! `schema_to_classes`, deleted with the SurrealQL fork — see
//! `docs/W3.3-DELETE-GATE-MATRIX.md`).
//!
//! This is the lowering the SurrealQL-AST-trap spellbook
//! (`specs/SURREAL-AST-TRAP.md`) and the W3 scope
//! (`specs/W3-BEHAVIORAL-ARM-SCOPE.md`) point at: Odoo's behavior is *not* DDL
//! (`DEFINE FUNCTION`/`DEFINE EVENT`); it is `ActionDef`, the value a classid
//! resolves to. The corpus carries the facts; this maps them onto the Core
//! vocabulary that `od-ontology` already pins (`ogar-vocab`, no new dep).
//!
//! # Two arms built (the facts the corpus carries)
//!
//! | Odoo source | corpus fact | → `ActionDef` |
//! |---|---|---|
//! | `_compute_*` + `@api.depends` | `(method, reads_field, field)` | `kausal: Depends { paths }`, `temporal: OnCommit`, `subject: Trigger` |
//! | `@api.constrains` guard | `(method, raises, exc:…)` | `kausal: LifecycleTrigger{"before_save"}`, `guard_failure_policy: Reject` |
//!
//! # One arm deferred (a producer-side gap, NOT a Core gap)
//!
//! The `action_*` **state crossing** (`action_post` sets `state := "posted"`
//! → [`ogar_vocab::EnterEffect`]) needs an explicit transition fact the corpus
//! does **not** carry today (there is no `writes_state` / `transitions_to`
//! predicate — only `emitted_by`/`reads_field`/`depends_on`). That is a
//! `lance-graph` odoo-spo producer ask, filed in `specs/UPSTREAM_WISHLIST.md`,
//! not something to synthesize here.

use std::collections::{BTreeMap, BTreeSet};

use ogar_vocab::{ActionDef, ActionSubject, GuardFailurePolicy, KausalSpec, TemporalSpec};

use crate::recompute_dag::MethodKind;
use crate::triple::{member_of, model_of, strip_ns, Triple};

/// Lower the SPO corpus's behavioral facts onto canonical
/// [`ogar_vocab::ActionDef`]s — one per `_compute_*` method (recompute arm)
/// and one per guard method that `raises` (constrains arm). Deterministic
/// order (sorted by method identity).
///
/// Methods that are neither (`_onchange_*` cooperative UI loops, `_inverse_*`
/// write-backs, `_search_*`, plain helpers) are intentionally not lowered —
/// they are not the reactive/guard behavioral arm. The `action_*` state-
/// crossing arm is deferred (see the module docs — a producer-side fact gap).
#[deprecated(since = "0.5.0", note = "the DO-arm lives in OGAR (operator ruling 2026-07-06): actions arrive on `CompiledClass.actions` via `compile_source`; this corpus-side pipeline is retained as (1) the kausal-parity witness and (2) the implementation behind `od-codegen --actions` (which reads ndjson corpora, not source) — do NOT remove until that CLI mode is migrated or retired")]
#[must_use]
pub fn corpus_to_actions(triples: &[Triple]) -> Vec<ActionDef> {
    let mut methods: BTreeSet<&str> = BTreeSet::new();
    let mut reads: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut raises: BTreeSet<&str> = BTreeSet::new();

    for t in triples {
        match t.p.as_str() {
            "has_function" => {
                methods.insert(strip_ns(&t.o));
            }
            "reads_field" => {
                reads
                    .entry(strip_ns(&t.s))
                    .or_default()
                    .insert(strip_ns(&t.o).to_string());
            }
            "raises" => {
                raises.insert(strip_ns(&t.s));
            }
            _ => {}
        }
    }

    let mut out: Vec<ActionDef> = Vec::new();
    for &method in &methods {
        let model = model_of(method);
        let member = member_of(method).unwrap_or(method);

        // `ActionDef` is `#[non_exhaustive]` — build via Default + field set.
        let mut def = ActionDef::default();
        def.identity = format!("odoo/{model}::action_def::{member}");
        def.predicate = member.to_string();
        def.object_class = model.to_string();

        if raises.contains(method) {
            // Guard arm: an `@api.constrains` validator that raises. Fires as a
            // lifecycle event; a raise is a hard reject (Pending → Failed).
            def.kausal = Some(KausalSpec::LifecycleTrigger {
                event: "before_save".to_string(),
            });
            def.guard_failure_policy = Some(GuardFailurePolicy::Reject);
            def.decorators = vec!["api.constrains".to_string()];
            out.push(def);
        } else if MethodKind::classify(method) == MethodKind::Compute {
            // Recompute arm: `@api.depends` reactive compute. The dependency
            // paths are the method's `reads_field` objects (the lifted
            // `@api.depends`, per lance-graph #523). Odoo materializes stored
            // computes on write/commit → OnCommit, reactively → Trigger.
            let paths: Vec<String> = reads
                .get(method)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            def.kausal = Some(KausalSpec::Depends { paths });
            def.default_temporal = TemporalSpec::OnCommit;
            def.default_subject = ActionSubject::Trigger;
            def.decorators = vec!["api.depends".to_string()];
            out.push(def);
        }
        // else: Onchange / Inverse / Search / Other — not the behavioral arm.
    }
    out
}

/// Inspection-friendly rows over [`corpus_to_actions`] — one
/// `(object_class, predicate, kind, detail)` per `ActionDef`, where `kind` is
/// `"depends"` or `"guard"` and `detail` is the joined dependency paths or the
/// `"event (policy)"` of the guard. Keeps the `ogar_vocab` enum match *inside*
/// this crate so a consumer (the `od-codegen --actions` CLI) can print the
/// lowering without depending on `ogar-vocab` or matching its
/// `#[non_exhaustive]` enums. Order mirrors [`corpus_to_actions`].
#[allow(deprecated)] // internally rides corpus_to_actions, deprecated together
#[deprecated(since = "0.5.0", note = "the DO-arm lives in OGAR (operator ruling 2026-07-06): actions arrive on `CompiledClass.actions` via `compile_source`; this corpus-side pipeline is retained as (1) the kausal-parity witness and (2) the implementation behind `od-codegen --actions` (which reads ndjson corpora, not source) — do NOT remove until that CLI mode is migrated or retired")]
#[must_use]
pub fn corpus_action_rows(triples: &[Triple]) -> Vec<(String, String, String, String)> {
    corpus_to_actions(triples)
        .into_iter()
        .map(|a| {
            let (kind, detail) = match &a.kausal {
                Some(KausalSpec::Depends { paths }) => ("depends".to_string(), paths.join(", ")),
                Some(KausalSpec::LifecycleTrigger { event }) => {
                    let policy = match a.guard_failure_policy {
                        Some(GuardFailurePolicy::Reject) => "reject",
                        Some(GuardFailurePolicy::Postponable) => "postpone",
                        _ => "?",
                    };
                    ("guard".to_string(), format!("{event} ({policy})"))
                }
                _ => ("other".to_string(), String::new()),
            };
            (a.object_class, a.predicate, kind, detail)
        })
        .collect()
}

#[cfg(test)]
// The module under test IS the deprecated corpus witness — its tests
// legitimately exercise the deprecated surface (that is their whole job).
#[allow(deprecated)]
mod tests {
    use super::*;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            s: s.into(),
            p: p.into(),
            o: o.into(),
            f: 0.9,
            c: 0.9,
        }
    }

    #[test]
    fn compute_method_lowers_to_depends_action() {
        let triples = vec![
            t(
                "odoo:account_move",
                "has_function",
                "odoo:account_move._compute_amount",
            ),
            t(
                "odoo:account_move._compute_amount",
                "reads_field",
                "odoo:account_move.line_ids",
            ),
            t(
                "odoo:account_move._compute_amount",
                "reads_field",
                "odoo:account_move_line.amount_residual",
            ),
        ];
        let actions = corpus_to_actions(&triples);
        assert_eq!(actions.len(), 1);
        let a = &actions[0];
        assert_eq!(a.predicate, "_compute_amount");
        assert_eq!(a.object_class, "account_move");
        assert_eq!(a.default_temporal, TemporalSpec::OnCommit);
        assert_eq!(a.default_subject, ActionSubject::Trigger);
        match &a.kausal {
            Some(KausalSpec::Depends { paths }) => {
                assert!(paths.contains(&"account_move.line_ids".to_string()));
                assert!(paths.contains(&"account_move_line.amount_residual".to_string()));
            }
            other => panic!("expected Depends, got {other:?}"),
        }
        assert!(a.guard_failure_policy.is_none());
    }

    #[test]
    fn raising_guard_method_lowers_to_lifecycle_reject() {
        let triples = vec![
            t(
                "odoo:account_move",
                "has_function",
                "odoo:account_move._check_balanced",
            ),
            t(
                "odoo:account_move._check_balanced",
                "raises",
                "exc:ValidationError",
            ),
        ];
        let actions = corpus_to_actions(&triples);
        assert_eq!(actions.len(), 1);
        let a = &actions[0];
        assert_eq!(a.predicate, "_check_balanced");
        assert_eq!(
            a.kausal,
            Some(KausalSpec::LifecycleTrigger {
                event: "before_save".to_string()
            })
        );
        assert_eq!(a.guard_failure_policy, Some(GuardFailurePolicy::Reject));
        assert_eq!(a.decorators, vec!["api.constrains".to_string()]);
    }

    #[test]
    fn raises_wins_over_compute_classification() {
        // A `_compute_*`-named method that also raises is a guard, not a
        // recompute — the `raises` fact is the stronger signal.
        let triples = vec![
            t("odoo:m", "has_function", "odoo:m._compute_guarded"),
            t("odoo:m._compute_guarded", "raises", "exc:ValidationError"),
            t("odoo:m._compute_guarded", "reads_field", "odoo:m.x"),
        ];
        let actions = corpus_to_actions(&triples);
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0].kausal,
            Some(KausalSpec::LifecycleTrigger { .. })
        ));
    }

    #[test]
    fn onchange_inverse_search_other_are_not_lowered() {
        let triples = vec![
            t("odoo:m", "has_function", "odoo:m._onchange_partner"),
            t("odoo:m", "has_function", "odoo:m._inverse_total"),
            t("odoo:m", "has_function", "odoo:m._search_name"),
            t("odoo:m", "has_function", "odoo:m.action_post"),
        ];
        // none raise, none are _compute_* → no behavioral arm
        assert!(corpus_to_actions(&triples).is_empty());
    }

    #[test]
    fn identity_is_stable_and_namespaced() {
        let triples = vec![t(
            "odoo:account_move",
            "has_function",
            "odoo:account_move._compute_amount",
        )];
        let actions = corpus_to_actions(&triples);
        assert_eq!(
            actions[0].identity,
            "odoo/account_move::action_def::_compute_amount"
        );
    }

    #[test]
    fn slice_2_corpus_yields_both_arms() {
        let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
        let triples = crate::triple::parse_ndjson(ndjson).expect("slice 2 parses");
        let actions = corpus_to_actions(&triples);
        let depends = actions
            .iter()
            .filter(|a| matches!(a.kausal, Some(KausalSpec::Depends { .. })))
            .count();
        let guards = actions
            .iter()
            .filter(|a| matches!(a.kausal, Some(KausalSpec::LifecycleTrigger { .. })))
            .count();
        eprintln!(
            "slice 2 actions: {} total, {depends} depends, {guards} guards",
            actions.len()
        );
        assert!(
            depends > 50,
            "expected many compute→Depends actions, saw {depends}"
        );
        assert!(
            guards > 0,
            "expected constrains→guard actions, saw {guards}"
        );
        // every guard carries the Reject policy; every Depends carries paths.
        for a in &actions {
            match &a.kausal {
                Some(KausalSpec::Depends { .. }) => {
                    assert_eq!(a.guard_failure_policy, None);
                }
                Some(KausalSpec::LifecycleTrigger { .. }) => {
                    assert_eq!(a.guard_failure_policy, Some(GuardFailurePolicy::Reject));
                }
                other => panic!("unexpected kausal in v1 lowering: {other:?}"),
            }
        }
    }

    #[test]
    fn action_rows_format_depends_and_guard() {
        let triples = vec![
            t("odoo:m", "has_function", "odoo:m._compute_x"),
            t("odoo:m._compute_x", "reads_field", "odoo:m.a"),
            t("odoo:m._compute_x", "reads_field", "odoo:m.b"),
            t("odoo:m", "has_function", "odoo:m._check_y"),
            t("odoo:m._check_y", "raises", "exc:ValidationError"),
        ];
        let rows = corpus_action_rows(&triples);
        // sorted by method identity: _check_y before _compute_x
        assert_eq!(
            rows,
            vec![
                (
                    "m".to_string(),
                    "_check_y".to_string(),
                    "guard".to_string(),
                    "before_save (reject)".to_string()
                ),
                (
                    "m".to_string(),
                    "_compute_x".to_string(),
                    "depends".to_string(),
                    "m.a, m.b".to_string()
                ),
            ]
        );
    }
}
