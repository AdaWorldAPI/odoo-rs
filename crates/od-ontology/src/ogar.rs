//! **Stage 1 — parallel emit.** Lowers the bespoke [`Schema`] onto
//! `ogar_vocab::Class` (the canonical OGAR IR) and re-emits SurrealQL DDL
//! through `ogar-adapter-surrealql`, *alongside* the native [`ToSql`] path —
//! never replacing it.
//!
//! [`Schema`]: crate::Schema
//! [`ToSql`]: crate::ToSql
//!
//! # Why
//!
//! `od-ontology` and `ogar-adapter-surrealql` independently arrived at the
//! same design: mirror `surrealdb-core`'s catalog shape, hand-write the DDL
//! formatter, stay zero-dep on `surrealdb-core`. Two crates emitting the same
//! catalog from the same kind of input is a fork that should be a dependency.
//! This module is the beachhead: it proves the bespoke `Schema` maps onto
//! `ogar_vocab::Class`, so a future stage can delete the hand-rolled AST +
//! `ToSql` and route DDL through the shared emitter — making Odoo (ERP) and
//! the planning forks (OpenProject / Redmine) *one reusable ontology* on the
//! OGAR codebook (the `OdooPort` vocabulary anchor, OGAR #94).
//!
//! # What converges today (the `ogar_vocab::Class` IR covers)
//!
//! | bespoke `Schema` construct | `ogar_vocab::Class` slot | emitted DDL |
//! |---|---|---|
//! | [`TableDefinition`] | [`Class`] | `DEFINE TABLE <t> SCHEMAFULL` |
//! | scalar [`FieldDefinition`] (`bool`/`int`/`string`/…) | [`Attribute`] | `DEFINE FIELD … TYPE <scalar>` |
//! | nullable scalar (`option<…>`) | [`Attribute`] + `required = Some(false)` | `… TYPE option<scalar>` |
//! | `Many2one` (`record<X>`) | [`Association`] `BelongsTo` | `… TYPE record<X>` |
//! | nullable `Many2one` | `BelongsTo` + `optional = Some(true)` | `… TYPE option<record<X>>` |
//!
//! # Documented Stage-2 gaps (NOT yet representable through the emitter)
//!
//! The convergence is partial *by design* — this stage measures the gap, it
//! does not hide it. The native emit still owns these until the shared
//! emitter grows to cover them (asserted as gaps in the convergence test):
//!
//! - **One2many / Many2many** (`array<record<…>>`): lowered to a `HasMany`
//!   association, which the OGAR emitter renders as a `--` comment (the
//!   non-owning side carries no column), *not* an `array<record<…>>` field.
//! - **Computed fields** (`VALUE fn::… READONLY`): the field's *type* carries
//!   over; its reactive `VALUE` + `READONLY` do not (that's the
//!   `ActionDef` / `ClassView` surface, a separate lift).
//! - **`DEFINE FUNCTION`** (compute/action stubs) and **`DEFINE EVENT`**
//!   (reactive recompute + `@api.constrains` guards): no equivalent in the
//!   `DEFINE TABLE`/`DEFINE FIELD`-only emitter yet.
//! - **`DEFINE INDEX`** (`_sql_constraints` unique tuples): not part of the
//!   OGAR IR; dropped.

use ogar_from_ruff::mint::{compile_graph_python, CompiledClass};
use ogar_vocab::app::render_classid_for;
use ogar_vocab::ports::{OdooPort, PortSpec};
use ogar_vocab::{canonical_concept_name, Association, AssociationKind, Attribute, Class};

use crate::surreal_ast::{FieldDefinition, Kind, Schema, TableDefinition};

/// Lower a bespoke [`Schema`](crate::Schema) onto the canonical
/// `ogar_vocab::Class` IR (the convergent `DEFINE TABLE` + `DEFINE FIELD`
/// subset; see the module docs for the documented gaps).
#[must_use]
pub fn schema_to_classes(schema: &Schema) -> Vec<Class> {
    schema.tables.iter().map(table_to_class).collect()
}

/// Emit SurrealQL DDL for `schema` through the shared `ogar-adapter-surrealql`
/// emitter (the OGAR-canonical path), parallel to [`Schema::to_sql`].
///
/// [`Schema::to_sql`]: crate::ToSql::to_sql
#[deprecated(since = "0.5.0", note = "SurrealQL is deprecated (operator ruling 2026-07-06): OGAR V3 is the transpile substrate, lance-graph V3 the database. Consume `compile_source` / `schema_to_classes` and sink the classes; DDL for the PostgreSQL system-of-record comes from the ClassView via ogar-adapter-postgres-ddl.")]
#[must_use]
pub fn emit_via_ogar(schema: &Schema) -> String {
    ogar_adapter_surrealql::emit_surrealql_ddl(&schema_to_classes(schema))
}

/// [`emit_via_ogar`] DDL, with each codebook table's canonical OGAR concept
/// **name** + full render classid stamped into its `DEFINE TABLE …
/// COMMENT 'commercial_document (classid:0x02020002)'` clause — so the shared
/// concept rides into `SurrealDB`'s own catalog metadata (queryable via
/// `INFO FOR TABLE`), human-readable, not just the emitted `.surql` text. A
/// `--` line-comment would evaporate at parse time; the `COMMENT` clause
/// survives ingestion.
///
/// Purely additive over [`schema_to_classes`]: a table outside the codebook
/// gets no classid comment (its structural DDL is byte-identical to the native
/// [`emit_via_ogar`]). The concept name comes from
/// [`ogar_vocab::canonical_concept_name`] — OGAR's `id → name` reverse map
/// (the `PROBE-OGAR-ID-TO-CONCEPT-NAME` capability, OGAR #98) — never
/// re-derived or copied locally, per the Core-First doctrine.
#[deprecated(since = "0.5.0", note = "SurrealQL is deprecated (operator ruling 2026-07-06). The classid+concept identity now rides the V3 facet (`CompiledClass.facet`), not a DDL COMMENT clause.")]
#[must_use]
pub fn emit_via_ogar_annotated(schema: &Schema) -> String {
    let classes: Vec<Class> = schema
        .tables
        .iter()
        .map(|table| {
            let mut class = table_to_class(table);
            // Identity stamp: the canonical concept name + full render classid
            // ride into the catalog COMMENT (the emitter renders
            // `class.description` as `… COMMENT '<desc>'`). Uncodified tables
            // keep `description = None` → no classid clause.
            if let Some(lo) = concept_classid(&table.name) {
                let render = render_classid_for::<OdooPort>(lo);
                class.description = Some(match canonical_concept_name(lo) {
                    Some(name) => format!("{name} (classid:0x{render:08X})"),
                    // `lo` came from the codebook, so the reverse lookup is
                    // total — the bare-hex arm is a defensive fallback only.
                    None => format!("classid:0x{render:08X}"),
                });
            }
            class
        })
        .collect();
    ogar_adapter_surrealql::emit_surrealql_ddl(&classes)
}

// ── Substrate-input lowering (Phase 2 Stage B) ──────────────────────────
//
// The convergence's INPUT half: lower an Odoo model *source* (`.py` text)
// straight through the shared OGAR transpile substrate (OGAR #132) — parse with
// `ruff_python_spo`, lift + mint with `compile_graph_python::<OdooPort>` — instead
// of re-deriving it through the bespoke `parse_ndjson -> corpus_to_schema ->
// schema_to_classes` corpus path. This is the "85% pulled from OGAR" thinning:
// `od-ontology` stops owning the lift and becomes a substrate caller. The shared
// `ogar-adapter-surrealql` then emits the DDL — including the Stage-A
// `array<record<…>>` for One2many/Many2many. Additive: the native path is
// untouched, and the W3.3 fork-delete (Stage C) stays gated on the behaviour arm.

/// Compile Odoo model source (`.py` text) to the canonical OGAR
/// `Vec<CompiledClass>` via `ruff_python_spo` + `compile_graph_python::<OdooPort>`
/// — the substrate-input leg of the odoo-rs⟷OGAR convergence. Source that fails
/// to parse contributes nothing (`ruff_python_spo`'s silent-skip invariant).
///
/// Each [`CompiledClass`] carries the lifted `ogar_vocab::Class` (structure:
/// attributes + associations, with the comodel on each association) and the
/// minted `facet` (identity: the render classid for codebook models).
#[must_use]
pub fn compile_source(src: &str) -> Vec<CompiledClass> {
    compile_graph_python::<OdooPort>(&ruff_python_spo::extract_from_source(src))
}

/// Emit SurrealQL DDL for Odoo model *source* through the shared OGAR substrate
/// and `ogar-adapter-surrealql`, stamping each codebook table's canonical
/// concept name and full render classid into its `DEFINE TABLE … COMMENT` clause.
///
/// The substrate-input sibling of [`emit_via_ogar_annotated`] — which lowers the
/// bespoke [`Schema`](crate::Schema); both converge on the same emitter, so the
/// `source` path and the `corpus` path produce the same shape of DDL. A model
/// outside the `OdooPort` codebook mints render classid `0` and gets no COMMENT;
/// its structural DDL is unaffected (the stamp is identity-only — never
/// lifecycle/behaviour, per the SurrealQL-AST-trap rule). The concept name comes
/// from [`canonical_concept_name`] (OGAR's `id -> name` reverse map), never
/// re-derived locally.
#[deprecated(since = "0.5.0", note = "SurrealQL is deprecated (operator ruling 2026-07-06). Use `compile_source` — the `Vec<CompiledClass>` IS the product; storage is the lance-graph V3 substrate.")]
#[must_use]
pub fn emit_source_via_ogar(src: &str) -> String {
    let classes: Vec<Class> = compile_source(src)
        .into_iter()
        .map(|cc| {
            let render = cc.facet.facet_classid();
            let mut class = cc.class;
            // Codebook hit (non-zero render classid): stamp the concept name +
            // render id into the catalog COMMENT (the emitter renders
            // `class.description` as `… COMMENT '<desc>'`). The low u16 is the
            // shared concept the reverse map keys on.
            if render != 0 {
                // canon-high (OGAR D-CLASSID-CANON-HIGH-FLIP, 2026-07-02):
                // concept = HIGH u16, app render lens = LOW u16.
                let concept = (render >> 16) as u16;
                class.description = Some(match canonical_concept_name(concept) {
                    Some(name) => format!("{name} (classid:0x{render:08X})"),
                    None => format!("classid:0x{render:08X}"),
                });
            }
            // Normalize relational comodel targets to TABLE form (dot ->
            // underscore) so the emitted `record<…>` points at the SurrealDB
            // table the schema defines. The substrate carries the raw dotted
            // Odoo comodel (`res.partner`) as provenance, but a model `_name` is
            // lowered to a table named `res_partner` (`_name.replace('.', '_')`),
            // so an un-normalized `record<`res.partner`>` would dangle. The
            // table-naming decision is the consumer's (Codex P2 on #20).
            for assoc in &mut class.associations {
                if let Some(target) = &assoc.class_name {
                    assoc.class_name = Some(target.replace('.', "_"));
                }
            }
            class
        })
        .collect();
    ogar_adapter_surrealql::emit_surrealql_ddl(&classes)
}

// ── Canonical classid pull (the "pull OGAR via class" deliverable) ──────
//
// `schema_to_classes` above lowers *structure* (tables → `Class` shells with
// attributes + associations). The functions below lower *identity*: they pull
// the canonical OGAR `classid` for an Odoo model straight from the [`OdooPort`]
// alias table — NO bridge object, NO registry, NO TTL hydration, just a pure
// static lookup over the shared codebook (OGAR #94). This is the consumer
// migration target (lance-graph #589 / OGAR `CONSUMER-MIGRATION-HOWTO`): a
// consumer names a surface concept and gets back the shared id that WoA
// `Stundenzettel`, SMB `Stundenzettel`, OpenProject/Redmine `TimeEntry`, and
// Odoo `account.analytic.line` all converge on (`BILLABLE_WORK_ENTRY`, the
// planner↔ERP billable-hours pin).

/// Odoo's APP-prefix — the high `u16` of the 32-bit *render* classid per OGAR's
/// `APP-CLASS-CODEBOOK-LAYOUT` (`classid = concept(hi) ‖ APP(lo) — canon-high per D-CLASSID-CANON-HIGH-FLIP`). The low
/// `u16` is the shared cross-app concept (WHAT it is — RBAC + ontology); the
/// high `u16` is the per-app render lens (WHOSE template). `0x0002` is Odoo's
/// lens, so every Odoo-rendered id is `0x0002_<concept>`.
///
/// **Bound to the Core** (OGAR #97): this is [`OdooPort::APP_PREFIX`], not a
/// local literal — the prefix allocation lives once, in OGAR's `PortSpec`. If
/// OGAR ever re-allocates Odoo's prefix this follows automatically; there is
/// no second copy to drift.
pub const ODOO_APP_PREFIX: u16 = OdooPort::APP_PREFIX;

/// Pull the canonical OGAR **concept** classid (the shared low `u16`) for an
/// Odoo model name, straight through [`OdooPort`] — the static alias table, no
/// bridge object, no registry, no hydration.
///
/// The SPO corpus names models in Odoo's *table* form (`.` replaced by `_`:
/// `account_move`, `account_analytic_line`); [`OdooPort`]'s aliases are the
/// *model* form (`account.move`, `account.analytic.line`). Deriving the table
/// name as `_name.replace('.', '_')` is the canonical Odoo convention, so the
/// inverse `_`→`.` recovers the model name losslessly; a name already in model
/// form (no `_`) passes through unchanged. The raw name is also tried as a
/// fallback so a caller that already holds a dotted model name still resolves.
///
/// Returns `None` for a model outside the codebook (e.g. `ir_cron`); the
/// structural lowering in [`schema_to_classes`] still covers such a table — only
/// the canonical-identity pull is codebook-gated.
///
/// `account_move` → `0x0202` (`COMMERCIAL_DOCUMENT`), `account_analytic_line` →
/// `0x0103` (`BILLABLE_WORK_ENTRY`, the cross-arm bridge), `res_partner` →
/// `0x0204` (`BILLING_PARTY`).
///
/// **Scope caveat — `_`→`.` is not a universal bijection.** The normalize is
/// lossless for all nine codebook aliases: each alias consists of dot-separated
/// single-word segments (no underscore inside a segment), so
/// `account_analytic_line` → `account.analytic.line` is an exact round-trip.
/// Odoo localization and multi-word module classes that carry an underscore
/// *inside* a segment (e.g. `l10n_es_edi_document`, `im_livechat_channel`) are
/// intentionally out of codebook scope and resolve to `None` — a fail-safe miss,
/// never a wrong id. Callers that need to map such names should maintain their
/// own alias table rather than extending the `_`→`.` heuristic.
#[must_use]
pub fn concept_classid(model: &str) -> Option<u16> {
    OdooPort::class_id(&model.replace('_', ".")).or_else(|| OdooPort::class_id(model))
}

/// The full 32-bit **render** classid for an Odoo model: Odoo's APP prefix
/// (`0x0002`) in the high `u16`, the shared canonical [`concept_classid`] in
/// the low `u16`. `account_move` → `0x0202_0002`; `account_analytic_line` →
/// `0x0103_0002`. `None` for an unaliased model.
///
/// Composition is OGAR's canonical [`render_classid_for`] (OGAR #97), not a
/// hand-rolled shift — the `(prefix << 16) | concept` layout lives in one
/// place (the Core), never re-implemented per consumer.
#[must_use]
pub fn render_classid(model: &str) -> Option<u32> {
    concept_classid(model).map(render_classid_for::<OdooPort>)
}

/// Resolve every table in `schema` to its canonical OGAR concept classid,
/// pairing the table name with its [`concept_classid`]. Tables outside the
/// `OdooPort` codebook resolve to `None`. Order mirrors `schema.tables`.
#[must_use]
pub fn schema_classids(schema: &Schema) -> Vec<(String, Option<u16>)> {
    schema
        .tables
        .iter()
        .map(|t| (t.name.clone(), concept_classid(&t.name)))
        .collect()
}

fn table_to_class(table: &TableDefinition) -> Class {
    let mut class = Class::new(table.name.as_str());
    for field in &table.fields {
        lower_field(field, &mut class);
    }
    class
}

/// Route one [`FieldDefinition`] to a [`Class`] attribute or association,
/// stripping a single `option<…>` layer into the IR-canonical nullability
/// marker (`Kind::optional()` already normalises `option<option<T>>`).
fn lower_field(field: &FieldDefinition, class: &mut Class) {
    let (inner, optional) = match &field.kind {
        Kind::Option(inner) => (inner.as_ref(), true),
        other => (other, false),
    };

    match inner {
        // Many2one → owning-side association (FK lives on this table).
        Kind::Record(targets) => {
            let mut assoc = Association::new(AssociationKind::BelongsTo, field.name.as_str());
            assoc.class_name = Some(record_target(targets));
            if optional {
                assoc.optional = Some(true);
            }
            class.associations.push(assoc);
        }
        // One2many / Many2many → non-owning collection side. DOCUMENTED gap:
        // the OGAR emitter renders HasMany as a comment, not an
        // `array<record<…>>` field. Target preserved when the element is a
        // record so a future emitter extension can recover it.
        Kind::Array(elem) => {
            let mut assoc = Association::new(AssociationKind::HasMany, field.name.as_str());
            if let Kind::Record(targets) = elem.as_ref() {
                assoc.class_name = Some(record_target(targets));
            }
            class.associations.push(assoc);
        }
        // Scalars. NOTE: `field.value` (computed VALUE) and `field.readonly`
        // are intentionally not carried — the reactive/computed layer is a
        // documented Stage-2 gap (see module docs).
        scalar => {
            let mut attr = Attribute::new(field.name.as_str());
            attr.type_name = Some(scalar_surql_name(scalar).to_owned());
            if optional {
                attr.options.required = Some(false);
            }
            class.attributes.push(attr);
        }
    }
}

/// A `record<…>` target as a single SurrealQL identifier. Single-target
/// `Many2one` is the common case; a polymorphic multi-target slot is joined
/// with `|` so the emitter renders `record<a|b|c>` faithfully.
fn record_target(targets: &[String]) -> String {
    targets.join("|")
}

/// The SurrealQL canonical scalar name `ogar-adapter-surrealql`'s
/// `map_type_to_surrealql` consumes as-is. Non-scalar kinds are handled by
/// the caller; the catch-all keeps this total without an `unreachable!`.
fn scalar_surql_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Bool => "bool",
        Kind::Int => "int",
        Kind::Float => "float",
        Kind::Decimal => "decimal",
        Kind::String => "string",
        Kind::Datetime => "datetime",
        // `Any`, plus the defensive catch-all for kinds the caller already
        // peeled (`Record` / `Array` / a residual `Option`): emit `any`.
        Kind::Any | Kind::Record(_) | Kind::Array(_) | Kind::Option(_) => "any",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surreal_ast::{FieldDefinition, Kind, TableDefinition};

    fn class_for(table: TableDefinition) -> Class {
        let schema = Schema {
            tables: vec![table],
            functions: Vec::new(),
            events: Vec::new(),
        };
        schema_to_classes(&schema).pop().expect("one class")
    }

    #[test]
    fn table_becomes_class_with_same_name() {
        let c = class_for(TableDefinition::new("account_move"));
        assert_eq!(c.name, "account_move");
    }

    #[test]
    fn required_scalar_becomes_attribute_without_optional_marker() {
        let mut t = TableDefinition::new("account_move");
        let mut f = FieldDefinition::new("account_move", "name");
        f.kind = Kind::String; // bare = required
        t.fields.push(f);
        let c = class_for(t);
        let attr = &c.attributes[0];
        assert_eq!(attr.name, "name");
        assert_eq!(attr.type_name.as_deref(), Some("string"));
        assert_eq!(attr.options.required, None);
    }

    #[test]
    fn nullable_scalar_carries_required_false() {
        let mut t = TableDefinition::new("account_move");
        let mut f = FieldDefinition::new("account_move", "ref");
        f.kind = Kind::String.optional();
        t.fields.push(f);
        let c = class_for(t);
        assert_eq!(c.attributes[0].options.required, Some(false));
    }

    #[test]
    fn many2one_becomes_belongs_to_association() {
        let mut t = TableDefinition::new("account_move");
        let mut f = FieldDefinition::new("account_move", "partner_id");
        f.kind = Kind::Record(vec!["res_partner".to_string()]).optional();
        t.fields.push(f);
        let c = class_for(t);
        let a = &c.associations[0];
        assert_eq!(a.kind, AssociationKind::BelongsTo);
        assert_eq!(a.name, "partner_id");
        assert_eq!(a.class_name.as_deref(), Some("res_partner"));
        assert_eq!(a.optional, Some(true));
    }

    #[test]
    fn one2many_becomes_has_many_association() {
        let mut t = TableDefinition::new("account_move");
        let mut f = FieldDefinition::new("account_move", "line_ids");
        f.kind = Kind::Array(Box::new(Kind::Record(
            vec!["account_move_line".to_string()],
        )))
        .optional();
        t.fields.push(f);
        let c = class_for(t);
        let a = &c.associations[0];
        assert_eq!(a.kind, AssociationKind::HasMany);
        assert_eq!(a.class_name.as_deref(), Some("account_move_line"));
    }

    // ── Canonical classid pull ──────────────────────────────────────────

    #[test]
    fn concept_classid_pulls_commercial_document_for_account_move() {
        // account_move (table form) → account.move (model form) →
        // COMMERCIAL_DOCUMENT 0x0202, straight from the OdooPort alias table.
        assert_eq!(concept_classid("account_move"), Some(0x0202));
    }

    #[test]
    fn concept_classid_pulls_billable_work_entry_for_analytic_line() {
        // The planner↔ERP convergence pin: account.analytic.line is the
        // cross-arm bridge into the project domain. Same id WoA/SMB
        // Stundenzettel + OpenProject/Redmine TimeEntry resolve to.
        assert_eq!(concept_classid("account_analytic_line"), Some(0x0103));
    }

    #[test]
    fn concept_classid_pulls_billing_party_for_res_partner() {
        assert_eq!(concept_classid("res_partner"), Some(0x0204));
    }

    #[test]
    fn concept_classid_resolves_multi_dot_model_names() {
        // Two underscores → two dots: account_move_line → account.move.line
        // → COMMERCIAL_LINE_ITEM 0x0201.
        assert_eq!(concept_classid("account_move_line"), Some(0x0201));
    }

    #[test]
    fn concept_classid_accepts_already_dotted_model_form() {
        // A caller already holding the dotted model name resolves via the
        // raw-name fallback (the normalize is a no-op without underscores).
        assert_eq!(concept_classid("account.move"), Some(0x0202));
    }

    #[test]
    fn concept_classid_is_none_outside_the_codebook() {
        assert_eq!(concept_classid("ir_cron"), None);
        assert_eq!(concept_classid(""), None);
    }

    #[test]
    fn render_classid_stamps_odoo_app_prefix() {
        // 0x0002 (Odoo render lens) ‖ low concept.
        assert_eq!(render_classid("account_move"), Some(0x0202_0002));
        assert_eq!(render_classid("account_analytic_line"), Some(0x0103_0002));
        assert_eq!(render_classid("res_partner"), Some(0x0204_0002));
        assert_eq!(render_classid("ir_cron"), None);
        assert_eq!(ODOO_APP_PREFIX, 0x0002);
    }

    #[test]
    fn schema_classids_resolves_every_table_in_order() {
        let schema = Schema {
            tables: vec![
                TableDefinition::new("account_move"),
                TableDefinition::new("account_analytic_line"),
                TableDefinition::new("ir_cron"),
            ],
            functions: Vec::new(),
            events: Vec::new(),
        };
        let ids = schema_classids(&schema);
        assert_eq!(
            ids,
            vec![
                ("account_move".to_string(), Some(0x0202)),
                ("account_analytic_line".to_string(), Some(0x0103)),
                ("ir_cron".to_string(), None),
            ]
        );
    }

    // ── emit_via_ogar_annotated ─────────────────────────────────────────

    #[test]
    fn emit_via_ogar_annotated_stamps_classid_into_comment_clause() {
        let schema = Schema {
            tables: vec![
                TableDefinition::new("account_move"),
                TableDefinition::new("ir_cron"),
            ],
            functions: Vec::new(),
            events: Vec::new(),
        };
        let ddl = emit_via_ogar_annotated(&schema);
        // Codebook hit: account_move carries the canonical concept NAME +
        // render classid 0x0202_0002 in a catalog COMMENT (single-quoted
        // SurrealQL literal), so it survives SurrealDB ingestion rather than
        // evaporating as a `--` line. The name comes from OGAR's reverse map.
        assert!(
            ddl.contains("COMMENT 'commercial_document (classid:0x02020002)'"),
            "account_move must carry its concept name + classid in a COMMENT clause; got:\n{ddl}"
        );
        // Codebook miss: ir_cron stays unstamped — exactly one classid clause.
        assert_eq!(
            ddl.matches("classid:").count(),
            1,
            "only the codebook table (account_move) is stamped, not ir_cron; got:\n{ddl}"
        );
    }

    #[test]
    fn emit_via_ogar_annotated_is_additive_over_native_emit() {
        // For a wholly-uncodified schema, no classid is stamped, so the
        // annotated emit is byte-identical to the native parallel emit — the
        // stamp is strictly additive, never a structural rewrite.
        let schema = Schema {
            tables: vec![TableDefinition::new("ir_cron")],
            functions: Vec::new(),
            events: Vec::new(),
        };
        assert_eq!(
            emit_via_ogar_annotated(&schema),
            emit_via_ogar(&schema),
            "an uncodified-only schema must emit identical DDL via both paths"
        );
    }

    // ── Substrate-input lowering (Phase 2 Stage B) ──────────────────────

    #[test]
    fn emit_source_via_ogar_lowers_odoo_source_to_annotated_ddl() {
        // The substrate-input path end to end: Odoo .py source -> ruff_python_spo
        // -> compile_graph_python::<OdooPort> -> shared ogar-adapter-surrealql. A
        // minimal account.move with a scalar (char), a Many2one, and a One2many
        // — also exercises the Stage-A array<record<…>>.
        const SRC: &str = r#"
from odoo import models, fields


class AccountMove(models.Model):
    _name = 'account.move'
    name = fields.Char(required=True)
    partner_id = fields.Many2one('res.partner')
    line_ids = fields.One2many('account.move.line', 'move_id')
"#;
        let ddl = emit_source_via_ogar(SRC);
        // Codebook identity rides into the catalog COMMENT via the minted facet
        // + OGAR's reverse map (account.move -> COMMERCIAL_DOCUMENT 0x0202_0002).
        assert!(
            ddl.contains("COMMENT 'commercial_document (classid:0x02020002)'"),
            "missing annotated COMMENT; got:\n{ddl}"
        );
        assert!(
            ddl.contains("DEFINE TABLE account_move"),
            "missing table; got:\n{ddl}"
        );
        // Many2one -> owning-side record<res_partner>; the comodel is normalized
        // to TABLE form (matching the DEFINE TABLE name), not left dotted (P2).
        assert!(
            ddl.contains("record<res_partner>"),
            "Many2one record<res_partner> missing / not normalized; got:\n{ddl}"
        );
        // One2many -> Stage-A array<record<account_move_line>> (normalized).
        assert!(
            ddl.contains("array<record<account_move_line>>"),
            "One2many array<record<account_move_line>> missing; got:\n{ddl}"
        );
        // The raw dotted comodel must NOT leak into the DDL (P2 regression).
        assert!(
            !ddl.contains("`res.partner`") && !ddl.contains("`account.move.line`"),
            "dotted comodel leaked into the DDL; got:\n{ddl}"
        );
    }

    #[test]
    fn compile_source_skips_unparseable_and_resolves_classids() {
        // Parse failure contributes nothing; a parseable codebook model mints
        // its render classid through the facet.
        assert!(compile_source("class Broken(:\n").is_empty());
        let compiled = compile_source(
            "from odoo import models, fields\n\n\nclass AM(models.Model):\n    _name = 'account.move'\n    name = fields.Char()\n",
        );
        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].facet.facet_classid(), 0x0202_0002);
    }

    /// AT-CONSUME (W3.3 delete gate, `docs/W3.3-DELETE-GATE-MATRIX.md`): the
    /// DO-arm that OGAR #164 (AT-CARRY-1) put on `CompiledClass` is actually
    /// consumed on this side — a live-source compile carries one `ActionDef`
    /// per method with its body facts, and the lifecycle classification agrees
    /// with the corpus-side mirror (`corpus_to_actions`'s prefix convention,
    /// `MethodKind::classify`). Before #164 `compile_source` dropped the whole
    /// behaviour arm; deleting the native fork would have lost the reactive
    /// wiring with no consumer ever noticing.
    #[test]
    fn compile_source_carries_the_do_arm() {
        let compiled = compile_source(concat!(
            "from odoo import api, models, fields\n\n\n",
            "class AM(models.Model):\n",
            "    _name = 'account.move'\n",
            "    amount_total = fields.Monetary(compute='_compute_amount')\n\n",
            "    @api.depends('line_ids.balance')\n",
            "    def _compute_amount(self):\n",
            "        for move in self:\n",
            "            move.amount_total = sum(move.line_ids.mapped('balance'))\n",
        ));
        assert_eq!(compiled.len(), 1);
        let cc = &compiled[0];

        // The DO-arm rides the compiled class (AT-CARRY-1 consumed).
        assert_eq!(cc.actions.len(), 1, "one ActionDef per harvested method");
        let act = &cc.actions[0];
        assert_eq!(act.predicate, "_compute_amount");
        // the frontend normalizes `_name = 'account.move'` to the table form
        assert_eq!(act.object_class, "account_move");
        assert!(
            act.identity.ends_with("::action_def::_compute_amount"),
            "identity carries the action-def address, got {}",
            act.identity
        );

        // Classification parity with the corpus-side mirror: the carried
        // predicate classifies as Compute under the same prefix convention
        // `corpus_to_actions` uses — the two arms can never silently drift.
        assert_eq!(
            crate::MethodKind::classify(&act.predicate),
            crate::MethodKind::Compute,
            "carried DO-arm predicate must classify as the corpus arm would"
        );

        // The THINK arm still carries the reactive schema half alongside.
        assert!(
            !cc.class.computed_fields.is_empty(),
            "computed_fields (THINK arm) and actions (DO arm) travel together"
        );
    }

    /// The foreign-consumer SDK (OGAR #177, `E-AR-DIRECT-SDK`): a
    /// `CompiledClass` materializes to a native-language class in Python / C# /
    /// Rust — no bridge, no serialization, the classid + typed fields ride
    /// straight into the target language. This pins the **Odoo → SDK** path:
    /// an Odoo model source lowered through the substrate emits a usable SDK
    /// class in each language, so a Python or C# consumer of Odoo models needs
    /// only the emitted dataclass, never SurrealQL (deprecated) or a bridge.
    #[test]
    fn odoo_source_materializes_to_the_foreign_consumer_sdk() {
        use ogar_from_ruff::emit::{emit_csharp, emit_python, emit_rust};

        let compiled = compile_source(concat!(
            "from odoo import models, fields\n\n\n",
            "class AM(models.Model):\n",
            "    _name = 'account.move'\n",
            "    name = fields.Char()\n",
            "    partner_id = fields.Many2one('res.partner')\n",
        ));
        assert_eq!(compiled.len(), 1);
        let cc = &compiled[0];

        // Python SDK: an `@dataclass` carrying the canon-high classid and the
        // typed fields (the association renders as a `ToOne[...]` relation).
        let py = emit_python(cc);
        assert!(py.contains("@dataclass"), "python SDK is a dataclass:\n{py}");
        assert!(
            py.contains("CLASSID: ClassVar[int] = 0x02020002"),
            "python SDK carries the canon-high classid:\n{py}"
        );
        assert!(py.contains("name:"), "python SDK carries the scalar field:\n{py}");
        assert!(
            py.contains("partner_id:"),
            "python SDK carries the association field:\n{py}"
        );

        // C# SDK: the same class materialized for the .NET consumer.
        let cs = emit_csharp(cc);
        assert!(
            cs.contains("0x02020002"),
            "c# SDK carries the canon-high classid:\n{cs}"
        );

        // Rust SDK: the sibling emitter — the materialized struct the Rust
        // consumer would use (distinct from the lance-graph V3 row sink; this
        // is the typed API surface, that is the storage row).
        let rs = emit_rust(cc);
        assert!(
            rs.contains("struct") && rs.contains("0x02020002"),
            "rust SDK materializes a struct with the classid:\n{rs}"
        );
    }
}
