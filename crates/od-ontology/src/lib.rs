//! `od-ontology` — Odoo's business-logic ontology as a native SurrealDB schema.
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
//! This crate lowers that ontology into **native SurrealDB constructs** so the
//! semantics live *in the database*, not marshalled into a Rust process:
//!
//! ```text
//!   SPO corpus  ──corpus_to_schema──►  Schema { tables, functions, events }
//!                                          │ ToSql
//!                                          ▼
//!                              SurrealQL DDL (DEFINE TABLE / FIELD / FUNCTION / EVENT)
//! ```
//!
//! # What this is NOT
//!
//! - **Not an ORM.** There is no sink-in marshalling layer. SurrealDB is the
//!   runtime; Rust (later) is a thin query broker, never the owner of the logic.
//! - **Not a codegen / migration tool.** That convenience layer (export typed
//!   AST, snapshot, shelve) is the explicitly *deferred* tail — see the README.
//!
//! # Faithful now vs deferred
//!
//! The *reactive wiring* (which field recomputes over what, which guard fires,
//! which method materialises which field, which relations link where) is 100%
//! derivable from the corpus and lands immediately. The compute/guard *bodies*
//! (Python expressions), exact field types, and cross-record child-table
//! resolution are stubbed and port incrementally — the schema is a faithful
//! skeleton-with-nerves on day one.
//!
//! # Slice 1 — `account.move`
//!
//! ```no_run
//! use od_ontology::{corpus_to_schema, parse_ndjson, ToSql};
//!
//! let ndjson = std::fs::read_to_string("data/account_move.spo.ndjson").unwrap();
//! let triples = parse_ndjson(&ndjson).unwrap();
//! let schema = corpus_to_schema(&triples, Some(&["account_move"]), None);
//! println!("{}", schema.to_sql());
//! ```

mod alignment;
mod emit;
mod inheritance;
mod mro;
mod ogar_actions;
// The OGAR substrate consumption surface (formerly `ogar_bridge` — renamed:
// the bridge pattern is deprecated per CONSUMER-BRIDGE-DEPRECATION; this is a
// direct, always-compiled-in consumer of `ogar-vocab` + the transpile substrate).
mod ogar;
mod recompute_dag;
mod relations;
mod surreal_ast;
mod triple;
mod view_mask;

pub use ogar::{
    compile_source, concept_classid, emit_source_via_ogar, emit_via_ogar, emit_via_ogar_annotated,
    render_classid, schema_classids, schema_to_classes, ODOO_APP_PREFIX,
};

/// Behavioral-arm lowering — Odoo's reactive lifecycle → `ogar_vocab::ActionDef`
/// (the sibling of [`schema_to_classes`]). See `specs/W3-BEHAVIORAL-ARM-SCOPE.md`.
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
pub use emit::corpus_to_schema;
pub use inheritance::InheritanceMap;
pub use mro::{Mro, MroError};
pub use view_mask::{extract_view_fields, field_universe, mint_mask, MaskWords, ViewFields};
/// Wide-mask convergence — mint `lance_graph_contract::WideFieldMask`
/// straight from the view-stratum harvest. See `view_mask`'s module docs.
#[cfg(feature = "fieldmask")]
pub use view_mask::{mint_wide_mask, WideMaskError};
pub use recompute_dag::{MethodId, MethodKind, RecomputeDag};
pub use relations::{Relation, RelationMap, RelationParseError};
pub use surreal_ast::{
    EventDefinition, FieldDefinition, FunctionDefinition, IndexDefinition, Kind, Schema,
    TableDefinition, ToSql,
};
pub use triple::{member_of, model_of, parse_ndjson, ParseError, Triple};
