//! The SPO triple corpus — the **AST** input to the OGAR compiler.
//!
//! # The compiler frame (operator-named 2026-06-22)
//!
//! `lance-graph` on top of OGAR is acting like a **compiler**, and the SPO
//! corpus is the **AST** that compiler consumes. The implication every
//! consumer of this module must hold: the AST has to *think like a compiler
//! input* — typed nodes (the 13 predicates below), provenance on every node
//! (the four truth bands below), and references that are well-formed even
//! when they resolve outside the current compilation unit ("externs",
//! resolved by the symbol table = OGAR `ClassView`, not by the slice).
//!
//! The pipeline:
//!
//! ```text
//!   Odoo source  ─[ruff_python_dto_check + odoo-blueprint-extractor]→  AST (this corpus)
//!                                       │
//!                                       │ frontend: parse_ndjson  (this module)
//!                                       ▼
//!                            Vec<Triple>  (the typed AST)
//!                                       │
//!                                       │ lowering passes (sibling modules):
//!                                       │   corpus_to_actions   → ogar_vocab::ActionDef (behaviour,
//!                                       │                          kausal-parity witness; the primary
//!                                       │                          structural+behavioural lowering is
//!                                       │                          `compile_source` over `.py` text,
//!                                       │                          not this corpus — see `ogar.rs`)
//!                                       ▼
//!                                  OGAR IR (`Class` + `ActionDef` + facet)
//!                                       │
//!                                       │ sink: lance-graph V3 database
//!                                       │ (16-byte facet key, canon-high classid) —
//!                                       │ NOT SurrealQL (deprecated 2026-07-06, see
//!                                       │ docs/W3.3-DELETE-GATE-MATRIX.md)
//!                                       ▼
//!                                  Target store / runtime
//! ```
//!
//! odoo-rs is the **Odoo language frontend** of that compiler (the per-source
//! glue); OGAR `Class`/`ActionDef` is the **IR**; lance-graph + the adapters
//! are the **compiler middle/back end**. The predicate vocabulary below IS
//! the AST node-type set — adding a 14th predicate is a language change, not
//! a documentation update.
//!
//! Byte-identical shape to `ruff_spo_triplet::ndjson` and (until the
//! repatriation completes) to `lance_graph::graph::spo::odoo_ontology` — see
//! `specs/REPATRIATION-FRAME.md`. The corpus is produced once by the Python
//! frontend (`ruff_python_dto_check` + `odoo-blueprint-extractor`); this crate
//! only *reads* it. **The schema below is the consumer-side source of truth.**
//!
//! IRI shape: `odoo:<model>.<member>` where the single dot separates model
//! from member, and dotted *dependency paths* (`account_move.line_ids.balance`)
//! are emitted verbatim — the cross-record reactive signal.
//!
//! # Triple schema (13 predicates × 3 provenance bands)
//!
//! | predicate            | subject              | object               | provenance |
//! | ---                  | ---                  | ---                  | --- |
//! | `rdf:type`           | `odoo:<family>`      | `ogit:ObjectType`    | structural |
//! | `rdf:type`           | `odoo:<fam>.<field>` | `ogit:Property`      | structural |
//! | `rdf:type`           | `odoo:<fam>.<fn>`    | `ogit:Function`      | structural |
//! | `has_function`       | `odoo:<family>`      | `odoo:<fam>.<fn>`    | structural |
//! | `emitted_by`         | `odoo:<fam>.<field>` | `odoo:<fam>.<fn>`    | body write (authoritative) |
//! | `depends_on`         | `odoo:<fam>.<field>` | `odoo:<fam>.<dep>`   | `@api.depends` arg (authoritative) |
//! | `reads_field`        | `odoo:<fam>.<fn>`    | `odoo:<fam>.<field>` | body read (inferred) |
//! | `raises`             | `odoo:<fam>.<fn>`    | `exc:<Type>`         | body raise (authoritative) |
//! | `traverses_relation` | `odoo:<fam>.<fn>`    | `odoo:<fam>.<rel>`   | body for-loop (inferred) |
//! | `target`             | `odoo:<fam>.<rel>`   | `"<comodel.dotted>"` | relational comodel (declared) |
//! | `inverse_name`       | `odoo:<fam>.<rel>`   | `"<inverse>"`        | One2many/inverse (declared) |
//! | `inherits_from`      | `odoo:<family>`      | `odoo:<base_family>` | `_inherit`/`_inherits` base (declared) |
//! | `validation_kind`    | `odoo:<fam>.<fn>`    | `"<kind>"`           | `@api.constrains` body pattern (inferred) |
//! | `selection_value`    | `odoo:<fam>.<field>` | `"<value_key>"`      | `fields.Selection` enum key (declared) |
//!
//! ## Truth-value provenance bands
//!
//! NARS `(frequency, confidence)` carries the provenance — **four** bands
//! pinned by the corpus regression test:
//!
//! - **structural-declaration** `(1.0, 1.0)` — `rdf:type`. The class
//!   definition itself declares the node's kind; certain.
//! - **structural-membership** `(1.0, 0.95)` — `has_function`. The class
//!   declares the function as a member; certain *that* the function exists,
//!   slightly less certain whether the harvest's enumeration is exhaustive
//!   (a per-emitter or partial extract may legitimately omit some).
//! - **decorator / body-authoritative** `(0.95, 0.9)` — `emitted_by`,
//!   `depends_on`, `raises`, `target`, `inverse_name`, `inherits_from`,
//!   `selection_value`. Lifted from a decorator argument or an unambiguous
//!   AST write/raise/relation.
//! - **body-inferred** `(0.85, 0.75)` — `reads_field`, `traverses_relation`,
//!   `validation_kind`. Inferred from method-body AST patterns where the
//!   match is heuristic, not certain.
//!
//! Downstream truth-aware queries filter by expectation; a hit on a
//! structural edge outweighs three hits on body-inferred edges.
//!
//! # The "a + b → c through d?" Foundry query (what the corpus is FOR)
//!
//! The Foundry compute graph answers *"which field `c` does method `d` emit
//! when inputs `a` and `b` change?"* by composing two reverse `depends_on`
//! lookups + one `emitted_by` lookup:
//!
//! ```text
//!   {c : (c depends_on a) ∧ (c depends_on b)}   then   {d : (c emitted_by d)}
//! ```
//!
//! This is a **graph deduction over the loaded triples** — not a similarity
//! search, not an LLM call. It is the structural read of Odoo's reactive
//! compute graph, and the universal pattern any ORM-with-lifecycle exposes
//! once the SPO vocabulary above is canonical.
//!
//! # Enrichment layers (additive over the base extraction)
//!
//! Two predicate families layer on top of the base AST extraction (the
//! `odoo-blueprint-extractor`'s `spo_enrich` pass):
//!
//! - **`target` / `inverse_name`** — for every relational field
//!   (Many2one / One2many / Many2many / Reference) whose comodel resolves from
//!   the Odoo source, a sibling triple keyed by the relation IRI carries the
//!   *raw* dotted comodel name (e.g.
//!   `(odoo:account_move.line_ids, target, "account.move.line")` +
//!   `(…, inverse_name, "move_id")`). This is the cross-language analog of
//!   ruff#18's `(WorkPackage.owner, class_name, "User")` — same shape, Odoo
//!   side. It resolves the phantom-target case where `invoice_line_ids` would
//!   otherwise lift to a `record<invoice_line>` that doesn't exist (the real
//!   comodel is `account.move.line`).
//! - **deep `reads_field`** — each `@api.depends('rel.leaf', …)` whose `rel`
//!   is a relational field is resolved through the `target` map and the
//!   transitive read lifted onto the field's emitting method: e.g.
//!   `(odoo:account_move._compute_amount, reads_field,
//!   odoo:account_move_line.amount_residual)` is emitted *in addition to* the
//!   shallow relation read. This surfaces the cross-model recompute-ordering
//!   edge that the surface-only corpus would leave invisible to
//!   [`crate::RecomputeDag`].
//!
//! ## `_inherit`-only extension classes
//!
//! Relational fields declared on an extension class
//! (`_inherit = "account.move"` with no `_name` — the common Odoo extension
//! form) still get their `target` / `inverse_name` lifted. Without this
//! correction the extension's fields would be dropped silently.
//!
//! ## Multi-emitter deep reads
//!
//! A field emitted by more than one method (e.g. `stock_move.quantity` is
//! emitted by both `_compute_quantity` AND `_onchange_product_uom_qty`) has
//! its deep `reads_field` lifted onto **every** emitter, not just the last —
//! otherwise the recompute-ordering edge drops for whichever emitter the
//! enrichment skipped.
//!
//! # Repatriation note
//!
//! The schema above used to live solely in
//! `lance_graph::graph::spo::odoo_ontology` (the generic graph spine — wrong
//! altitude per `specs/REPATRIATION-FRAME.md`). Pulled here in Phase 1: the
//! consumer that *uses* the SPO vocabulary now owns its documentation. A
//! later phase pushes the universal SPO predicates (`has_function`,
//! `emitted_by`, `depends_on`, `reads_field`, `raises`, `target`) to OGAR /
//! ruff as the universal cross-language vocabulary, leaving Odoo-specific
//! predicates (`validation_kind`, `inverse_name`, `inherits_from`,
//! `selection_value`, `traverses_relation`) here as muscle memory.

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

