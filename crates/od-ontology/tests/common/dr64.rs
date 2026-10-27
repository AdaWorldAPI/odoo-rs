//! DR64 helpers shared by the od-ontology lab probes: 64 x 64 Boolean
//! relations split into blocks, bound as `dr-math-binding-probe` operands and
//! composed through `relcomp::Chain`.
//!
//! Each test binary uses a different subset of these items.
#![allow(dead_code)]

use dr_math_binding_probe::{
    bind_dr64,
    relcomp::{Chain, RelExpr, Rows},
    Axis, AxisKind, BoundDr64, Focus, FrameRevisions, FrameStore, Guards, MethodSignature, Operand,
    Scope,
};

pub const W: usize = 64;
pub const FOCUS: Focus = Focus { revision: 1 };
pub const SCOPE: Scope = Scope { id: 1 };
pub const METHOD_AXIS: Axis = Axis {
    class: 1,
    kind: AxisKind::Capability,
};
pub const FIELD_AXIS: Axis = Axis {
    class: 2,
    kind: AxisKind::Property,
};

/// A Boolean relation stored as 64 × 64 blocks, row-block major.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Blocked {
    pub rows: usize,
    pub cols: usize,
    pub blocks: Vec<Rows>,
}

impl Blocked {
    pub fn new(rows: usize, cols: usize) -> Self {
        let n = rows.div_ceil(W) * cols.div_ceil(W);
        Self {
            rows,
            cols,
            blocks: vec![[0u64; W]; n],
        }
    }
    pub fn col_blocks(&self) -> usize {
        self.cols.div_ceil(W)
    }
    pub fn set(&mut self, i: usize, j: usize) {
        let b = (i / W) * self.col_blocks() + j / W;
        self.blocks[b][i % W] |= 1u64 << (j % W);
    }
    pub fn get(&self, i: usize, j: usize) -> bool {
        let b = (i / W) * self.col_blocks() + j / W;
        (self.blocks[b][i % W] >> (j % W)) & 1 == 1
    }
    pub fn block(&self, bi: usize, bj: usize) -> &Rows {
        &self.blocks[bi * self.col_blocks() + bj]
    }
    pub fn count(&self) -> usize {
        self.blocks
            .iter()
            .flat_map(|b| b.iter())
            .map(|w| w.count_ones() as usize)
            .sum()
    }
    pub fn without_diagonal(&self) -> Self {
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
pub enum Dom {
    Method,
    Field,
}

impl Dom {
    pub fn axis(self) -> Axis {
        match self {
            Dom::Method => METHOD_AXIS,
            Dom::Field => FIELD_AXIS,
        }
    }
}

/// One relation of the chain with the domains it maps between.
pub struct Operand2<'a> {
    pub rel: &'a Blocked,
    pub row: Dom,
    pub col: Dom,
    pub ordinal: u16,
}

pub fn bind<'a>(st: &FrameStore, op: &Operand2<'_>, rows: &'a Rows) -> BoundDr64<'a> {
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
pub fn compose_blocked(st: &FrameStore, a: &Operand2<'_>, b: &Operand2<'_>) -> (Blocked, u64) {
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

pub fn store() -> FrameStore {
    FrameStore::new(FrameRevisions {
        source: 1,
        relation_map: 1,
        coordinates: 1,
    })
}
