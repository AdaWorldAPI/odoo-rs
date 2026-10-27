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

use dr_math_binding_probe::{
    bind_dr64,
    relcomp::{all_orders, Chain, RelExpr, Rows},
    Axis, AxisKind, BoundDr64, Focus, FrameRevisions, FrameStore, Guards, MethodSignature, Operand,
    Scope,
};
use od_ontology::{parse_ndjson, RecomputeDag, Triple};
use ruff_spo_triplet::{Predicate, PredicateRole};

const SLICE_1: &str = include_str!("../../../data/account_move.spo.ndjson");
const SLICE_2: &str = include_str!("../../../data/slice_2.spo.ndjson");

const W: usize = 64;
const FOCUS: Focus = Focus { revision: 1 };
const SCOPE: Scope = Scope { id: 1 };
const METHOD_AXIS: Axis = Axis {
    class: 1,
    kind: AxisKind::Capability,
};
const FIELD_AXIS: Axis = Axis {
    class: 2,
    kind: AxisKind::Property,
};

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

/// A Boolean relation stored as 64 × 64 blocks, row-block major.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Blocked {
    rows: usize,
    cols: usize,
    blocks: Vec<Rows>,
}

impl Blocked {
    fn new(rows: usize, cols: usize) -> Self {
        let n = rows.div_ceil(W) * cols.div_ceil(W);
        Self {
            rows,
            cols,
            blocks: vec![[0u64; W]; n],
        }
    }
    fn col_blocks(&self) -> usize {
        self.cols.div_ceil(W)
    }
    fn set(&mut self, i: usize, j: usize) {
        let b = (i / W) * self.col_blocks() + j / W;
        self.blocks[b][i % W] |= 1u64 << (j % W);
    }
    fn get(&self, i: usize, j: usize) -> bool {
        let b = (i / W) * self.col_blocks() + j / W;
        (self.blocks[b][i % W] >> (j % W)) & 1 == 1
    }
    fn block(&self, bi: usize, bj: usize) -> &Rows {
        &self.blocks[bi * self.col_blocks() + bj]
    }
    fn count(&self) -> usize {
        self.blocks
            .iter()
            .flat_map(|b| b.iter())
            .map(|w| w.count_ones() as usize)
            .sum()
    }
    fn without_diagonal(&self) -> Self {
        let mut out = self.clone();
        for i in 0..self.rows.min(self.cols) {
            let b = (i / W) * out.col_blocks() + i / W;
            out.blocks[b][i % W] &= !(1u64 << (i % W));
        }
        out
    }
}

/// Which domain an operand's rows and columns live in.
#[derive(Clone, Copy)]
enum Dom {
    Method,
    Field,
}

impl Dom {
    fn axis(self) -> Axis {
        match self {
            Dom::Method => METHOD_AXIS,
            Dom::Field => FIELD_AXIS,
        }
    }
}

/// One relation of the chain with the domains it maps between.
struct Operand2<'a> {
    rel: &'a Blocked,
    row: Dom,
    col: Dom,
    ordinal: u16,
}

fn bind<'a>(st: &FrameStore, op: &Operand2<'_>, rows: &'a Rows) -> BoundDr64<'a> {
    let sig = MethodSignature::presence(7, op.ordinal, op.row.axis(), op.col.axis());
    let at = Guards {
        frame: st.seal(),
        focus: FOCUS,
        scope: SCOPE,
    };
    bind_dr64(
        st,
        sig,
        sig,
        &at,
        (FOCUS, SCOPE),
        Operand {
            rows,
            validity: None,
        },
    )
    .expect("a two-valued Distinct operand binds")
}

/// `a ∘ b`, every block product computed by `Chain::eval`.
fn compose_blocked(st: &FrameStore, a: &Operand2<'_>, b: &Operand2<'_>) -> (Blocked, u64) {
    assert_eq!(a.rel.cols, b.rel.rows, "inner dimensions");
    let mut out = Blocked::new(a.rel.rows, b.rel.cols);
    let mut work = 0;
    let order = RelExpr::comp(RelExpr::Leaf(0), RelExpr::Leaf(1));
    for bi in 0..a.rel.rows.div_ceil(W) {
        for bk in 0..b.rel.cols.div_ceil(W) {
            let mut acc = [0u64; W];
            for bj in 0..a.rel.cols.div_ceil(W) {
                let chain = Chain::new(
                    st,
                    vec![
                        bind(st, a, a.rel.block(bi, bj)),
                        bind(st, b, b.rel.block(bj, bk)),
                    ],
                )
                .expect("domains line up");
                let (rows, w) = chain.eval(&order).expect("order covers the chain");
                work += w.row_ors;
                for (dst, src) in acc.iter_mut().zip(rows.iter()) {
                    *dst |= src;
                }
            }
            let n = out.col_blocks();
            out.blocks[bi * n + bk] = acc;
        }
    }
    (out, work)
}

fn store() -> FrameStore {
    FrameStore::new(FrameRevisions {
        source: 1,
        relation_map: 1,
        coordinates: 1,
    })
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
