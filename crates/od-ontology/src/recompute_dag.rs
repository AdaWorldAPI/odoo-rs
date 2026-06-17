//! Recompute-DAG cycle detector — the wishlist P0 corpus-side probe.
//!
//! This module builds a method-level dependency graph from the SPO corpus's
//! `reads_field` + `emitted_by` triples, runs cycle detection, and produces a
//! topological order if the graph is acyclic.
//!
//! # What this catches
//!
//! For every read-emit chain `(method_a, reads_field, field) +
//! (field, emitted_by, method_b)`, an edge `method_b → method_a` is added.
//! Reads from `method_b`'s outputs back into `method_a` (which `method_b`
//! depends on) form a cycle.
//!
//! # What this DOES NOT catch (the upstream wishlist's P0 gap)
//!
//! Cross-record / through-relation dependencies are **invisible** at this layer.
//! Concretely: `account.move._compute_amount` reads `line_ids` (a One2many
//! relation), then indirectly reads `account.move.line.reconciled` /
//! `.amount_residual` via the resulting recordset. The Odoo SPO extractor
//! today emits a single `(_compute_amount, reads_field, account_move.line_ids)`
//! triple, NOT the transitive `(_compute_amount, reads_field,
//! account_move_line.reconciled)` it would need to land the cross-model edge.
//! See `specs/_compute_amount.md` § "MISSED-1 (P0)" — the audit caught the
//! cycle by hand, the corpus alone cannot.
//!
//! See `specs/UPSTREAM_WISHLIST.md` § "P0 · Cross-method recompute-ordering DAG"
//! for the matching ask: deep-`reads_field` enrichment that follows relation
//! traversals would let this module catch the audit's exact P0 finding.
//!
//! # What this proves
//!
//! Running the detector on the slice 2 corpus (`account_move` +
//! `account_move_line` + `res_currency_rate` + `res_partner`) **restricted
//! to `MethodKind::Compute`** yields zero cycles — confirming the limitation
//! above with data. The probe is therefore the CONJECTURE→FINDING gate for
//! the wishlist's P0: the corpus needs extractor enrichment before this
//! primitive can catch cross-model cycles.

use crate::triple::{model_of, strip_ns, Triple};
use std::collections::{BTreeMap, BTreeSet};

/// A method identifier — the `s` or `o` from a `has_function` triple. Stored
/// without the `odoo:` namespace prefix so `account_move._compute_amount` not
/// `odoo:account_move._compute_amount`.
pub type MethodId = String;

/// Method kinds that need different ordering semantics in the projection.
///
/// - `Compute` — `_compute_*` reactive recompute; the projection lowers each
///   to a `DEFINE FUNCTION` invoked by `VALUE` on the emitted fields. **This
///   subset MUST be a DAG** or reactive recompute loops at runtime.
/// - `Check` — `_check_*` `@api.constrains` invariant guards; lower to
///   `DEFINE EVENT WHEN/THEN THROW`. No inter-method ordering — they all fire
///   on save.
/// - `Onchange` — `_onchange_*` UI-trigger handlers; cooperative re-runs at
///   the form-view layer. Cycles are LEGITIMATE here (two onchange handlers
///   can mutually re-trigger via shared write+read on the same field). The
///   projection does not lower these to reactive fields.
/// - `Inverse` — paired with a `_compute_*` for write-back; out-of-band of
///   the read→emit graph.
/// - `Search` — search-domain materialiser, not a recompute method.
/// - `Other` — anything that isn't classified by the underscore-prefix
///   convention (raw action methods, public API, helpers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    /// `_compute_*` — reactive, must be DAG-ordered.
    Compute,
    /// `_check_*` — `@api.constrains` invariant guard.
    Check,
    /// `_onchange_*` — UI trigger handler; cycles legitimate.
    Onchange,
    /// `_inverse_*` — compute write-back partner.
    Inverse,
    /// `_search_*` — search-domain materialiser.
    Search,
    /// Anything else.
    Other,
}

impl MethodKind {
    /// Classify a method IRI by its underscore-prefix convention. Looks at
    /// the member segment only — `account_move._compute_amount` → `Compute`,
    /// `account_move.action_post` → `Other`.
    #[must_use]
    pub fn classify(method_iri: &str) -> Self {
        let local = strip_ns(method_iri);
        let member = local.rsplit_once('.').map_or(local, |(_, m)| m);
        if member.starts_with("_compute_") {
            Self::Compute
        } else if member.starts_with("_check_") {
            Self::Check
        } else if member.starts_with("_onchange_") {
            Self::Onchange
        } else if member.starts_with("_inverse_") {
            Self::Inverse
        } else if member.starts_with("_search_") {
            Self::Search
        } else {
            Self::Other
        }
    }
}

/// The method-level dependency graph derived from a corpus.
///
/// Edges `a → b` mean "method `b` must run after method `a`" (i.e. `b` reads a
/// field that `a` emits). Same-method edges (a method reading a field it
/// itself emits) are dropped during construction — they would be spurious
/// self-loops, not real cycles.
#[derive(Debug, Default, Clone)]
pub struct RecomputeDag {
    /// Adjacency: `a → {b₁, b₂, …}` where each `bᵢ` must run after `a`.
    edges: BTreeMap<MethodId, BTreeSet<MethodId>>,
    /// All known method IRIs (subjects and targets), so isolated methods
    /// still appear in the topological order.
    methods: BTreeSet<MethodId>,
}

impl RecomputeDag {
    /// Build the DAG from a triple slice. Two passes:
    ///
    ///   1. Index `field → emit_method` from `emitted_by` triples.
    ///   2. For each `reads_field` triple, look up the field's emitter and
    ///      add `emit_method → read_method`.
    ///
    /// `reads_field` objects that don't correspond to a `<model>.<field>`
    /// emitted by some method are skipped (the corpus emits framework methods
    /// like `filtered` / `with_context` / `browse` as `reads_field` objects;
    /// these are not real fields and have no emitter).
    #[must_use]
    pub fn from_triples(triples: &[Triple]) -> Self {
        let mut field_emitter: BTreeMap<&str, &str> = BTreeMap::new();
        let mut methods: BTreeSet<MethodId> = BTreeSet::new();

        for t in triples {
            if t.p == "has_function" {
                methods.insert(strip_ns(&t.o).to_string());
            }
            if t.p == "emitted_by" {
                // (field, emitted_by, method)
                field_emitter.insert(strip_ns(&t.s), strip_ns(&t.o));
                methods.insert(strip_ns(&t.o).to_string());
            }
        }

        let mut edges: BTreeMap<MethodId, BTreeSet<MethodId>> = BTreeMap::new();
        for t in triples {
            if t.p != "reads_field" {
                continue;
            }
            let read_method = strip_ns(&t.s);
            let field = strip_ns(&t.o);
            let Some(&emit_method) = field_emitter.get(field) else {
                continue;
            };
            if emit_method == read_method {
                continue;
            }
            edges
                .entry(emit_method.to_string())
                .or_default()
                .insert(read_method.to_string());
            methods.insert(read_method.to_string());
            methods.insert(emit_method.to_string());
        }

        Self { edges, methods }
    }

    /// Number of method nodes in the graph.
    #[must_use]
    pub fn method_count(&self) -> usize {
        self.methods.len()
    }

    /// Number of `(emit_method → read_method)` ordering edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(BTreeSet::len).sum()
    }

    /// Whether `a → b` (b reads what a emits).
    #[must_use]
    pub fn has_edge(&self, a: &str, b: &str) -> bool {
        self.edges.get(a).is_some_and(|set| set.contains(b))
    }

    /// DFS-based cycle detection. Returns the first cycle found as a path of
    /// method IRIs (in traversal order, with the back-edge target repeated at
    /// the end), or `None` if the graph is acyclic.
    #[must_use]
    pub fn detect_cycle(&self) -> Option<Vec<MethodId>> {
        let mut color: BTreeMap<&str, Color> = self
            .methods
            .iter()
            .map(|m| (m.as_str(), Color::White))
            .collect();
        let mut stack: Vec<MethodId> = Vec::new();

        for start in &self.methods {
            if color.get(start.as_str()) != Some(&Color::White) {
                continue;
            }
            if let Some(cycle) = self.dfs_cycle(start.as_str(), &mut color, &mut stack) {
                return Some(cycle);
            }
        }
        None
    }

    fn dfs_cycle<'a>(
        &'a self,
        node: &'a str,
        color: &mut BTreeMap<&'a str, Color>,
        stack: &mut Vec<MethodId>,
    ) -> Option<Vec<MethodId>> {
        color.insert(node, Color::Gray);
        stack.push(node.to_string());

        if let Some(succs) = self.edges.get(node) {
            for succ in succs {
                match color.get(succ.as_str()).copied() {
                    Some(Color::Gray) => {
                        let start = stack.iter().position(|m| m == succ).unwrap_or(0);
                        let mut cycle: Vec<MethodId> = stack[start..].to_vec();
                        cycle.push(succ.clone());
                        return Some(cycle);
                    }
                    Some(Color::White) => {
                        let key = self.methods.get(succ.as_str())?;
                        if let Some(cycle) = self.dfs_cycle(key.as_str(), color, stack) {
                            return Some(cycle);
                        }
                    }
                    _ => {}
                }
            }
        }

        color.insert(node, Color::Black);
        stack.pop();
        None
    }

    /// Kahn's-algorithm topological order. Returns `Err(Vec<MethodId>)` with a
    /// witness cycle path on failure (delegating to [`Self::detect_cycle`]).
    ///
    /// Tie-breaking is stable (`BTreeSet` ordering) so the output is
    /// deterministic across runs.
    ///
    /// # Errors
    ///
    /// Returns a cycle path if the graph is not a DAG.
    pub fn topological_order(&self) -> Result<Vec<MethodId>, Vec<MethodId>> {
        let mut indeg: BTreeMap<&str, usize> =
            self.methods.iter().map(|m| (m.as_str(), 0)).collect();
        for succs in self.edges.values() {
            for s in succs {
                *indeg.entry(s.as_str()).or_insert(0) += 1;
            }
        }
        let mut ready: BTreeSet<&str> = indeg
            .iter()
            .filter_map(|(m, &d)| (d == 0).then_some(*m))
            .collect();
        let mut order: Vec<MethodId> = Vec::with_capacity(self.methods.len());

        while let Some(n) = ready.iter().next().copied() {
            ready.remove(n);
            order.push(n.to_string());
            if let Some(succs) = self.edges.get(n) {
                for s in succs {
                    // INVARIANT: every successor has an indegree entry — both
                    // `indeg` and `edges` are populated only from the same
                    // method set, so the lookup is total.
                    let Some(d) = indeg.get_mut(s.as_str()) else {
                        continue;
                    };
                    *d -= 1;
                    if *d == 0 {
                        ready.insert(s.as_str());
                    }
                }
            }
        }

        if order.len() == self.methods.len() {
            Ok(order)
        } else {
            Err(self.detect_cycle().unwrap_or_default())
        }
    }

    /// The qualifying model of a method IRI (`account_move._compute_amount` →
    /// `account_move`). Convenience for downstream consumers grouping by model.
    #[must_use]
    pub fn method_model(method: &str) -> &str {
        model_of(method)
    }

    /// Restrict the DAG to methods whose kind is in `kinds`. Edges to / from
    /// dropped methods are also dropped. Cheap clone (graphs are O(methods +
    /// edges) and the slice corpora are small).
    ///
    /// The projection only emits **`MethodKind::Compute`** as reactive
    /// `DEFINE FUNCTION` bodies, so `restrict_to(&[Compute])` is the natural
    /// query for "does the projection's reactive subset form a DAG?". Other
    /// kinds (legitimate `Onchange` cooperative loops, fire-on-save `Check`
    /// guards) are out-of-scope for the recompute-DAG invariant.
    #[must_use]
    pub fn restrict_to(&self, kinds: &[MethodKind]) -> Self {
        let keep: BTreeSet<MethodId> = self
            .methods
            .iter()
            .filter(|m| kinds.contains(&MethodKind::classify(m)))
            .cloned()
            .collect();
        let edges: BTreeMap<MethodId, BTreeSet<MethodId>> = self
            .edges
            .iter()
            .filter_map(|(from, tos)| {
                if !keep.contains(from) {
                    return None;
                }
                let kept_tos: BTreeSet<MethodId> = tos
                    .iter()
                    .filter(|t| keep.contains(*t))
                    .cloned()
                    .collect();
                if kept_tos.is_empty() {
                    None
                } else {
                    Some((from.clone(), kept_tos))
                }
            })
            .collect();
        Self {
            edges,
            methods: keep,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Color {
    White,
    Gray,
    Black,
}

#[cfg(test)]
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
    fn empty_corpus_empty_dag() {
        let dag = RecomputeDag::from_triples(&[]);
        assert_eq!(dag.method_count(), 0);
        assert_eq!(dag.edge_count(), 0);
        assert!(dag.detect_cycle().is_none());
        assert_eq!(dag.topological_order(), Ok(vec![]));
    }

    #[test]
    fn simple_linear_chain_orders_emit_before_read() {
        // A emits field f; B reads field f → edge A→B, order [A, B].
        let triples = vec![
            t("odoo:m", "has_function", "odoo:m.a"),
            t("odoo:m", "has_function", "odoo:m.b"),
            t("odoo:m.f", "emitted_by", "odoo:m.a"),
            t("odoo:m.b", "reads_field", "odoo:m.f"),
        ];
        let dag = RecomputeDag::from_triples(&triples);
        assert_eq!(dag.edge_count(), 1);
        assert!(dag.has_edge("m.a", "m.b"));
        let order = dag.topological_order().expect("acyclic");
        let a = order.iter().position(|x| x == "m.a").unwrap();
        let b = order.iter().position(|x| x == "m.b").unwrap();
        assert!(a < b, "emit must come before read: {order:?}");
    }

    #[test]
    fn cycle_is_detected() {
        // A reads g, g emitted by B; B reads f, f emitted by A → cycle.
        let triples = vec![
            t("odoo:m.f", "emitted_by", "odoo:m.a"),
            t("odoo:m.g", "emitted_by", "odoo:m.b"),
            t("odoo:m.a", "reads_field", "odoo:m.g"),
            t("odoo:m.b", "reads_field", "odoo:m.f"),
        ];
        let dag = RecomputeDag::from_triples(&triples);
        let cycle = dag.detect_cycle().expect("cycle expected");
        assert!(cycle.len() >= 2, "cycle path must have at least 2 nodes: {cycle:?}");
        assert!(
            cycle.iter().any(|m| m == "m.a") && cycle.iter().any(|m| m == "m.b"),
            "cycle must mention both methods: {cycle:?}"
        );
        assert!(dag.topological_order().is_err());
    }

    #[test]
    fn self_loop_is_dropped_not_a_cycle() {
        // A method reading a field it itself emits is dropped — it's a
        // recompute-recursion artifact of the extractor (the @api.depends
        // listing the same field), not a true ordering constraint.
        let triples = vec![
            t("odoo:m.f", "emitted_by", "odoo:m.a"),
            t("odoo:m.a", "reads_field", "odoo:m.f"),
        ];
        let dag = RecomputeDag::from_triples(&triples);
        assert_eq!(dag.edge_count(), 0);
        assert!(dag.detect_cycle().is_none());
    }

    #[test]
    fn method_kind_classifies_by_prefix() {
        assert_eq!(
            MethodKind::classify("odoo:account_move._compute_amount"),
            MethodKind::Compute
        );
        assert_eq!(
            MethodKind::classify("odoo:account_move._check_balanced"),
            MethodKind::Check
        );
        assert_eq!(
            MethodKind::classify("odoo:account_move._onchange_partner_id"),
            MethodKind::Onchange
        );
        assert_eq!(
            MethodKind::classify("account_move._inverse_quick_edit_total"),
            MethodKind::Inverse
        );
        assert_eq!(
            MethodKind::classify("account_move._search_name"),
            MethodKind::Search
        );
        assert_eq!(
            MethodKind::classify("account_move.action_post"),
            MethodKind::Other
        );
    }

    #[test]
    fn slice_1_compute_subset_is_acyclic() {
        // FINDING #1: the slice 1 compute-method subset (the only kind that
        // lowers to reactive `DEFINE FUNCTION`) topologically sorts cleanly.
        let ndjson = include_str!("../../../data/account_move.spo.ndjson");
        let triples = crate::triple::parse_ndjson(ndjson).expect("slice 1 parses");
        let full = RecomputeDag::from_triples(&triples);
        let compute = full.restrict_to(&[MethodKind::Compute]);
        let cycle = compute.detect_cycle();
        eprintln!(
            "slice 1 compute subset: methods={} edges={} cycle={:?}",
            compute.method_count(),
            compute.edge_count(),
            cycle
        );
        assert!(
            cycle.is_none(),
            "slice 1 compute subset must be acyclic; cycle: {cycle:?}"
        );
    }

    #[test]
    fn slice_1_full_graph_finds_legitimate_onchange_cycle() {
        // FINDING #2 (NEW): the corpus *does* carry a real cycle, but it is
        // between `_onchange_*` methods — `_onchange_invoice_vendor_bill` and
        // `_onchange_quick_edit_total_amount` both emit AND read
        // `account_move.invoice_line_ids`, forming a cooperative UI loop. This
        // is LEGITIMATE Odoo semantics (the form view manages the re-fire);
        // it is NOT a recompute-DAG violation. Demonstrates *why* restricting
        // to `MethodKind::Compute` is the correct invariant.
        let ndjson = include_str!("../../../data/account_move.spo.ndjson");
        let triples = crate::triple::parse_ndjson(ndjson).expect("slice 1 parses");
        let full = RecomputeDag::from_triples(&triples);
        let cycle = full.detect_cycle().expect("expected onchange loop");
        assert!(
            cycle.iter().any(|m| m.contains("_onchange_")),
            "expected onchange-shaped cycle, got: {cycle:?}"
        );
    }

    #[test]
    fn slice_2_compute_subset_no_cross_model_cycle() {
        // **THE PROBE FINDING — the value-bearing one.**
        //
        // Slice 2 spans account_move + account_move_line +
        // res_currency_rate + res_partner. The audit's MISSED-1 cycle
        // (move._compute_amount → line.reconciled →
        // line._compute_amount_residual → … → back to move._compute_amount)
        // exists in the actual Python (a hand-found P0 in
        // `specs/_compute_amount.md`).
        //
        // This test asserts the cycle is **INVISIBLE** to the corpus-only
        // compute-DAG: `_compute_amount`'s `reads_field` for `line_ids` is a
        // relation traversal, and the extractor does NOT lift the transitive
        // `line.reconciled` read out into a separate triple.
        //
        // **Promotes the wishlist's P0 ask from CONJECTURE to FINDING:**
        // catching the audit's MISSED-1 needs extractor enrichment
        // (deep-`reads_field` that follows relation traversals and lifts the
        // `@api.depends` leaf), NOT ClassView design. Lowest-cost path
        // forward is the same shape as the wishlist's P1 FK-target-override
        // ask: one new triple per leaf relation-traversal-read.
        let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
        let triples = crate::triple::parse_ndjson(ndjson).expect("slice 2 parses");
        let full = RecomputeDag::from_triples(&triples);
        let compute = full.restrict_to(&[MethodKind::Compute]);
        let cycle = compute.detect_cycle();
        eprintln!(
            "slice 2 compute subset: methods={} edges={} cycle={:?}",
            compute.method_count(),
            compute.edge_count(),
            cycle
        );
        assert!(
            cycle.is_none(),
            "slice 2 compute subset is acyclic only under surface-`reads_field`; \
             cycle would appear if extractor lifts cross-record reads"
        );

        // Sanity: the methods that *would* form the cycle ARE in the graph,
        // proving the limitation is in EDGES, not method coverage.
        assert!(
            compute.method_count() > 10,
            "slice 2 should yield many compute methods (saw {})",
            compute.method_count()
        );
    }
}
