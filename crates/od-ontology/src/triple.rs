//! The SPO triple corpus — input to the ontology projection.
//!
//! Byte-identical shape to `lance_graph::graph::spo::odoo_ontology` (the
//! `{"s","p","o","f","c"}` ndjson) and to `ruff_spo_triplet::ndjson`. The
//! corpus is produced once by the Python frontend (`ruff_python_dto_check` +
//! `odoo-blueprint-extractor`); this crate only *reads* it.
//!
//! IRI shape: `odoo:<model>.<member>` where the single dot separates model
//! from member, and dotted *dependency paths* (`account_move.line_ids.balance`)
//! are emitted verbatim — the cross-record reactive signal.

use serde::Deserialize;

/// One ontology triple: subject, predicate, object, NARS `(frequency, confidence)`.
///
/// `deny_unknown_fields` so harvester schema drift fails loudly instead of
/// silently degrading the truth signal.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Triple {
    /// Subject IRI (`odoo:account_move.amount_total`).
    pub s: String,
    /// Predicate (`depends_on`, `emitted_by`, `rdf:type`, …).
    pub p: String,
    /// Object IRI (`odoo:account_move.line_ids.balance`, `ogit:Property`, `exc:ValidationError`).
    pub o: String,
    /// NARS frequency.
    pub f: f32,
    /// NARS confidence.
    pub c: f32,
}

/// Parse newline-delimited triples. Blank lines are skipped; a malformed line
/// is an error (the frontend emits valid JSON, so a parse failure means a
/// corrupted corpus, not an expected case).
///
/// # Errors
/// Returns the offending line number + `serde_json` error on the first line
/// that fails to parse.
pub fn parse_ndjson(ndjson: &str) -> Result<Vec<Triple>, ParseError> {
    let mut out = Vec::new();
    for (i, line) in ndjson.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let t = serde_json::from_str(line).map_err(|e| ParseError {
            line: i + 1,
            source: e,
        })?;
        out.push(t);
    }
    Ok(out)
}

/// A triple-corpus parse failure, with the 1-based line number.
#[derive(Debug)]
pub struct ParseError {
    /// 1-based line number of the offending row.
    pub line: usize,
    /// Underlying `serde_json` error.
    pub source: serde_json::Error,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ndjson parse error at line {}: {}",
            self.line, self.source
        )
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Strip a known namespace prefix (`odoo:`, `ogit:`, `exc:`) from an IRI.
#[must_use]
pub fn strip_ns(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, rest)| rest)
}

/// The model segment of an `odoo:<model>.<member…>` IRI — everything before
/// the first dot of the local part. `odoo:account_move.amount_total` →
/// `account_move`; the bare `odoo:account_move` → `account_move`.
#[must_use]
pub fn model_of(iri: &str) -> &str {
    let local = strip_ns(iri);
    local.split_once('.').map_or(local, |(m, _)| m)
}

/// The member path of an `odoo:<model>.<member…>` IRI — everything after the
/// first dot. `odoo:account_move.line_ids.balance` → `line_ids.balance`. A
/// bare model IRI has no member → `None`.
#[must_use]
pub fn member_of(iri: &str) -> Option<&str> {
    strip_ns(iri).split_once('.').map(|(_, m)| m)
}

/// Whether a member path is *cross-record* — i.e. it walks a relation before
/// reaching the leaf (`line_ids.balance` has a dot; `amount_total` does not).
/// Cross-record deps lower to a `DEFINE EVENT`; same-record deps are implicit
/// in the computed field's `VALUE` recompute.
#[must_use]
pub fn is_cross_record(member: &str) -> bool {
    member.contains('.')
}

/// Split a cross-record member path into `(relation, leaf)` — the first hop and
/// the remainder. `line_ids.balance` → `("line_ids", "balance")`;
/// `company_id.country.code` → `("company_id", "country.code")`.
#[must_use]
pub fn first_hop(member: &str) -> Option<(&str, &str)> {
    member.split_once('.')
}
