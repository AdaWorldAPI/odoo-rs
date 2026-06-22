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

use ogar_vocab::ports::{OdooPort, PortSpec};
use ogar_vocab::{Association, AssociationKind, Attribute, Class};

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
#[must_use]
pub fn emit_via_ogar(schema: &Schema) -> String {
    ogar_adapter_surrealql::emit_surrealql_ddl(&schema_to_classes(schema))
}

/// [`emit_via_ogar`] DDL, prefixed with a `SurrealQL` comment header that
/// stamps each table's canonical OGAR render classid (`0xAABBCCDD`, the
/// full APP‖class id from [`render_classid`]). Tables outside the codebook
/// are listed as `(uncodified)` so the header is a complete table census,
/// not a silent subset. The header is pure `SurrealQL` line-comments (`--`),
/// so the output is still valid DDL — the classids ride alongside, making
/// the emitted schema self-describing about which shared concept each
/// table denotes.
#[must_use]
pub fn emit_via_ogar_annotated(schema: &Schema) -> String {
    use std::fmt::Write as _;
    let mut header = String::from("-- OGAR canonical classids (APP 0x0002 = Odoo render lens)\n");
    for table in &schema.tables {
        match render_classid(&table.name) {
            // `writeln!` into a String is infallible; the `let _` discards the
            // `fmt::Result` without an `unwrap` (clippy `format_push_string`).
            Some(id) => {
                let _ = writeln!(header, "-- classid {} = 0x{id:08X}", table.name);
            }
            None => {
                let _ = writeln!(header, "-- classid {} = (uncodified)", table.name);
            }
        }
    }
    header.push('\n');
    header.push_str(&emit_via_ogar(schema));
    header
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
/// `APP-CLASS-CODEBOOK-LAYOUT` (`classid = APP(hi) ‖ concept(lo)`). The low
/// `u16` is the shared cross-app concept (WHAT it is — RBAC + ontology); the
/// high `u16` is the per-app render lens (WHOSE template). `0x0002` is Odoo's
/// lens, so every Odoo-rendered id is `0x0002_<concept>`.
pub const ODOO_APP_PREFIX: u16 = 0x0002;

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
/// ([`ODOO_APP_PREFIX`], `0x0002`) in the high `u16`, the shared canonical
/// [`concept_classid`] in the low `u16`. `account_move` → `0x0002_0202`;
/// `account_analytic_line` → `0x0002_0103`. `None` for an unaliased model.
#[must_use]
pub fn render_classid(model: &str) -> Option<u32> {
    concept_classid(model).map(|lo| (u32::from(ODOO_APP_PREFIX) << 16) | u32::from(lo))
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
        assert_eq!(render_classid("account_move"), Some(0x0002_0202));
        assert_eq!(render_classid("account_analytic_line"), Some(0x0002_0103));
        assert_eq!(render_classid("res_partner"), Some(0x0002_0204));
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
    fn emit_via_ogar_annotated_stamps_classid_header() {
        let schema = Schema {
            tables: vec![
                TableDefinition::new("account_move"),
                TableDefinition::new("ir_cron"),
            ],
            functions: Vec::new(),
            events: Vec::new(),
        };
        let out = emit_via_ogar_annotated(&schema);
        // Codebook hit: account_move render classid = 0x0002_0202
        assert!(
            out.contains("-- classid account_move = 0x00020202"),
            "expected render-classid stamp for account_move; got:\n{out}"
        );
        // Codebook miss: ir_cron is not in OdooPort aliases
        assert!(
            out.contains("-- classid ir_cron = (uncodified)"),
            "expected (uncodified) for ir_cron; got:\n{out}"
        );
        // DDL body still follows the header
        assert!(
            out.contains("DEFINE TABLE"),
            "expected DDL body after header; got:\n{out}"
        );
    }

    #[test]
    fn emit_via_ogar_annotated_is_valid_ddl_prefix() {
        let schema = Schema {
            tables: vec![
                TableDefinition::new("account_move"),
                TableDefinition::new("ir_cron"),
            ],
            functions: Vec::new(),
            events: Vec::new(),
        };
        // The annotation is purely additive: the native emit is a suffix.
        let native = emit_via_ogar(&schema);
        let annotated = emit_via_ogar_annotated(&schema);
        assert!(
            annotated.contains(&native),
            "emit_via_ogar output must appear as-is inside emit_via_ogar_annotated output"
        );
    }
}
