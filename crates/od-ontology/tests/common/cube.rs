//! The ON x WHEN x DO method registry shared by the od-ontology lab probes.
//! Design: `docs/DT64-ON-WHEN-DO-REGISTRY.md`.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{parse_ndjson, MethodKind, Triple};

use super::dr64::W;

pub const SLICE_1: &str = include_str!("../../../../data/account_move.spo.ndjson");
pub const MANIFEST: &str = include_str!("../../../../data/odoo_inheritance_manifest.ndjson");
pub const CLASS: &str = "account_move";
pub const UNATTESTED: &str = "unattested";
pub const ALWAYS: &str = "always";

pub fn strip(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, rest)| rest)
}

/// Names interned to axis positions, in first-seen order.
#[derive(Default)]
pub struct Vocab {
    pub names: Vec<String>,
    pub index: BTreeMap<String, u16>,
}

impl Vocab {
    pub fn intern(&mut self, name: &str) -> u16 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = u16::try_from(self.names.len()).expect("vocabulary fits u16");
        self.names.push(name.to_owned());
        self.index.insert(name.to_owned(), i);
        i
    }
    pub fn get(&self, name: &str) -> Option<u16> {
        self.index.get(name).copied()
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn blocks(&self) -> usize {
        self.len().div_ceil(W)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell {
    pub on: u16,
    pub when: u16,
    pub do_: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Register,
    Retract,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub version: u32,
    pub op: Op,
    pub class: String,
    pub method: String,
    pub cell: Cell,
}

/// `(class, cell) -> methods` live at one version.
pub type Live = BTreeMap<(String, Cell), BTreeSet<String>>;

#[derive(Default)]
pub struct Registry {
    pub on: Vocab,
    pub when: Vocab,
    pub do_: Vocab,
    pub log: Vec<Row>,
}

impl Registry {
    pub fn push(&mut self, version: u32, op: Op, class: &str, method: &str, cell: Cell) {
        if let Some(last) = self.log.last() {
            assert!(version >= last.version, "the log is append-only by version");
        }
        self.log.push(Row {
            version,
            op,
            class: class.to_owned(),
            method: method.to_owned(),
            cell,
        });
    }

    pub fn cell(&mut self, on: &str, when: &str, do_: &str) -> Cell {
        Cell {
            on: self.on.intern(on),
            when: self.when.intern(when),
            do_: self.do_.intern(do_),
        }
    }

    /// Replay the log up to and including `version`.
    pub fn live(&self, version: u32) -> Live {
        let mut out: Live = BTreeMap::new();
        for r in self.log.iter().take_while(|r| r.version <= version) {
            let key = (r.class.clone(), r.cell);
            match r.op {
                Op::Register => {
                    out.entry(key).or_default().insert(r.method.clone());
                }
                Op::Retract => {
                    if let Some(set) = out.get_mut(&key) {
                        set.remove(&r.method);
                        if set.is_empty() {
                            out.remove(&key);
                        }
                    }
                }
            }
        }
        out
    }
}

/// The presence tensor of one class at one version. For each ON position, a
/// WHEN-block × DO-block grid of 64 rows (WHEN) of 64 bits (DO).
pub struct Cube {
    pub when_blocks: usize,
    pub do_blocks: usize,
    pub planes: BTreeMap<u16, Vec<[u64; W]>>,
}

impl Cube {
    pub fn build(reg: &Registry, live: &Live, class: &str) -> Self {
        let (when_blocks, do_blocks) = (reg.when.blocks(), reg.do_.blocks());
        let mut planes: BTreeMap<u16, Vec<[u64; W]>> = BTreeMap::new();
        for (c, cell) in live.keys() {
            if c != class {
                continue;
            }
            let grid = planes
                .entry(cell.on)
                .or_insert_with(|| vec![[0u64; W]; when_blocks * do_blocks]);
            let (w, d) = (usize::from(cell.when), usize::from(cell.do_));
            grid[(w / W) * do_blocks + d / W][w % W] |= 1u64 << (d % W);
        }
        Self {
            when_blocks,
            do_blocks,
            planes,
        }
    }

    /// The fold: OR the DO rows of every guard set in `guards`.
    pub fn dispatch(&self, on: u16, guards: &[u64]) -> Vec<u64> {
        let mut out = vec![0u64; self.do_blocks];
        let Some(grid) = self.planes.get(&on) else {
            return out;
        };
        for (wb, &mask) in guards.iter().enumerate().take(self.when_blocks) {
            let mut m = mask;
            while m != 0 {
                let w = m.trailing_zeros() as usize;
                m &= m - 1;
                for (db, word) in out.iter_mut().enumerate() {
                    *word |= grid[wb * self.do_blocks + db][w];
                }
            }
        }
        out
    }
}

pub fn bits(words: &[u64]) -> BTreeSet<u16> {
    let mut out = BTreeSet::new();
    for (b, &w) in words.iter().enumerate() {
        let mut m = w;
        while m != 0 {
            let i = m.trailing_zeros() as usize;
            m &= m - 1;
            out.insert(u16::try_from(b * W + i).unwrap());
        }
    }
    out
}

/// Slice-1 facts the registry is built from.
pub struct Facts {
    pub triples: Vec<Triple>,
    /// method -> fields it emits
    pub emits: BTreeMap<String, BTreeSet<String>>,
    /// field -> declared dependencies
    pub deps: BTreeMap<String, BTreeSet<String>>,
    /// method -> exceptions it raises
    pub raises: BTreeMap<String, BTreeSet<String>>,
    /// method -> validation kind
    pub kind: BTreeMap<String, String>,
}

pub fn facts() -> Facts {
    let triples = parse_ndjson(SLICE_1).expect("slice 1 parses");
    let mut f = Facts {
        triples: Vec::new(),
        emits: BTreeMap::new(),
        deps: BTreeMap::new(),
        raises: BTreeMap::new(),
        kind: BTreeMap::new(),
    };
    for t in &triples {
        let (s, o) = (strip(&t.s).to_owned(), strip(&t.o).to_owned());
        match t.p.as_str() {
            "emitted_by" => {
                f.emits.entry(o).or_default().insert(s);
            }
            "depends_on" => {
                f.deps.entry(s).or_default().insert(o);
            }
            "raises" => {
                f.raises.entry(s).or_default().insert(o);
            }
            "validation_kind" => {
                f.kind.insert(s, o);
            }
            _ => {}
        }
    }
    f.triples = triples;
    f
}

/// Version 1: every registration the corpus supports.
pub fn registry(f: &Facts) -> Registry {
    let mut reg = Registry::default();
    reg.on.intern(UNATTESTED);
    reg.when.intern(ALWAYS);
    for (m, fields) in &f.emits {
        let compute = MethodKind::classify(m) == MethodKind::Compute;
        for field in fields {
            let ons: Vec<&str> = match f.deps.get(field) {
                Some(d) if compute => d.iter().map(String::as_str).collect(),
                _ => vec![UNATTESTED],
            };
            for on in ons {
                let cell = reg.cell(on, ALWAYS, field);
                reg.push(1, Op::Register, CLASS, m, cell);
            }
        }
    }
    for (m, excs) in &f.raises {
        let when = f
            .kind
            .get(m)
            .map_or_else(|| "check:unclassified".to_owned(), |k| format!("check:{k}"));
        for exc in excs {
            let cell = reg.cell(UNATTESTED, &when, &format!("raise:{exc}"));
            reg.push(1, Op::Register, CLASS, m, cell);
        }
    }
    reg
}

/// Naive scan of live registrations: the DO positions `class` reaches on `on`
/// with the guards in `guards` holding.
pub fn naive(live: &Live, class: &str, on: u16, guards: &BTreeSet<u16>) -> BTreeSet<u16> {
    live.keys()
        .filter(|(c, cell)| c == class && cell.on == on && guards.contains(&cell.when))
        .map(|(_, cell)| cell.do_)
        .collect()
}

pub fn guard_words(reg: &Registry, set: &BTreeSet<u16>) -> Vec<u64> {
    let mut out = vec![0u64; reg.when.blocks()];
    for &g in set {
        out[usize::from(g) / W] |= 1u64 << (usize::from(g) % W);
    }
    out
}
