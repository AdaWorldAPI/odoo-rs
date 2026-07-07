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
//! # Repatriation note — Phase 2 DONE (2026-07-07): `Triple` + parse consumed upstream
//!
//! The schema above used to live solely in
//! `lance_graph::graph::spo::odoo_ontology` (the generic graph spine — wrong
//! altitude per `specs/REPATRIATION-FRAME.md`). Phase 1 pulled its
//! *documentation* here (the consumer that uses the vocabulary owns the docs);
//! **Phase 2 (this file) retires the duplicated `Triple` struct + `parse_ndjson`
//! reader** in favour of consuming the canonical
//! [`ruff_spo_triplet::Triple`] / [`ruff_spo_triplet::from_ndjson`]. This is the
//! council-Q1 R3-doctrine fix: odoo-rs was carrying a byte-identical copy of the
//! shared SPO carrier (both structs are `{s, p, o: String, f, c: f32}`), the
//! exact duplication `core-first-transcode-doctrine.md` warns against.
//!
//! **What consuming upstream buys us (behaviour, not just dedup):**
//! `ruff_spo_triplet::from_ndjson` additionally validates every `t.p` against
//! the **closed predicate vocabulary** (`Predicate::from_str`) — a predicate
//! typo like `depend_on` now fails loud at parse time instead of silently
//! vanishing from downstream `depends_on` queries. The old local `parse_ndjson`
//! only validated the JSON *field* shape (`deny_unknown_fields`), never the
//! predicate *value*. Every predicate the Odoo corpus emits (`rdf:type`,
//! `has_function`, `emitted_by`, `depends_on`, `reads_field`, `raises`,
//! `inherits_from`, `inverse_name`, `target`, `validation_kind`) is in
//! `Predicate::ALL`, so the shipped corpora parse clean; the fail-loud gate is
//! pure upside.
//!
//! **The one still-local predicate:** `selection_value` (schema row above) is
//! the last Odoo-specific predicate NOT yet in upstream `Predicate::ALL`. It is
//! also not emitted by any shipped corpus (P3 Selection-enumeration, still open
//! in `specs/UPSTREAM_WISHLIST.md`), so `from_ndjson` never sees it today. When
//! the extractor starts emitting it, `ruff_spo_triplet::Predicate` must grow a
//! `SelectionValue` variant first (council Q1 ruff-side follow-up) — otherwise
//! the fail-loud gate would (correctly) reject it. Tracked, not synthesised: we
//! do not add a reader for a fact no corpus carries.
//!
//! The IRI-shape helpers below (`strip_ns` / `model_of` / `member_of` /
//! `is_cross_record`) stay local — they parse the `odoo:<model>.<member>` IRI
//! convention, which is odoo-rs's frontend concern, not part of the shared
//! carrier.

/// The canonical SPO carrier — re-exported from `ruff_spo_triplet` so the
/// Odoo frontend and the OGAR transpiler share one `Triple` type (they must,
/// or `ModelGraph` values would not unify). Fields: `{s, p, o: String,
/// f, c: f32}`.
pub use ruff_spo_triplet::Triple;

/// Parse newline-delimited triples with **closed-vocabulary predicate
/// validation** — the canonical `ruff_spo_triplet::from_ndjson`, re-exported
/// under odoo-rs's historical name. Blank lines are skipped; a malformed line
/// OR an unknown predicate is a fail-loud error (the frontend emits valid JSON
/// over the closed vocabulary, so either failure means a corrupted corpus, not
/// an expected case).
pub use ruff_spo_triplet::from_ndjson as parse_ndjson;

/// A triple-corpus parse failure, with the 1-based line number — the canonical
/// `ruff_spo_triplet::ParseError` (carries `{line, message}`; the old local
/// variant's `source: serde_json::Error` field is gone, but no consumer read
/// it — `od-codegen` only uses the `Display`).
pub use ruff_spo_triplet::ParseError;

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

