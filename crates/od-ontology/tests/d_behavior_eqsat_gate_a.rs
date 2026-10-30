//! D-BEHAVIOR-EQSAT-0 / Gate A. Test-only, source-qualified behavioral loss.
//!
//! This deliberately reuses the #44 DR64 and DT registry helpers and #46's
//! source-only example. No new graph/schema, source interpreter or Odoo trace.
//! Source blob: data/account_move.spo.ndjson
//!   459659e1c1ece2303de2bc3c8eab715c27e6c53d (baseline #46).
//! A coarse relation proves a dependency, never effect order or authority.

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{parse_ndjson, RecomputeDag, Triple};
use serde_json::Value;

mod common;
use common::cube::{facts, registry, strip, CLASS, UNATTESTED};
use common::dr64::{compose_blocked, store, Blocked, Dom, Operand2, W};

const SOURCE: &str = include_str!("../../../data/account_move.spo.ndjson");
const EXAMPLE: &str = include_str!("../../../docs/examples/odoo-codegraph-harvest-v1.example.json");
const FIELD: &str = "account_move.currency_id";
const COMPUTE: &str = "account_move._compute_currency_id";
const ONCHANGE: &str = "account_move._onchange_invoice_vendor_bill";

fn source_writers(ts: &[Triple]) -> BTreeSet<String> {
    ts.iter()
        .filter(|t| t.p == "emitted_by" && strip(&t.s) == FIELD)
        .map(|t| strip(&t.o).to_owned())
        .collect()
}

fn exported_writers(graph: &Value) -> BTreeSet<String> {
    graph["edges"]
        .as_array()
        .expect("the existing #46 example has edges")
        .iter()
        .filter(|e| {
            e["kind"] == "emitted_by"
                && e["source"] == "field:account.move:currency_id"
        })
        .filter_map(|e| e["target"].as_str())
        .map(|s| s.replace("method:account.move:", "account_move."))
        .collect()
}

/// Compares an independent source reference with a projection. This is the
/// load-bearing admission check: an omitted emitter cannot be accepted.
fn require_all_emitters(
    expected: &BTreeSet<String>,
    projected: &BTreeSet<String>,
) -> Result<(), &'static str> {
    if expected == projected {
        Ok(())
    } else {
        Err("source_declared_emitter_mismatch")
    }
}

#[test]
fn source_codegraph_and_dt_registry_retain_both_currency_methods() {
    let ts = parse_ndjson(SOURCE).expect("pinned source corpus parses");
    let source = source_writers(&ts);
    assert_eq!(
        source,
        BTreeSet::from([COMPUTE.to_owned(), ONCHANGE.to_owned()]),
        "the source oracle has exactly these distinct declared writers"
    );

    let example: Value = serde_json::from_str(EXAMPLE).unwrap();
    assert_eq!(example["metadata"]["snapshot_kind"], "source_harvest");
    assert!(example["metadata"].get("registry_snapshot").is_none());
    let projected = exported_writers(&example);
    require_all_emitters(&source, &projected).unwrap();

    let f = facts();
    let reg = registry(&f);
    let do_position = reg.do_.get(FIELD).expect("existing DO position");
    let from_registry: BTreeSet<String> = reg
        .live(1)
        .iter()
        .filter(|((class, cell), _)| class == CLASS && cell.do_ == do_position)
        .flat_map(|(_, methods)| methods.iter().cloned())
        .collect();
    require_all_emitters(&source, &from_registry).unwrap();

    // Falsifier 1: omit either emitter, independent of JSON syntax validity.
    for dropped in [COMPUTE, ONCHANGE] {
        let mut damaged = projected.clone();
        assert!(damaged.remove(dropped));
        assert_eq!(
            require_all_emitters(&source, &damaged),
            Err("source_declared_emitter_mismatch")
        );
    }
    // Missing trigger is not evidence that the onchange does not exist.
    let unattested = reg.on.get(UNATTESTED).unwrap();
    assert!(reg.live(1).iter().any(|((_, c), methods)| {
        c.on == unattested && methods.contains(ONCHANGE)
    }));
}

type Pairs = BTreeSet<(String, String)>;

/// Independently enumerated source relation. No DR kernel, RecomputeDag,
/// registry helper, or shared join iterator participates in this oracle.
fn enumerate_dependencies(ts: &[Triple]) -> (Pairs, Vec<String>, Vec<String>, Blocked, Blocked) {
    let mut emit: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut read: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for t in ts {
        if t.p == "emitted_by" {
            emit.entry(strip(&t.s).to_owned())
                .or_default()
                .insert(strip(&t.o).to_owned());
        }
        if t.p == "reads_field" {
            read.entry(strip(&t.o).to_owned())
                .or_default()
                .insert(strip(&t.s).to_owned());
        }
    }
    let fields: Vec<String> = emit
        .keys()
        .filter(|field| read.contains_key(*field))
        .cloned()
        .collect();
    let methods: Vec<String> = fields
        .iter()
        .flat_map(|f| emit[f].iter().chain(read[f].iter()).cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut left = Blocked::new(methods.len(), fields.len());
    let mut right = Blocked::new(fields.len(), methods.len());
    let mut expected = Pairs::new();
    for (fi, field) in fields.iter().enumerate() {
        for producer in &emit[field] {
            let pi = methods.binary_search(producer).unwrap();
            left.set(pi, fi);
            for consumer in &read[field] {
                if producer != consumer {
                    expected.insert((producer.clone(), consumer.clone()));
                }
            }
        }
        for consumer in &read[field] {
            let ci = methods.binary_search(consumer).unwrap();
            right.set(fi, ci);
        }
    }
    (expected, methods, fields, left, right)
}

#[test]
fn independent_enumeration_matches_dr_but_single_emitter_dag_loses_edges() {
    let ts = parse_ndjson(SOURCE).unwrap();
    let (expected, methods, fields, emits, read_by) = enumerate_dependencies(&ts);
    assert!(methods.len() <= W && fields.len() <= W, "slice 1 fits DR64");
    let st = store();
    let e = Operand2 { rel: &emits, row: Dom::Method, col: Dom::Field, ordinal: 0 };
    let r = Operand2 { rel: &read_by, row: Dom::Field, col: Dom::Method, ordinal: 1 };
    let (composed, _) = compose_blocked(&st, &e, &r);
    let composed = composed.without_diagonal();

    assert_eq!(composed.count(), expected.len());
    for (i, from) in methods.iter().enumerate() {
        for (j, to) in methods.iter().enumerate() {
            assert_eq!(
                composed.get(i, j),
                expected.contains(&(from.clone(), to.clone())),
                "DR64 disagrees with independently enumerated {from} -> {to}"
            );
        }
    }

    let dag = RecomputeDag::from_triples(&ts);
    let missing: Pairs = expected
        .iter()
        .filter(|(from, to)| !dag.has_edge(from, to))
        .cloned()
        .collect();
    assert_eq!(dag.edge_count(), 44, "reproduce merged #44 baseline");
    assert_eq!(missing.len(), 26, "reproduce merged #44 loss, not a DR error");
    assert!(
        expected.iter().all(|(from, to)| {
            !dag.has_edge(from, to) || composed.get(
                methods.binary_search(from).unwrap(),
                methods.binary_search(to).unwrap(),
            )
        }),
        "the reduced DAG must not invent dependencies"
    );
}

#[derive(Clone, Copy)]
enum Effect {
    Set(i64),
    Add(i64),
}

fn run_ordered(initial: i64, ops: &[Effect]) -> i64 {
    ops.iter().fold(initial, |state, effect| match effect {
        Effect::Set(v) => *v,
        Effect::Add(v) => state + v,
    })
}

#[test]
fn unordered_spo_cannot_certify_effect_order_or_runtime_observation() {
    // Synthetic scalar falsifier, NOT an observed Odoo execution trace.
    let first = [Effect::Set(1), Effect::Add(1)];
    let reordered = [Effect::Add(1), Effect::Set(1)];
    assert_eq!(run_ordered(0, &first), 2);
    assert_eq!(run_ordered(0, &reordered), 1);
    // Both have the same unordered operation-kind set.
    let coarse = BTreeSet::from(["Add", "Set"]);
    assert_eq!(coarse, BTreeSet::from(["Set", "Add"]));
    // The #46 source-only example contains no Odoo runtime attestation.
    let example: Value = serde_json::from_str(EXAMPLE).unwrap();
    assert_eq!(example["metadata"]["snapshot_kind"], "source_harvest");
    assert!(example["metadata"].get("registry_snapshot").is_none());
    // None = UNKNOWN. A missing trigger/permission/effect-order witness must
    // not silently be admitted as either TRUE or FALSE.
    let trigger: Option<bool> = None;
    let permission: Option<bool> = None;
    let ordered_effects_witness: Option<bool> = None;
    assert_ne!(trigger, Some(true));
    assert_ne!(trigger, Some(false));
    assert_ne!(permission, Some(true));
    assert_ne!(ordered_effects_witness, Some(true));
    let missing = example["metadata"]["coverage"]["missing_facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(missing.contains("trigger") && missing.contains("ACL"));
}

#[test]
fn inherit_and_inherits_cannot_be_disambiguated_from_coarse_predicate() {
    // The existing Ruff/Odoo upstream lift maps both source forms to the
    // same `inherits_from` predicate (od-ontology/src/inheritance.rs).
    // This is a synthetic non-injectivity proof, not a claim of one Odoo
    // instance's effective MRO or delegation registry.
    fn coarse_lift(kind: &str) -> &'static str {
        match kind {
            "_inherit" | "_inherits" => "inherits_from",
            _ => panic!("not a tested Odoo source form"),
        }
    }
    assert_eq!(coarse_lift("_inherit"), coarse_lift("_inherits"));
    // Yet these have distinct method-inheritance semantics.
    fn inherits_python_methods(kind: &str) -> bool {
        kind == "_inherit"
    }
    assert!(inherits_python_methods("_inherit"));
    assert!(!inherits_python_methods("_inherits"));
    // The projected triple alone is insufficient to authorize method
    // inheritance. Neither C3 nor relation composition repairs erased kind.
    let projected = r#"{"s":"odoo:child","p":"inherits_from","o":"odoo:parent","f":1.0,"c":1.0}"#;
    let ts = parse_ndjson(projected).unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].p, "inherits_from");
    // No source-kind witness exists here: classification must remain UNKNOWN.
    let witnessed_kind: Option<&str> = None;
    assert_eq!(witnessed_kind, None);
}
