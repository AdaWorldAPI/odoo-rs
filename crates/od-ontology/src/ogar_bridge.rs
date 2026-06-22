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
}
