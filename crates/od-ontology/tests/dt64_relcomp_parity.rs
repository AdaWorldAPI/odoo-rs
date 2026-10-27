//! Odoo method ordering through DR64 relation composition, checked against
//! [`RecomputeDag`].
//!
//! The corpus carries two relations: `field emitted_by method` and
//! `method reads_field field`. "Method `b` must run after method `a`" is
//! their composition `emits ∘ read_by` with the diagonal removed. Here that
//! composition is computed by `dr-math-binding-probe`'s relcomp kernel over
//! 64 × 64 Boolean registers, and compared with the hand-written index
//! `RecomputeDag::from_triples`, which shares no code with it.
//!
//! The two are not identical, and the difference is part of the result:
//! `RecomputeDag` keeps ONE emitter per field (the last `emitted_by` line
//! wins), while composition keeps every emitter. So the checks are:
//!
//! 1. every `RecomputeDag` edge is in the composition;
//! 2. every composition edge `RecomputeDag` lacks goes through a field with
//!    two or more emitters;
//! 3. there is at least one such edge, so check 2 is not vacuous.
//!
//! Slice 1 (57 methods, 34 fields) fits one register; it runs through
//! `Chain::eval` for every association order. Slice 2 (125 methods) does
//! not, so it is split into 64-wide blocks and every block product goes
//! through the same `Chain`.

use std::collections::{BTreeMap, BTreeSet};

use dr_math_binding_probe::relcomp::{all_orders, Chain};
use od_ontology::{parse_ndjson, RecomputeDag, Triple};
use ruff_spo_triplet::{Predicate, PredicateRole};

mod common;
use common::dr64::{bind, compose_blocked, store, Blocked, Dom, Operand2, W};

const SLICE_1: &str = include_str!("../../../data/account_move.spo.ndjson");
const SLICE_2: &str = include_str!("../../../data/slice_2.spo.ndjson");

fn strip(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, rest)| rest)
}

/// The emit/read relations restricted to fields that are both emitted and
/// read, with the methods and fields numbered.
struct Corpus {
    methods: Vec<String>,
    fields: Vec<String>,
    /// field index -> emitting method indices
    emitters: Vec<BTreeSet<usize>>,
    /// field index -> reading method indices
    readers: Vec<BTreeSet<usize>>,
}

impl Corpus {
    fn from_triples(triples: &[Triple]) -> Self {
        let mut emit: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut read: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for t in triples {
            match Predicate::from_str(&t.p) {
                Some(Predicate::EmittedBy) => {
                    emit.entry(strip(&t.s)).or_default().insert(strip(&t.o));
                }
                Some(Predicate::ReadsField) => {
                    read.entry(strip(&t.o)).or_default().insert(strip(&t.s));
                }
                _ => {}
            }
        }
        let fields: Vec<&str> = emit
            .keys()
            .copied()
            .filter(|f| read.contains_key(f))
            .collect();
        let methods: BTreeSet<&str> = fields
            .iter()
            .flat_map(|f| emit[f].iter().chain(read[f].iter()).copied())
            .collect();
        let methods: Vec<String> = methods.into_iter().map(str::to_owned).collect();
        let index = |m: &str| methods.binary_search_by(|x| x.as_str().cmp(m)).unwrap();
        Self {
            emitters: fields
                .iter()
                .map(|f| emit[f].iter().map(|m| index(m)).collect())
                .collect(),
            readers: fields
                .iter()
                .map(|f| read[f].iter().map(|m| index(m)).collect())
                .collect(),
            fields: fields.into_iter().map(str::to_owned).collect(),
            methods,
        }
    }
}

/// `emits` (method × field) and `read_by` (field × method).
fn relations(c: &Corpus) -> (Blocked, Blocked) {
    let (m, f) = (c.methods.len(), c.fields.len());
    let mut emits = Blocked::new(m, f);
    let mut read_by = Blocked::new(f, m);
    for (fi, ms) in c.emitters.iter().enumerate() {
        for &mi in ms {
            emits.set(mi, fi);
        }
    }
    for (fi, ms) in c.readers.iter().enumerate() {
        for &mi in ms {
            read_by.set(fi, mi);
        }
    }
    (emits, read_by)
}

/// Oracle edges between the corpus's methods, as a matrix.
fn oracle_matrix(c: &Corpus, dag: &RecomputeDag) -> Blocked {
    let n = c.methods.len();
    let mut out = Blocked::new(n, n);
    for (a, ma) in c.methods.iter().enumerate() {
        for (b, mb) in c.methods.iter().enumerate() {
            if dag.has_edge(ma, mb) {
                out.set(a, b);
            }
        }
    }
    out
}

/// The checks of the module docs, returning (oracle edges, extra edges).
fn check_one_hop(c: &Corpus, dag: &RecomputeDag, comp: &Blocked) -> (usize, usize) {
    let oracle = oracle_matrix(c, dag);
    assert_eq!(
        oracle.count(),
        dag.edge_count(),
        "every RecomputeDag edge has both ends in the corpus's method index"
    );
    let mut extra = 0;
    for a in 0..c.methods.len() {
        for b in 0..c.methods.len() {
            let (o, r) = (oracle.get(a, b), comp.get(a, b));
            assert!(
                !o || r,
                "oracle edge {} -> {} missing",
                c.methods[a],
                c.methods[b]
            );
            if r && !o {
                extra += 1;
                let explained = (0..c.fields.len()).any(|f| {
                    c.emitters[f].len() > 1
                        && c.emitters[f].contains(&a)
                        && c.readers[f].contains(&b)
                });
                assert!(
                    explained,
                    "extra edge {} -> {} is not through a multi-emitter field",
                    c.methods[a], c.methods[b]
                );
            }
        }
    }
    (oracle.count(), extra)
}

fn load(src: &str) -> (Vec<Triple>, Corpus) {
    let triples = parse_ndjson(src).expect("corpus parses");
    let corpus = Corpus::from_triples(&triples);
    (triples, corpus)
}

#[test]
fn the_composed_operands_carry_the_effect_and_read_roles() {
    assert!(Predicate::EmittedBy.role().contains(PredicateRole::EFFECTS));
    assert!(Predicate::ReadsField.role().contains(PredicateRole::READS));
    // The ordering relation is effect-then-read; neither is a description.
    assert!(!Predicate::EmittedBy
        .role()
        .intersects(PredicateRole::DESCRIBES));
    assert!(!Predicate::ReadsField
        .role()
        .intersects(PredicateRole::DESCRIBES));
}

#[test]
fn slice_1_one_hop_matches_recompute_dag_in_one_register() {
    let (triples, c) = load(SLICE_1);
    assert!(
        c.methods.len() <= W && c.fields.len() <= W,
        "slice 1 fits one register"
    );
    let dag = RecomputeDag::from_triples(&triples);
    let (emits, read_by) = relations(&c);
    let st = store();
    let e = Operand2 {
        rel: &emits,
        row: Dom::Method,
        col: Dom::Field,
        ordinal: 0,
    };
    let r = Operand2 {
        rel: &read_by,
        row: Dom::Field,
        col: Dom::Method,
        ordinal: 1,
    };
    let (comp, _) = compose_blocked(&st, &e, &r);
    let (oracle, extra) = check_one_hop(&c, &dag, &comp.without_diagonal());
    assert!(oracle > 0, "the oracle has edges");
    assert!(
        extra > 0,
        "multi-emitter fields produce edges the oracle drops"
    );
    eprintln!(
        "slice 1: {} methods, {} fields, {oracle} oracle edges, {extra} extra",
        c.methods.len(),
        c.fields.len()
    );
}

#[test]
fn slice_2_one_hop_matches_recompute_dag_across_blocks() {
    let (triples, c) = load(SLICE_2);
    assert!(c.methods.len() > W, "slice 2 needs more than one register");
    let dag = RecomputeDag::from_triples(&triples);
    let (emits, read_by) = relations(&c);
    let st = store();
    let e = Operand2 {
        rel: &emits,
        row: Dom::Method,
        col: Dom::Field,
        ordinal: 0,
    };
    let r = Operand2 {
        rel: &read_by,
        row: Dom::Field,
        col: Dom::Method,
        ordinal: 1,
    };
    let (comp, _) = compose_blocked(&st, &e, &r);
    // Edges that cross a block boundary exercise the block split.
    let crossing = (0..c.methods.len())
        .flat_map(|a| (0..c.methods.len()).map(move |b| (a, b)))
        .filter(|&(a, b)| a / W != b / W && comp.get(a, b))
        .count();
    assert!(crossing > 0, "some edge crosses a block boundary");
    let (oracle, extra) = check_one_hop(&c, &dag, &comp.without_diagonal());
    assert!(oracle > 0 && extra > 0);
    eprintln!(
        "slice 2: {} methods, {} fields, {oracle} oracle edges, {extra} extra, {crossing} cross-block",
        c.methods.len(),
        c.fields.len()
    );
}

/// Every association order of `emits ∘ read_by ∘ emits ∘ read_by` gives the
/// same relation, and it contains every two-step path of the oracle.
#[test]
fn slice_1_two_hop_is_order_independent_and_covers_oracle_paths() {
    let (triples, c) = load(SLICE_1);
    let dag = RecomputeDag::from_triples(&triples);
    let (emits, read_by) = relations(&c);
    let st = store();
    let e = Operand2 {
        rel: &emits,
        row: Dom::Method,
        col: Dom::Field,
        ordinal: 0,
    };
    let r = Operand2 {
        rel: &read_by,
        row: Dom::Field,
        col: Dom::Method,
        ordinal: 1,
    };
    let ops = [&e, &r, &e, &r];
    let chain = Chain::new(
        &st,
        ops.iter()
            .map(|op| bind(&st, op, op.rel.block(0, 0)))
            .collect(),
    )
    .expect("method -> field -> method -> field -> method lines up");
    let orders = all_orders(4);
    assert_eq!(orders.len(), 5);
    let mut results = Vec::new();
    for o in &orders {
        let (rows, work) = chain.eval(o).expect("order covers the chain");
        results.push((rows, work.row_ors));
    }
    let first = results[0].0;
    assert!(results.iter().all(|(r, _)| *r == first), "orders disagree");

    let oracle = oracle_matrix(&c, &dag);
    let n = c.methods.len();
    let mut paths = 0;
    for (a, row) in first.iter().enumerate().take(n) {
        for b in 0..n {
            if (0..n).any(|x| oracle.get(a, x) && oracle.get(x, b)) {
                paths += 1;
                assert!(
                    (row >> b) & 1 == 1,
                    "oracle path {} -> {} missing",
                    c.methods[a],
                    c.methods[b]
                );
            }
        }
    }
    assert!(paths > 0, "the oracle has two-step paths");
    let works: Vec<u64> = results.iter().map(|(_, w)| *w).collect();
    eprintln!("slice 1 two-hop: {paths} oracle paths, work per order {works:?}");
}
