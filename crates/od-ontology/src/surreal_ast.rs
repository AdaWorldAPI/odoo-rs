//! > ⚠️ **PRE-FLIGHT (OGAR #99 — SurrealQL-AST trap).** Before treating this
//! > AST as the IR / spine — or adding a `From<Ddl…> for ActionDef`, a
//! > behavioral roundtrip claim, or `DEFINE EVENT` lifecycle — read
//! > `specs/SURREAL-AST-TRAP.md` (90-second Q1–Q5 mirror). This file is an
//! > **egress adapter, not a spine.** Structure lowers through
//! > `ogar_vocab::Class` (DDL is a *lossy projection* of it, one-way); behavior
//! > (compute / `@api.constrains` / actions) has **no DDL home** and lowers to
//! > OGAR's `ActionDef` arm, never `DEFINE EVENT`. The mapping table below is
//! > the structural projection only — it is not a claim that lifecycle round-
//! > trips through DDL.
//!
//! Typed SurrealQL DDL AST — the shape Odoo's ontology lowers into.
//!
//! This is **not** a migration target and **not** an ORM schema. It is the
//! typed surface on which Odoo's runtime semantics become *native SurrealDB
//! constructs*:
//!
//! | Odoo construct                       | SurrealQL DDL node            |
//! | ---                                  | ---                           |
//! | `class X(models.Model)`              | [`TableDefinition`]           |
//! | `fields.Char(required=True)`         | [`FieldDefinition`] `string`  |
//! | `fields.Many2one('res.partner')`     | [`FieldDefinition`] `record<…>` |
//! | `compute='_x', store=True`           | [`FieldDefinition::value`] + `READONLY` |
//! | `@api.depends('a','b')` (same row)   | recompute-on-write of the `VALUE` field |
//! | `@api.depends('rel.sub')` (cross row)| [`EventDefinition`] on the child table |
//! | `@api.constrains` / `_check_*`       | [`EventDefinition`] guard with `THROW` |
//! | `def action_post(self)`              | [`FunctionDefinition`]        |
//! | `_sql_constraints unique(...)`       | [`IndexDefinition`] `UNIQUE`  |
//!
//! Mirrors the DDL-relevant slots of surrealdb-core's `catalog::*` so a future
//! `From<od_ontology::TableDefinition> for catalog::TableDefinition` is
//! mechanical, but stays zero-dep on surrealdb-core (which pulls tokio +
//! rust-1.95 + optional rocksdb — disproportionate for "emit DDL").
//!
//! [`ToSql`] renders every node as real SurrealQL.

use std::fmt::Write as _;

/// Render a typed AST node as a SurrealQL string fragment.
pub trait ToSql {
    /// Append this node's SurrealQL form to `out`.
    fn fmt_sql(&self, out: &mut String);

    /// Render this node as a standalone SurrealQL string.
    #[must_use]
    fn to_sql(&self) -> String {
        let mut s = String::new();
        self.fmt_sql(&mut s);
        s
    }
}

/// A SurrealQL field-type expression. Grown one variant per Odoo field family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `any` — untyped slot (the honest default when the field's Odoo type is
    /// not yet lifted from the typed `OdooEntity` consts).
    Any,
    /// `bool` — `fields.Boolean`.
    Bool,
    /// `int` — `fields.Integer`.
    Int,
    /// `float` — `fields.Float`.
    Float,
    /// `decimal` — `fields.Monetary`.
    Decimal,
    /// `string` — `fields.Char` / `fields.Text` / `fields.Selection`.
    String,
    /// `datetime` — `fields.Date` / `fields.Datetime`.
    Datetime,
    /// `record<a|b|c>` — `fields.Many2one` (single target today; the vector
    /// supports polymorphic `reference` fields without a layout change).
    Record(Vec<String>),
    /// `array<inner>` — `fields.One2many` / `fields.Many2many`.
    Array(Box<Kind>),
    /// `option<inner>` — a non-`required` field (Odoo fields are nullable
    /// unless `required=True`).
    Option(Box<Kind>),
}

impl Kind {
    /// Wrap in `option<…>`, normalising `option<option<T>>` → `option<T>`.
    #[must_use]
    pub fn optional(self) -> Self {
        match self {
            Self::Option(_) => self,
            other => Self::Option(Box::new(other)),
        }
    }
}

impl ToSql for Kind {
    fn fmt_sql(&self, out: &mut String) {
        match self {
            Self::Any => out.push_str("any"),
            Self::Bool => out.push_str("bool"),
            Self::Int => out.push_str("int"),
            Self::Float => out.push_str("float"),
            Self::Decimal => out.push_str("decimal"),
            Self::String => out.push_str("string"),
            Self::Datetime => out.push_str("datetime"),
            Self::Record(targets) => {
                out.push_str("record<");
                for (i, t) in targets.iter().enumerate() {
                    if i != 0 {
                        out.push('|');
                    }
                    out.push_str(t);
                }
                out.push('>');
            }
            Self::Array(inner) => {
                out.push_str("array<");
                inner.fmt_sql(out);
                out.push('>');
            }
            Self::Option(inner) => {
                out.push_str("option<");
                inner.fmt_sql(out);
                out.push('>');
            }
        }
    }
}

/// One `DEFINE FIELD …`. Carries the reactive `VALUE`, the `ASSERT` guard, and
/// the same-record dependency set (rendered as an inline `--` audit comment so
/// the reactive wiring is visible in the emitted DDL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDefinition {
    /// Owning table (Odoo model, dots → underscores).
    pub table: String,
    /// Field name.
    pub name: String,
    /// Declared type.
    pub kind: Kind,
    /// `VALUE <expr>` — set for computed fields (`compute=`). The expression
    /// calls the field's `_compute_*` function. `None` for stored scalars.
    pub value: Option<String>,
    /// `ASSERT <expr>` — a field-level invariant (`required=True`,
    /// single-field `@api.constrains`).
    pub assert: Option<String>,
    /// `READONLY` — a stored computed field (`store=True`) the user can't set.
    pub readonly: bool,
    /// The same-record `@api.depends` set this field recomputes over. Rendered
    /// as an audit comment; SurrealDB recomputes `VALUE` on every write so no
    /// extra wiring is needed for the same-record case.
    pub reactive_over: Vec<String>,
}

impl FieldDefinition {
    /// A plain, nullable, untyped field on `table`.
    #[must_use]
    pub fn new(table: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            name: name.into(),
            kind: Kind::Any.optional(),
            value: None,
            assert: None,
            readonly: false,
            reactive_over: Vec::new(),
        }
    }
}

impl ToSql for FieldDefinition {
    fn fmt_sql(&self, out: &mut String) {
        if !self.reactive_over.is_empty() {
            let _ = writeln!(out, "-- reactive over: {}", self.reactive_over.join(", "));
        }
        let _ = write!(out, "DEFINE FIELD {} ON {} TYPE ", self.name, self.table);
        self.kind.fmt_sql(out);
        if let Some(v) = &self.value {
            let _ = write!(out, " VALUE {v}");
        }
        if let Some(a) = &self.assert {
            let _ = write!(out, " ASSERT {a}");
        }
        if self.readonly {
            out.push_str(" READONLY");
        }
        out.push(';');
    }
}

/// One `DEFINE INDEX … UNIQUE` — an Odoo `_sql_constraints` unique tuple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDefinition {
    /// Owning table.
    pub table: String,
    /// Index name.
    pub name: String,
    /// Columns covered, in order.
    pub fields: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
}

impl ToSql for IndexDefinition {
    fn fmt_sql(&self, out: &mut String) {
        let _ = write!(
            out,
            "DEFINE INDEX {} ON {} FIELDS {}",
            self.name,
            self.table,
            self.fields.join(", ")
        );
        if self.unique {
            out.push_str(" UNIQUE");
        }
        out.push(';');
    }
}

/// One `DEFINE EVENT …`. Two Odoo origins:
/// 1. a cross-record `@api.depends('rel.sub')` → recompute the parent on child write;
/// 2. an `@api.constrains` / `_check_*` guard → `THROW` on a violated invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDefinition {
    /// Event name.
    pub name: String,
    /// Table the event fires on.
    pub table: String,
    /// `WHEN <cond>` clause.
    pub when: String,
    /// `THEN { <body> }` block.
    pub then: String,
    /// Provenance note (rendered as a leading `--` comment).
    pub note: Option<String>,
}

impl ToSql for EventDefinition {
    fn fmt_sql(&self, out: &mut String) {
        if let Some(n) = &self.note {
            let _ = writeln!(out, "-- {n}");
        }
        let _ = write!(
            out,
            "DEFINE EVENT {} ON {} WHEN {} THEN {{ {} }};",
            self.name, self.table, self.when, self.then
        );
    }
}

/// One `DEFINE FUNCTION fn::<model>::<method>(…)`. The Odoo method body is
/// Python; this emits a typed stub with the correct signature and a deferred
/// body — the *wiring* is faithful immediately, the *logic* ports incrementally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDefinition {
    /// Owning model.
    pub model: String,
    /// Method name (`_compute_amount`, `action_post`, `_check_dates`).
    pub method: String,
    /// Typed parameters (always at least `$this: record<model>`).
    pub params: Vec<(String, Kind)>,
    /// Body source. Defaults to a deferred stub returning `NONE`.
    pub body: String,
}

impl FunctionDefinition {
    /// A deferred-body stub for `model::method` taking `$this: record<model>`.
    #[must_use]
    pub fn stub(model: impl Into<String>, method: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            params: vec![("this".to_string(), Kind::Record(vec![model.clone()]))],
            model,
            method: method.into(),
            body: "/* deferred: port from Python */ RETURN NONE;".to_string(),
        }
    }
}

impl ToSql for FunctionDefinition {
    fn fmt_sql(&self, out: &mut String) {
        let _ = write!(out, "DEFINE FUNCTION fn::{}::{}(", self.model, self.method);
        for (i, (pname, pkind)) in self.params.iter().enumerate() {
            if i != 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "${pname}: ");
            pkind.fmt_sql(out);
        }
        let _ = write!(out, ") {{ {} }};", self.body);
    }
}

/// One `DEFINE TABLE …` plus its `DEFINE FIELD` / `DEFINE INDEX` children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDefinition {
    /// Table name (Odoo model, dots → underscores).
    pub name: String,
    /// `SCHEMAFULL` vs `SCHEMALESS`. Odoo models are schema-bound → `true`.
    pub schemafull: bool,
    /// Field children, deterministic order.
    pub fields: Vec<FieldDefinition>,
    /// Index children, deterministic order.
    pub indices: Vec<IndexDefinition>,
}

impl TableDefinition {
    /// A new `SCHEMAFULL TYPE NORMAL` table with no children.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schemafull: true,
            fields: Vec::new(),
            indices: Vec::new(),
        }
    }
}

impl ToSql for TableDefinition {
    fn fmt_sql(&self, out: &mut String) {
        let _ = write!(out, "DEFINE TABLE {} ", self.name);
        out.push_str(if self.schemafull {
            "SCHEMAFULL"
        } else {
            "SCHEMALESS"
        });
        out.push_str(" TYPE NORMAL;");
        for f in &self.fields {
            out.push('\n');
            f.fmt_sql(out);
        }
        for i in &self.indices {
            out.push('\n');
            i.fmt_sql(out);
        }
    }
}

/// A full Odoo-ontology schema: tables (+ their fields/indices), the function
/// catalogue, and the reactive/guard events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    /// Table definitions.
    pub tables: Vec<TableDefinition>,
    /// `DEFINE FUNCTION` catalogue (compute methods + actions + checks).
    pub functions: Vec<FunctionDefinition>,
    /// `DEFINE EVENT` set (cross-record recompute + guards).
    pub events: Vec<EventDefinition>,
}

impl ToSql for Schema {
    fn fmt_sql(&self, out: &mut String) {
        for (i, t) in self.tables.iter().enumerate() {
            if i != 0 {
                out.push('\n');
            }
            t.fmt_sql(out);
            out.push('\n');
        }
        if !self.functions.is_empty() {
            out.push_str("\n-- ── functions (bodies deferred) ──\n");
            for f in &self.functions {
                f.fmt_sql(out);
                out.push('\n');
            }
        }
        if !self.events.is_empty() {
            out.push_str("\n-- ── reactive + guard events ──\n");
            for e in &self.events {
                e.fmt_sql(out);
                out.push('\n');
            }
        }
    }
}
