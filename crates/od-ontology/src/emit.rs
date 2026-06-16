//! The corpus → ontology-shape projection.
//!
//! Turns the SPO triple corpus into a typed [`Schema`] of native SurrealDB
//! constructs. The projection is split exactly along the
//! *wiring-faithful-now / bodies-deferred* line:
//!
//! - **Faithful immediately** (100% from the triples): which table exists,
//!   which field is computed by which function, the same-record reactive
//!   dependency set, which methods are guards, which deps cross records,
//!   **and which child table the cross-record event targets when the child
//!   is in the focus set** (slice-2 lift).
//! - **Deferred** (stubbed, ports incrementally): the compute/guard *bodies*
//!   (Python expressions); exact field types for non-relation scalars; child
//!   tables that fall outside the focus set (audited as TODO inline).
//!
//! # Multi-model focus + back-ref resolution
//!
//! `corpus_to_schema(triples, focus)` accepts a *set* of models. Within that
//! focus set, the projection resolves relation targets by name:
//! `partner_id` on `account_move` finds `res_partner` if it's in the focus
//! (via the `res_<stem>` Odoo convention); `line_ids` finds `account_move_line`
//! (via the `<parent>_<stem>` convention). The convention ladder is:
//!
//! 1. exact: stem itself is a focus model
//! 2. `res_<stem>` (the Odoo namespace for `res.*` master data)
//! 3. `<parent>_<stem>` (parent-suffixed children, e.g. `account_move_line`)
//!
//! First hit wins. A miss falls back to the bare stem and is audited inline.

use std::collections::{BTreeMap, BTreeSet};

use crate::surreal_ast::{
    EventDefinition, FieldDefinition, FunctionDefinition, Kind, Schema, TableDefinition,
};
use crate::triple::{self, Triple};

/// Predicate constants (the closed vocabulary this projection consumes).
mod pred {
    pub const RDF_TYPE: &str = "rdf:type";
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
/// `focus = Some(&["account_move", "account_move_line", …])` restricts the
/// projection to the named models *and* enables back-ref resolution within the
/// set. `focus = None` emits every model in the corpus (no within-set resolver
/// — bare stems fall back).
///
/// The output ordering is deterministic (tables by name, fields by emission
/// order, functions/events by `(model, name)`); the same `(triples, focus)`
/// always emits the same DDL.
#[must_use]
pub fn corpus_to_schema(triples: &[Triple], focus: Option<&[&str]>) -> Schema {
    let in_focus: Box<dyn Fn(&str) -> bool> = match focus {
        Some(set) => {
            let set: BTreeSet<&str> = set.iter().copied().collect();
            Box::new(move |iri| set.contains(triple::model_of(iri)))
        }
        None => Box::new(|_| true),
    };

    // The focus set as model names — used for back-ref resolution.
    let focus_set: BTreeSet<String> = focus
        .map(|s| s.iter().map(|m| (*m).to_string()).collect())
        .unwrap_or_default();

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
                fd.kind = infer_kind(field, &model, &focus_set);
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
    // recompute the dependent parent fields on the parent table.
    for ((parent_model, rel), deps) in cross {
        let resolved = resolve_target(&rel, &parent_model, &focus_set);
        let child_table = match &resolved {
            Some(t) => t.clone(),
            None => format!("{parent_model}__{rel}"), // unresolved placeholder
        };

        // The conventional Odoo back-reference column name. When the child is
        // in focus we trust the convention; if it ever diverges (Odoo's
        // `account.move.line.move_id` vs the parent `account.move` is exactly
        // this case — `move_id`, not `account_move_id`), the value falls
        // through to the recorded child→parent column on the OdooEntity const
        // (deferred wiring).
        let back_ref = back_ref_name(&parent_model);

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
            .map(|p| format!("{p} = fn::{parent_model}::_recompute_{p}($parent)"))
            .collect::<Vec<_>>()
            .join(", ");

        let then = if resolved.is_some() {
            format!("LET $parent = $after.{back_ref}; UPDATE $parent SET {recompute}")
        } else {
            // No back-ref column known → audit instead of guess.
            format!("/* TODO: resolve child→parent back-ref for {parent_model}.{rel} */ UPDATE $parent SET {recompute}")
        };

        let note = match &resolved {
            Some(child) => format!(
                "cross-record @api.depends: {parent_model}.{rel}.* (child={child}, back-ref={back_ref}) → recompute [{}]",
                parents
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            None => format!(
                "cross-record @api.depends: {parent_model}.{rel}.* (child UNRESOLVED — not in focus set) → recompute [{}]",
                parents
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        };

        events.push(EventDefinition {
            name: format!("recompute_{parent_model}_via_{rel}"),
            table: child_table,
            when: format!("$event = \"UPDATE\" AND ({leaf_clause})"),
            then,
            note: Some(note),
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

/// Name-heuristic field type. `*_ids` → `option<array<record<resolved>>>`,
/// `*_id` → `option<record<resolved>>`, everything else → `option<any>`.
///
/// Relation targets are resolved against `focus_set` via [`resolve_target`].
/// All non-relation fields are nullable (`option<…>`) — Odoo fields are
/// optional unless `required=True`, which is not in this triple slice.
fn infer_kind(field: &str, parent_model: &str, focus_set: &BTreeSet<String>) -> Kind {
    if let Some(stem) = field.strip_suffix("_ids") {
        let target =
            resolve_target(stem, parent_model, focus_set).unwrap_or_else(|| stem.to_string());
        Kind::Array(Box::new(Kind::Record(vec![target]))).optional()
    } else if let Some(stem) = field.strip_suffix("_id") {
        let target =
            resolve_target(stem, parent_model, focus_set).unwrap_or_else(|| stem.to_string());
        Kind::Record(vec![target]).optional()
    } else {
        Kind::Any.optional()
    }
}

/// Resolve a relation name (`line_ids`, `partner_id`) or pre-stripped stem
/// (`line`, `partner`) to a target model name, using the Odoo naming
/// conventions in order: exact, `res_<stem>`, `<parent>_<stem>`.
///
/// Accepts both raw field names and stripped stems — the `_ids` / `_id`
/// suffix is removed internally if present, so callsites don't need to.
///
/// Returns `None` when no convention finds a focus-set model.
///
/// Examples (with focus = {account_move, account_move_line, res_partner, res_company}):
///
/// - `resolve_target("partner_id", "account_move", focus)` → `Some("res_partner")`
/// - `resolve_target("line_ids", "account_move", focus)` → `Some("account_move_line")`
/// - `resolve_target("move_id", "account_move_line", focus)` → `None` (`move` neither
///   matches exactly, nor `res_move`, nor `account_move_line_move` — this is an
///   Odoo naming exception that needs the typed `OdooEntity` lift; the bare-stem
///   fallback emits `record<move>` and the cross-record event audits inline.)
/// - `resolve_target("product_id", "account_move_line", focus)` → `None` (out of focus)
fn resolve_target(name: &str, parent_model: &str, focus_set: &BTreeSet<String>) -> Option<String> {
    let stem = name
        .strip_suffix("_ids")
        .or_else(|| name.strip_suffix("_id"))
        .unwrap_or(name);
    if focus_set.contains(stem) {
        return Some(stem.to_string());
    }
    let res = format!("res_{stem}");
    if focus_set.contains(&res) {
        return Some(res);
    }
    let par = format!("{parent_model}_{stem}");
    if focus_set.contains(&par) {
        return Some(par);
    }
    None
}

/// The conventional Odoo back-reference column on a child table pointing at
/// `parent_model`. Strips a leading `<namespace>_` (`account_move` → `move`,
/// `res_partner` → `partner`), then appends `_id`.
///
/// This is the convention Odoo's One2many inverse names follow
/// (`account.move.line.move_id`, `account.bank.statement.line.statement_id`,
/// `res.partner.bank.partner_id`). The naming exceptions surface as fixture
/// mismatches in tests, not silently — they'd be picked up when the typed
/// `OdooEntity` `inverse_name` lift wires in.
fn back_ref_name(parent_model: &str) -> String {
    let stem = parent_model
        .split_once('_')
        .map_or(parent_model, |(_, rest)| rest);
    format!("{stem}_id")
}
