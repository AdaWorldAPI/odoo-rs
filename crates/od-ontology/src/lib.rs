//! `od-ontology` — Odoo's business-logic ontology, lowered into the OGAR V3
//! transpile substrate.
//!
//! # What this is
//!
//! Odoo is not a SQL app with logic bolted on; **Odoo's ORM is an ontology**.
//! Every construct — `fields.Monetary(compute=…, store=True)`,
//! `@api.depends('line_ids.balance')`, `@api.constrains`, `def action_post`,
//! `_sql_constraints` — is a runtime assertion the framework executes. The
//! 22 245-triple SPO corpus already extracted from the Odoo source
//! (`lance_graph::graph::spo::odoo_ontology`) *is* that ontology in
//! machine-readable form.
//!
//! **`SurrealQL` is deprecated** (operator ruling 2026-07-06: *"`SurrealQL` is
//! absolutely deprecated. OGAR V3 for transpile substrate, lance-graph V3 for
//! database."*). This crate no longer lowers the ontology into a bespoke
//! `SurrealQL` DDL AST; it lowers Odoo model source straight through the OGAR
//! transpile substrate (`compile_source`) and pulls canonical classids
//! (`concept_classid` / `render_classid`) from the shared OGAR codebook.
//! Classes sink into the lance-graph V3 database (16-byte facet key,
//! canon-high classid), not a `SurrealQL` string.
//!
//! ```text
//!   Odoo .py source  ──compile_source──►  Vec<CompiledClass>  (OGAR IR: Class + ActionDef + facet)
//! ```
//!
//! # What this is NOT
//!
//! - **Not an ORM.** There is no sink-in marshalling layer.
//! - **Not a `SurrealQL` codegen tool.** The former `corpus_to_schema` /
//!   `ToSql` / `schema_to_classes` / `emit_via_ogar*` DDL-emit path has been
//!   deleted (the `SurrealQL` fork); see `docs/W3.3-DELETE-GATE-MATRIX.md`.
//!
//! # Faithful now vs deferred
//!
//! The *reactive wiring* (which field recomputes over what, which guard fires,
//! which method materialises which field, which relations link where) is 100%
//! derivable from the corpus. The compute/guard *bodies* (Python expressions)
//! port incrementally.
//!
//! # Pulling a canonical classid
//!
//! ```no_run
//! use od_ontology::{compile_source, concept_classid};
//!
//! let src = std::fs::read_to_string("account_move.py").unwrap();
//! let classes = compile_source(&src);
//! assert_eq!(concept_classid("account_move"), Some(0x0202));
//! ```

mod alignment;
mod inheritance;
mod mro;
mod ogar_actions;
// The OGAR substrate consumption surface (formerly `ogar_bridge` — renamed:
// the bridge pattern is deprecated per CONSUMER-BRIDGE-DEPRECATION; this is a
// direct, always-compiled-in consumer of `ogar-vocab` + the transpile substrate).
mod ogar;
mod recompute_dag;
mod relations;
mod triple;
mod view_mask;

pub use ogar::{compile_source, concept_classid, render_classid, ODOO_APP_PREFIX};

/// Behavioral-arm lowering — Odoo's reactive lifecycle → `ogar_vocab::ActionDef`.
/// See `specs/W3-BEHAVIORAL-ARM-SCOPE.md`.
#[allow(deprecated)] // corpus DO-arm stays exported as the kausal-parity witness
pub use ogar_actions::{corpus_action_rows, corpus_to_actions};

/// Re-export the canonical OGAR codebook constants (e.g.
/// `class_ids::BILLABLE_WORK_ENTRY`) so consumers can symbol-bind to the
/// shared ids rather than copy hex literals.
pub use ogar_vocab::class_ids;

pub use alignment::{
    dolce_odoo, resolve_odoo, strip_odoo_prefix, DolceMarker, OdooFamily, OdooSeedRow,
    OntologyBundleId, OwlPivot, FAMILY_BILLING_CORE, FAMILY_HR_FOUNDATION, FAMILY_PRODUCT_CATALOG,
    FAMILY_SMB_ACCOUNTING, FAMILY_SMB_FOUNDRY_CUSTOMER, FAMILY_SMB_FOUNDRY_INVOICE, ODOO_BUNDLE_ID,
    ODOO_EDGE_WHITELIST, ODOO_INHERITS_FROM_FIBOFND_V1, ODOO_NAMESPACE_IRI, ODOO_SEED,
    ODOO_TTL_SOURCES,
};
pub use inheritance::InheritanceMap;
pub use mro::{Mro, MroError};
pub use view_mask::{extract_view_fields, field_universe, mint_mask, MaskWords, ViewFields};
/// Wide-mask convergence — mint `lance_graph_contract::WideFieldMask`
/// straight from the view-stratum harvest. See `view_mask`'s module docs.
#[cfg(feature = "fieldmask")]
pub use view_mask::{mint_wide_mask, WideMaskError};
pub use recompute_dag::{MethodId, MethodKind, RecomputeDag};
pub use relations::{Relation, RelationMap, RelationParseError};
pub use triple::{member_of, model_of, parse_ndjson, ParseError, Triple};
