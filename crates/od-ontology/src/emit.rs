//! The corpus → ontology-shape projection.
//!
//! Turns the SPO triple corpus into a typed [`Schema`] of native SurrealDB
//! constructs. The projection is split exactly along the
//! *wiring-faithful-now / bodies-deferred* line:
//!
//! - **Faithful immediately** (100% from the triples): which table exists,
//!   which field is computed by which function, the same-record reactive
//!   dependency set, which methods are guards, which deps cross records.
//! - **Deferred** (stubbed, ports incrementally): the compute/guard *bodies*
//!   (Python expressions), exact field types beyond the name heuristic, and
//!   cross-record child-table resolution.
//!
//! The point of slice 1 (`account.move`) is that the *reactive graph* — the
//! thing Odoo actually is — lands as SurrealDB `DEFINE` statements, not as
//! boilerplate in a Rust binary.

use std::collections::BTreeMap;

use crate::surreal_ast::{
    EventDefinition, FieldDefinition, FunctionDefinition, Kind, Schema, TableDefinition,
};
use crate::triple::{self, Triple};

/// Predicate constants (the closed vocabulary this projection consumes).
mod pred {
    pub const RDF_TYPE: &str = "rdf:type";
    pub const HAS_FUNCTION: &str = "has_function";
    pub const EMITTED_BY: &str = "emitted_by";
    pub const DEPENDS_ON: &str = "depends_on";
    pub const RAISES: &str = "raises";
}

/// Object constants for `rdf:type`.
mod obj {
    pub const OBJECT_TYPE: &str = "ogit:ObjectType";
    pub const PROPERTY: &str = "ogit:Property";
    pub const FUNCTION: &str = "ogit:Function";
}

/// Project the triple corpus into a SurrealQL [`Schema`].
///
/// When `focus` is `Some(model)`, only that model's tables/fields/functions/
/// events are emitted (the slice-1 driver passes `Some("account_move")`).
#[must_use]
pub fn corpus_to_schema(triples: &[Triple], focus: Option<&str>) -> Schema {
    let in_focus = |iri: &str| focus.is_none_or(|m| triple::model_of(iri) == m);

    // field IRI → the `_compute_*` method IRI that materialises it.
    let mut computed_by: BTreeMap<&str, &str> = BTreeMap::new();
    for t in triples {
        if t.p == pred::EMITTED_BY {
            computed_by.insert(&t.s, &t.o);
        }
    }

    // model → table (BTreeMap = deterministic output order).
    let mut tables: BTreeMap<String, TableDefinition> = BTreeMap::new();
    let mut functions: Vec<FunctionDefinition> = Vec::new();

    // 1. Tables + 2. Functions + 3. Fields, from the rdf:type structural edges.
    for t in triples {
        if t.p != pred::RDF_TYPE || !in_focus(&t.s) {
            continue;
        }
        let model = triple::model_of(&t.s).to_string();
        match t.o.as_str() {
            obj::OBJECT_TYPE => {
                tables
                    .entry(model.clone())
                    .or_insert_with(|| TableDefinition::new(model));
            }
            obj::FUNCTION => {
                if let Some(method) = triple::member_of(&t.s) {
                    functions.push(FunctionDefinition::stub(model, method));
                }
            }
            obj::PROPERTY => {
                let Some(field) = triple::member_of(&t.s) else {
                    continue;
                };
                let mut fd = FieldDefinition::new(&model, field);
                fd.kind = infer_kind(field);
                // computed field → VALUE + READONLY (store=True compute).
                if let Some(computer) = computed_by.get(t.s.as_str()) {
                    if let Some(method) = triple::member_of(computer) {
                        fd.value = Some(format!("fn::{model}::{method}($this)"));
                        fd.readonly = true;
                    }
                }
                tables
                    .entry(model.clone())
                    .or_insert_with(|| TableDefinition::new(model.clone()))
                    .fields
                    .push(fd);
            }
            _ => {}
        }
    }

    // 4. Reactive dependency wiring (depends_on).
    //    same-record  → audit comment on the computed field's VALUE recompute.
    //    cross-record → one DEFINE EVENT per (relation, parent-model).
    let mut cross: BTreeMap<(String, String), Vec<(String, String)>> = BTreeMap::new();
    for t in triples {
        if t.p != pred::DEPENDS_ON || !in_focus(&t.s) {
            continue;
        }
        let (Some(parent_field), Some(dep)) = (triple::member_of(&t.s), triple::member_of(&t.o))
        else {
            continue;
        };
        if triple::is_cross_record(dep) {
            if let Some((rel, leaf)) = triple::first_hop(dep) {
                let model = triple::model_of(&t.s).to_string();
                cross
                    .entry((model, rel.to_string()))
                    .or_default()
                    .push((leaf.to_string(), parent_field.to_string()));
            }
        } else {
            // same-record: attach to the parent field's audit list.
            let model = triple::model_of(&t.s);
            if let Some(tbl) = tables.get_mut(model) {
                if let Some(f) = tbl.fields.iter_mut().find(|f| f.name == parent_field) {
                    if !f.reactive_over.contains(&dep.to_string()) {
                        f.reactive_over.push(dep.to_string());
                    }
                }
            }
        }
    }

    let mut events: Vec<EventDefinition> = Vec::new();

    // Cross-record reactive events: when a child relation's leaf changes,
    // recompute the dependent parent fields. Child-table resolution + the
    // back-reference are the declared deferred bits (annotated inline).
    for ((model, rel), deps) in cross {
        let mut leaves: Vec<&String> = deps.iter().map(|(leaf, _)| leaf).collect();
        leaves.sort_unstable();
        leaves.dedup();
        let mut parents: Vec<&String> = deps.iter().map(|(_, p)| p).collect();
        parents.sort_unstable();
        parents.dedup();
        let leaf_clause = leaves
            .iter()
            .map(|l| format!("$before.{l} != $after.{l}"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let recompute = parents
            .iter()
            .map(|p| format!("{p} = fn::{model}::_recompute_{p}($parent)"))
            .collect::<Vec<_>>()
            .join(", ");
        events.push(EventDefinition {
            name: format!("recompute_{model}_via_{rel}"),
            // TODO(child-table): resolve `rel` → its target model + back-ref.
            table: format!("{model}__{rel}"),
            when: format!("$event = \"UPDATE\" AND ({leaf_clause})"),
            then: format!("/* resolve child→parent back-ref */ UPDATE $parent SET {recompute}"),
            note: Some(format!(
                "cross-record @api.depends: {model}.{rel}.* → recompute [{}]",
                parents
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        });
    }

    // 5. Guards: `_check_*` methods that `raises` → DEFINE EVENT with THROW.
    for t in triples {
        if t.p != pred::RAISES || !in_focus(&t.s) {
            continue;
        }
        let Some(method) = triple::member_of(&t.s) else {
            continue;
        };
        if !is_guard(method) {
            continue;
        }
        let model = triple::model_of(&t.s).to_string();
        let exc = triple::strip_ns(&t.o);
        events.push(EventDefinition {
            name: method.trim_start_matches('_').to_string(),
            table: model.clone(),
            when: "$event IN [\"CREATE\", \"UPDATE\"]".to_string(),
            then: format!(
                "IF !fn::{model}::{method}($after) {{ THROW \"{exc}: {method} failed\" }}"
            ),
            note: Some(format!("@api.constrains guard → {exc}")),
        });
    }

    // Deterministic order for stable output / diffs.
    functions.sort_by(|a, b| (&a.model, &a.method).cmp(&(&b.model, &b.method)));
    functions.dedup_by(|a, b| a.model == b.model && a.method == b.method);
    events.sort_by(|a, b| (&a.table, &a.name).cmp(&(&b.table, &b.name)));

    Schema {
        tables: tables.into_values().collect(),
        functions,
        events,
    }
}

/// Whether a method name is an `@api.constrains` guard (`_check_*`).
fn is_guard(method: &str) -> bool {
    method.starts_with("_check_") || method.starts_with("_constrain")
}

/// Name-heuristic field type (slice-1 stand-in for the typed `OdooEntity`
/// consts). `*_ids` → `array<record<…>>`; `*_id` → `record<…>`; everything
/// else → `option<any>` with the type left for the `OdooEntity` lift.
///
/// All non-relation fields are nullable (`option<…>`) — Odoo fields are
/// optional unless `required=True`, which is not in this triple slice.
fn infer_kind(field: &str) -> Kind {
    if let Some(rel) = field.strip_suffix("_ids") {
        Kind::Array(Box::new(Kind::Record(vec![relation_target(rel)]))).optional()
    } else if let Some(rel) = field.strip_suffix("_id") {
        Kind::Record(vec![relation_target(rel)]).optional()
    } else {
        Kind::Any.optional()
    }
}

/// Best-effort relation → target-table name. The exact target lives in the
/// typed `OdooEntity` `Many2one`/`One2many` decorator; until that's wired the
/// relation stem is the placeholder (e.g. `partner` → `partner`, resolved to
/// `res_partner` in the `OdooEntity` lift).
fn relation_target(rel: &str) -> String {
    // singularise a trailing `s` from a `_ids` stem (`line` already singular).
    rel.to_string()
}
