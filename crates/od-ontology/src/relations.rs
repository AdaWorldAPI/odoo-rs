//! Typed relation overrides — the `OdooEntity.fields[…].target` ground truth
//! in transit between the lance-graph-ontology blueprint and the projection.
//!
//! # Why this exists
//!
//! The projection's name-heuristic resolver (`emit::resolve_target`) is right
//! often enough to be useful (`partner_id` → `res_partner`, `line_ids` →
//! `account_move_line`) but wrong on the Odoo cases that don't follow
//! convention. The canonical Many2one/One2many target is the **first
//! positional arg of the field decorator**:
//!
//! ```python
//! invoice_line_ids = fields.One2many('account.move.line', 'move_id', domain=[…])
//! #                                  ↑ TARGET             ↑ INVERSE
//! ```
//!
//! That truth lives in
//! `lance-graph-ontology::odoo_blueprint::OdooField { target: Some("..."), …}`.
//! Rather than pull lance-graph-ontology (oxttl + oxrdf + …) as a hard dep, we
//! treat the relation set as a **durable data artifact**: a separate
//! `od-ontology-bridge` binary extracts `OdooEntity::fields` once and writes
//! the ndjson; this crate reads it.
//!
//! # On-disk shape
//!
//! ```text
//! {"model":"account_move","field":"line_ids","target":"account_move_line","inverse":"move_id"}
//! {"model":"account_move","field":"invoice_line_ids","target":"account_move_line","inverse":"move_id"}
//! {"model":"account_move","field":"partner_id","target":"res_partner","inverse":null}
//! ```
//!
//! Names are **underscored** (matching the SPO corpus IRI form), not dotted
//! (the OdooEntity blueprint form). The bridge binary does the conversion.

use std::collections::BTreeMap;

use serde::Deserialize;

/// One Many2one / One2many / Many2many declaration lifted from the typed
/// `OdooField` decorator.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    /// Owning model (underscored: `account_move`).
    pub model: String,
    /// Field name (`line_ids`, `partner_id`).
    pub field: String,
    /// Target table (underscored: `account_move_line`, `res_partner`).
    pub target: String,
    /// One2many inverse field on the target. `Many2one`/`Many2many` set `null`.
    pub inverse: Option<String>,
}

/// A lookup table from `(model, field)` to the declared target table + inverse.
///
/// Lookups are O(log n) over a `BTreeMap` keyed by `(model, field)`. Construct
/// via [`RelationMap::from_ndjson`] or [`RelationMap::insert`].
#[derive(Debug, Clone, Default)]
pub struct RelationMap {
    by_field: BTreeMap<(String, String), (String, Option<String>)>,
}

impl RelationMap {
    /// An empty map. The projection treats `None` and an empty map equivalently
    /// — both fall back to the convention ladder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a relation-override file (one [`Relation`] per non-empty line).
    ///
    /// # Errors
    /// Returns the 1-based line number + the underlying serde_json error on
    /// the first row that fails to parse.
    pub fn from_ndjson(ndjson: &str) -> Result<Self, RelationParseError> {
        let mut by_field = BTreeMap::new();
        for (i, line) in ndjson.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let r: Relation = serde_json::from_str(line).map_err(|e| RelationParseError {
                line: i + 1,
                source: e,
            })?;
            by_field.insert((r.model, r.field), (r.target, r.inverse));
        }
        Ok(Self { by_field })
    }

    /// Add a relation override programmatically.
    pub fn insert(
        &mut self,
        model: impl Into<String>,
        field: impl Into<String>,
        target: impl Into<String>,
        inverse: Option<String>,
    ) {
        self.by_field
            .insert((model.into(), field.into()), (target.into(), inverse));
    }

    /// The declared target table for `<model>.<field>`, if known.
    #[must_use]
    pub fn target(&self, model: &str, field: &str) -> Option<&str> {
        self.by_field
            .get(&(model.to_string(), field.to_string()))
            .map(|(t, _)| t.as_str())
    }

    /// The declared inverse field on the target (One2many back-ref), if known.
    #[must_use]
    pub fn inverse(&self, model: &str, field: &str) -> Option<&str> {
        self.by_field
            .get(&(model.to_string(), field.to_string()))
            .and_then(|(_, i)| i.as_deref())
    }

    /// Number of declared relations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_field.len()
    }

    /// Whether the map carries any overrides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_field.is_empty()
    }

    /// Build a relation map directly from a SPO triple slice, consuming the
    /// `target` and `inverse_name` predicates the
    /// `lance-graph/tools/odoo-blueprint-extractor` enrichment landed for
    /// `AdaWorldAPI/odoo-rs` (PR ratified the cross-language shape from
    /// `AdaWorldAPI/ruff#18`).
    ///
    /// Wire shape:
    ///
    /// ```text
    /// (odoo:account_move.line_ids, target, "account.move.line")
    /// (odoo:account_move.line_ids, inverse_name, "move_id")
    /// ```
    ///
    /// Targets are normalized **dots → underscores** so they match the rest
    /// of this crate's API (which keys models by underscored ident, mirroring
    /// the `odoo:<model>` IRI's local part). Unknown / framework subjects
    /// (no `<model>.<field>` split) are skipped silently.
    ///
    /// When the corpus carries the same `(model, field)` as a sidecar
    /// `relations.ndjson`, the corpus wins — call this AFTER any
    /// [`Self::from_ndjson`] / [`Self::insert`] to override, or BEFORE to be
    /// overridden. The map is APPEND on key collisions (last write wins).
    #[must_use]
    pub fn from_corpus(triples: &[crate::Triple]) -> Self {
        let mut targets: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut inverses: BTreeMap<(String, String), String> = BTreeMap::new();

        for t in triples {
            if t.p != "target" && t.p != "inverse_name" {
                continue;
            }
            let Some((model, field)) = split_model_field(&t.s) else {
                continue;
            };
            let key = (model.to_string(), field.to_string());
            if t.p == "target" {
                targets.insert(key, t.o.replace('.', "_"));
            } else {
                inverses.insert(key, t.o.clone());
            }
        }

        let mut by_field = BTreeMap::new();
        for (key, target) in targets {
            let inverse = inverses.remove(&key);
            by_field.insert(key, (target, inverse));
        }
        Self { by_field }
    }
}

/// Split `odoo:<model>.<field>` into `(model, field)`. Returns `None` for
/// bare-model IRIs or framework symbols that don't follow the form.
fn split_model_field(iri: &str) -> Option<(&str, &str)> {
    let local = iri.split_once(':').map_or(iri, |(_, r)| r);
    local.split_once('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Triple;

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
    fn from_corpus_lifts_target_and_inverse() {
        let triples = vec![
            t("odoo:account_move.line_ids", "target", "account.move.line"),
            t("odoo:account_move.line_ids", "inverse_name", "move_id"),
            t("odoo:account_move.partner_id", "target", "res.partner"),
            t("odoo:account_move", "has_function", "odoo:account_move._post"),
        ];
        let map = RelationMap::from_corpus(&triples);
        assert_eq!(map.target("account_move", "line_ids"), Some("account_move_line"));
        assert_eq!(map.inverse("account_move", "line_ids"), Some("move_id"));
        assert_eq!(map.target("account_move", "partner_id"), Some("res_partner"));
        assert_eq!(map.inverse("account_move", "partner_id"), None);
    }

    #[test]
    fn slice_2_corpus_yields_account_move_line_ids() {
        // FINDING: the wishlist's P1 (FK-target-override) landed upstream
        // in lance-graph PR #523. The fresh slice 2 corpus now carries the
        // `target` + `inverse_name` predicates this method consumes.
        // `RelationMap::from_corpus` is the ~30 LOC promised in the wishlist.
        let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
        let triples = crate::parse_ndjson(ndjson).expect("slice 2 parses");
        let map = RelationMap::from_corpus(&triples);
        assert_eq!(
            map.target("account_move", "line_ids"),
            Some("account_move_line"),
            "FK-target-override should resolve line_ids → account_move_line"
        );
        assert_eq!(
            map.inverse("account_move", "line_ids"),
            Some("move_id"),
            "inverse_name should be move_id"
        );
    }
}

/// Relation-override parse failure.
#[derive(Debug)]
pub struct RelationParseError {
    /// 1-based line number of the offending row.
    pub line: usize,
    /// Underlying serde_json error.
    pub source: serde_json::Error,
}

impl std::fmt::Display for RelationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "relation-map ndjson parse error at line {}: {}",
            self.line, self.source
        )
    }
}

impl std::error::Error for RelationParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
