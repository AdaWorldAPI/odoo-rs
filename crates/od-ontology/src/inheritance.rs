//! Inheritance map — `(model → bases)` lifted from the corpus's `inherits_from`
//! triples shipped by lance-graph#526.
//!
//! # Wire shape
//!
//! Per ruff#19's cross-language inheritance convention (`(class, inherits_from,
//! <base>)`, shared by C++ + Rails + Odoo frontends):
//!
//! ```text
//! (odoo:account_move, inherits_from, odoo:mail_thread)
//! (odoo:account_move, inherits_from, odoo:mail_activity_mixin)
//! (odoo:account_move, inherits_from, odoo:portal_mixin)
//! (odoo:account_move, inherits_from, odoo:sequence_mixin)
//! ```
//!
//! Both `_inherit` (mixin composition) and `_inherits` (delegation) lift to
//! the same predicate per the upstream extractor's convention — the delegation
//! FK itself is captured separately via `target` / `inverse_name`.
//!
//! # Scope
//!
//! This is the corpus-side **direct bases** lookup, NOT a `ClassView` MRO
//! resolver. Transitive walks / `virtually_overrides` precedence are the
//! wishlist P2 ask that genuinely needs the lance-graph ClassView design
//! session. Direct-bases is enough to surface, for any model, "which mixins'
//! `DEFINE FIELD` / `DEFINE FUNCTION` / `DEFINE EVENT` should be unioned
//! into this model's table" — the immediate consumer the wishlist names.

use crate::triple::{strip_ns, Triple};
use std::collections::{BTreeMap, BTreeSet};

/// A lookup from underscored model name (`account_move`) to its declared
/// `_inherit` / `_inherits` bases (also underscored: `mail_thread`,
/// `mail_activity_mixin`).
#[derive(Debug, Clone, Default)]
pub struct InheritanceMap {
    bases: BTreeMap<String, BTreeSet<String>>,
}

impl InheritanceMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build directly from the SPO triple slice, consuming `inherits_from`
    /// edges. Triples whose subject or object is not an `odoo:<model>` IRI
    /// (no member dot, namespaced under `odoo:`) are skipped silently.
    ///
    /// Both endpoints are namespace-stripped: `odoo:account_move` →
    /// `account_move` for both subject and object.
    #[must_use]
    pub fn from_corpus(triples: &[Triple]) -> Self {
        let mut bases: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for t in triples {
            if t.p != "inherits_from" {
                continue;
            }
            let Some(child) = bare_model(&t.s) else {
                continue;
            };
            let Some(base) = bare_model(&t.o) else {
                continue;
            };
            bases.entry(child.to_string()).or_default().insert(base.to_string());
        }
        Self { bases }
    }

    /// The direct bases of `model`, sorted (BTreeSet ordering). Empty when
    /// the model has no `_inherit` declarations or isn't in the corpus.
    #[must_use]
    pub fn bases(&self, model: &str) -> Vec<&str> {
        self.bases
            .get(model)
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Whether `model` has any declared bases.
    #[must_use]
    pub fn has_bases(&self, model: &str) -> bool {
        self.bases.get(model).is_some_and(|s| !s.is_empty())
    }

    /// Total number of models with at least one declared base.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bases.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }
}

/// `odoo:<model>` → `Some("model")`. Bare-model IRIs only (no member dot);
/// `odoo:<model>.<field>` returns `None` — fields are not models.
fn bare_model(iri: &str) -> Option<&str> {
    let local = strip_ns(iri);
    if local.contains('.') {
        None
    } else {
        Some(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            s: s.into(),
            p: p.into(),
            o: o.into(),
            f: 0.95,
            c: 0.9,
        }
    }

    #[test]
    fn empty_corpus_empty_map() {
        let m = InheritanceMap::from_corpus(&[]);
        assert!(m.is_empty());
        assert_eq!(m.bases("account_move"), Vec::<&str>::new());
    }

    #[test]
    fn lifts_single_inherits_from_edge() {
        let triples = vec![t("odoo:account_move", "inherits_from", "odoo:mail_thread")];
        let m = InheritanceMap::from_corpus(&triples);
        assert_eq!(m.bases("account_move"), vec!["mail_thread"]);
        assert!(m.has_bases("account_move"));
    }

    #[test]
    fn aggregates_multiple_bases_sorted() {
        let triples = vec![
            t("odoo:account_move", "inherits_from", "odoo:mail_thread"),
            t("odoo:account_move", "inherits_from", "odoo:portal_mixin"),
            t("odoo:account_move", "inherits_from", "odoo:mail_activity_mixin"),
        ];
        let m = InheritanceMap::from_corpus(&triples);
        // BTreeSet iteration is sorted; mail_activity_mixin < mail_thread < portal_mixin
        assert_eq!(
            m.bases("account_move"),
            vec!["mail_activity_mixin", "mail_thread", "portal_mixin"]
        );
    }

    #[test]
    fn skips_non_inherits_from_predicates() {
        let triples = vec![
            t("odoo:account_move", "has_function", "odoo:account_move._post"),
            t("odoo:account_move.line_ids", "target", "account.move.line"),
        ];
        let m = InheritanceMap::from_corpus(&triples);
        assert!(m.is_empty());
    }

    #[test]
    fn skips_field_iri_subjects() {
        // A subject with a member dot is a field, not a model — never a valid
        // `inherits_from` source per ruff#19's class-level shape.
        let triples = vec![t(
            "odoo:account_move.partner_id",
            "inherits_from",
            "odoo:res_partner",
        )];
        let m = InheritanceMap::from_corpus(&triples);
        assert!(m.is_empty());
    }

    #[test]
    fn slice_2_corpus_carries_account_move_inherits() {
        // FINDING: lance-graph#527 corpus regen surfaced 166 `inherits_from`
        // edges in the master corpus; 8 of them are in the slice 2 subset.
        // `account_move` carries at least one base (`mail.thread` mixin
        // composition, ratified by the consumer-side InheritanceMap probe).
        let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
        let triples = crate::parse_ndjson(ndjson).expect("slice 2 parses");
        let m = InheritanceMap::from_corpus(&triples);
        assert!(
            m.has_bases("account_move"),
            "expected `account_move` to declare at least one `_inherit` base \
             after lance-graph#527 regen; map.len() = {}",
            m.len()
        );
    }
}
