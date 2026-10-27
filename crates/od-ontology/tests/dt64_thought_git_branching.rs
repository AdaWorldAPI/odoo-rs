//! Lab probe: counterfactual branches of the ON × WHEN × DO registry, landed
//! and resolved through thought-git. Design: `docs/DT64-ON-WHEN-DO-REGISTRY.md`
//! §12.
//!
//! A branch is one assignment of the guards whose truth is unknown. Each
//! branch runs the registry's dispatch fold for one event and becomes one
//! thought. Its writes are the dispatch result as 64-bit DO block masks: row
//! `b`, lane 0 holds block `b`. thought-git's cell space is 12 rows × 4 lanes,
//! so a branch cannot write one cell per DO position; a disagreement inside a
//! block is decoded from the offered words (XOR).
//!
//! What is checked:
//! - the oracle's divergences are exactly the DO positions the unknown guards
//!   change, computed independently by a naive scan;
//! - branches collapse (no divergence) when the guards do not matter;
//! - landing order does not change the version;
//! - coexisting writers never diverge;
//! - supersession runs along a parent link (squash), never between siblings;
//! - synthesis: a truth-valued cell lands by NARS revision, while a Boolean
//!   cell with the same disagreement stays a divergence.

use std::collections::{BTreeMap, BTreeSet};

use lance_graph_contract::exploration::NarsTruth;
use thought_git_probe::{
    fixtures::{cell, Cell as TgCell, Outcome, Thought},
    oracle::{next_version, squash, Givens, Version},
    slots::{Eviction, SlotPool},
};

mod common;
use common::cube::{
    facts, guard_words, naive, registry, Cube, Live, Op, Registry, ALWAYS, CLASS, UNATTESTED,
};
use common::dr64::W;

const MASK_LANE: u8 = 0;
const TRUTH_LANE: u8 = 1;
const BOOL_LANE: u8 = 2;
/// Confidence each branch puts in its own guard assumption (synthetic).
const BRANCH_CONFIDENCE: f32 = 0.5;

/// One guard assignment: the WHEN positions assumed true.
type Assignment = BTreeSet<u16>;

/// `always` is known true; every other guard is unknown.
fn unknown_guards(reg: &Registry) -> Vec<u16> {
    let always = reg.when.get(ALWAYS).unwrap();
    (0..u16::try_from(reg.when.len()).unwrap())
        .filter(|&g| g != always)
        .collect()
}

/// Every assignment of the unknown guards, `always` included in each.
fn assignments(reg: &Registry) -> Vec<Assignment> {
    let always = reg.when.get(ALWAYS).unwrap();
    let unknown = unknown_guards(reg);
    (0u32..1 << unknown.len())
        .map(|bitsel| {
            let mut a: Assignment = unknown
                .iter()
                .enumerate()
                .filter(|(i, _)| bitsel >> i & 1 == 1)
                .map(|(_, &g)| g)
                .collect();
            a.insert(always);
            a
        })
        .collect()
}

fn mask_row(block: usize) -> TgCell {
    cell(u32::try_from(block).unwrap(), MASK_LANE)
}

/// The thought for one branch: the dispatch masks of `on` under `a`.
fn branch_thought(id: u32, reg: &Registry, cube: &Cube, on: u16, a: &Assignment) -> Thought {
    let words = cube.dispatch(on, &guard_words(reg, a));
    let writes = words
        .iter()
        .enumerate()
        .map(|(b, &w)| (mask_row(b), w))
        .collect();
    Thought::new(id, writes, Outcome::Commit)
}

/// DO positions on which the landed branches disagree, decoded from the
/// oracle's divergences.
fn divergent_positions(v: &Version) -> BTreeSet<u16> {
    let mut out = BTreeSet::new();
    for d in &v.divergences {
        assert_eq!(d.cell.lane, MASK_LANE);
        let first = d.offers[0].0;
        let diff = d.offers.iter().fold(0u64, |acc, &(w, _)| acc | (w ^ first));
        for i in 0..64 {
            if diff >> i & 1 == 1 {
                out.insert(u16::try_from(d.cell.row as usize * W + i).unwrap());
            }
        }
    }
    out
}

/// DO positions whose dispatch differs between assignments, by naive scan.
fn naive_divergent(live: &Live, on: u16, all: &[Assignment]) -> BTreeSet<u16> {
    let results: Vec<BTreeSet<u16>> = all.iter().map(|a| naive(live, CLASS, on, a)).collect();
    let union: BTreeSet<u16> = results.iter().flatten().copied().collect();
    let inter: BTreeSet<u16> = union
        .iter()
        .copied()
        .filter(|p| results.iter().all(|r| r.contains(p)))
        .collect();
    union.difference(&inter).copied().collect()
}

/// Branch every assignment from main in a slot pool, and land them.
fn fork_and_land(reg: &Registry, live: &Live, on: u16) -> (Vec<Assignment>, Vec<Thought>, Version) {
    let cube = Cube::build(reg, live, CLASS);
    let all = assignments(reg);
    let mut pool = SlotPool::new(Eviction::Fifo);
    let mut thoughts = Vec::new();
    for (i, a) in all.iter().enumerate() {
        let id = u32::try_from(i + 1).unwrap();
        let (_, pressure) = pool.branch(id, None).expect("main is always live");
        assert!(pressure.is_none(), "every branch fits in the pool");
        thoughts.push(branch_thought(id, reg, &cube, on, a));
    }
    assert_eq!(pool.live(), all.len());
    let v = next_version(&Givens::default(), &Version::default(), &thoughts);
    (all, thoughts, v)
}

#[test]
fn divergences_are_exactly_the_positions_the_unknown_guards_change() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let on = reg.on.get(UNATTESTED).unwrap();
    let (all, _, v) = fork_and_land(&reg, &live, on);
    assert!(all.len() > 2 && all.len() <= 63, "{} branches", all.len());
    let from_oracle = divergent_positions(&v);
    assert_eq!(from_oracle, naive_divergent(&live, on, &all));
    assert!(!from_oracle.is_empty(), "the guards change something");
    for p in &from_oracle {
        assert!(
            reg.do_.names[usize::from(*p)].starts_with("raise:"),
            "only guarded raises diverge, not {}",
            reg.do_.names[usize::from(*p)]
        );
    }
    assert_eq!(v.contradictions.len(), 0, "no given is contradicted");
    eprintln!(
        "{} unknown guards -> {} branches; {} divergent DO positions: {:?}",
        unknown_guards(&reg).len(),
        all.len(),
        from_oracle.len(),
        from_oracle
            .iter()
            .map(|p| reg.do_.names[usize::from(*p)].as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn branches_collapse_when_the_guards_do_not_matter() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let on = reg
        .on
        .get("account_move.partner_id")
        .expect("a declared event");
    let (all, _, v) = fork_and_land(&reg, &live, on);
    assert!(
        naive_divergent(&live, on, &all).is_empty(),
        "fixture: guards do not matter here"
    );
    assert!(v.divergences.is_empty(), "agreeing branches collapse");
    let cube = Cube::build(&reg, &live, CLASS);
    let always: Assignment = [reg.when.get(ALWAYS).unwrap()].into();
    let expect = cube.dispatch(on, &guard_words(&reg, &always));
    assert!(
        expect.iter().any(|&w| w != 0),
        "the event dispatches something"
    );
    for (b, w) in expect.iter().enumerate() {
        assert_eq!(v.cells.get(&mask_row(b)), Some(w), "block {b}");
    }
}

#[test]
fn landing_order_does_not_change_the_version() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let on = reg.on.get(UNATTESTED).unwrap();
    let (_, thoughts, v) = fork_and_land(&reg, &live, on);
    let mut reversed = thoughts.clone();
    reversed.reverse();
    let mut rotated = thoughts.clone();
    rotated.rotate_left(thoughts.len() / 3);
    for order in [reversed, rotated] {
        assert_eq!(
            next_version(&Givens::default(), &Version::default(), &order),
            v
        );
    }
}

#[test]
fn coexisting_writers_never_diverge() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let on = reg.on.get(UNATTESTED).unwrap();
    let field = reg.do_.get("account_move.currency_id").unwrap();
    // The onchange writer is registered at `unattested`; the compute writer at
    // its declared events. Both live in version 1, in every branch.
    let writers: BTreeSet<&str> = live
        .iter()
        .filter(|((_, c), _)| c.do_ == field)
        .flat_map(|(_, ms)| ms.iter().map(String::as_str))
        .collect();
    assert!(writers.len() >= 2);
    let (_, thoughts, v) = fork_and_land(&reg, &live, on);
    assert!(!divergent_positions(&v).contains(&field));
    let row = mask_row(usize::from(field) / W);
    let bit = 1u64 << (usize::from(field) % W);
    for t in &thoughts {
        let w = t.writes.iter().find(|(c, _)| *c == row).unwrap().1;
        assert_eq!(w & bit, bit, "every branch writes currency_id");
    }
}

#[test]
fn supersession_runs_along_a_parent_link_not_between_siblings() {
    let f = facts();
    let mut reg = registry(&f);
    let v1 = reg.live(1);
    let old = "account_move._onchange_invoice_vendor_bill";
    let cells: Vec<_> = v1
        .iter()
        .filter(|(_, ms)| ms.contains(old))
        .map(|((_, c), _)| *c)
        .collect();
    // Version 2 retracts the onchange without a replacement (synthetic).
    for &c in &cells {
        reg.push(2, Op::Retract, CLASS, old, c);
    }
    let v2 = reg.live(2);
    let on = reg.on.get(UNATTESTED).unwrap();
    let all_true: Assignment = (0..u16::try_from(reg.when.len()).unwrap()).collect();
    let parent = branch_thought(1, &reg, &Cube::build(&reg, &v1, CLASS), on, &all_true);
    let child = branch_thought(2, &reg, &Cube::build(&reg, &v2, CLASS), on, &all_true);
    assert_ne!(
        parent.writes, child.writes,
        "the override changes the dispatch"
    );

    let mut pool = SlotPool::new(Eviction::Fifo);
    let (p, _) = pool.branch(1, None).unwrap();
    let (_, _) = pool
        .branch(2, Some(p))
        .expect("the child branches from the parent");

    // Landed as siblings, parent and child diverge.
    let as_siblings = next_version(
        &Givens::default(),
        &Version::default(),
        &[parent.clone(), child.clone()],
    );
    assert_ne!(as_siblings.divergences.len(), 0, "siblings diverge");
    // Squashed along the link, child over parent, the child's view lands.
    let squashed = squash(3, &[parent, child.clone()]);
    let landed = next_version(&Givens::default(), &Version::default(), &[squashed]);
    assert_eq!(landed.divergences.len(), 0, "the squash lands cleanly");
    for (c, w) in &child.writes {
        assert_eq!(landed.cells.get(c), Some(w));
    }
}

fn encode(t: NarsTruth) -> u64 {
    (u64::from(t.frequency.to_bits()) << 32) | u64::from(t.confidence.to_bits())
}

fn decode(v: u64) -> NarsTruth {
    NarsTruth::new(
        f32::from_bits(u32::try_from(v >> 32).unwrap()),
        f32::from_bits(u32::try_from(v & 0xffff_ffff).unwrap()),
    )
}

/// A candidate landing rule: like `next_version`, except that a divergence on
/// a truth-valued cell is resolved by NARS revision of the offers, folded in
/// thought-id order. Boolean cells keep their divergence.
fn land_with_revision(landing: &[Thought], truth: &BTreeSet<TgCell>) -> Version {
    let mut v = next_version(&Givens::default(), &Version::default(), landing);
    let mut kept = Vec::new();
    for d in std::mem::take(&mut v.divergences) {
        if truth.contains(&d.cell) {
            let mut offers: Vec<(u32, u64)> = landing
                .iter()
                .filter_map(|t| {
                    t.writes
                        .iter()
                        .find(|(c, _)| *c == d.cell)
                        .map(|&(_, w)| (t.id, w))
                })
                .collect();
            offers.sort_unstable();
            let revised = offers
                .iter()
                .map(|&(_, w)| decode(w))
                .reduce(|a, b| a.revision(&b))
                .expect("a divergence has offers");
            v.cells.insert(d.cell, encode(revised));
        } else {
            kept.push(d);
        }
    }
    v.divergences = kept;
    v
}

#[test]
fn a_truth_cell_revises_where_a_boolean_cell_stays_divergent() {
    let corpus = facts();
    let reg = registry(&corpus);
    let live = reg.live(1);
    let on = reg.on.get(UNATTESTED).unwrap();
    let all = assignments(&reg);
    let raises: Vec<u16> = (0..u16::try_from(reg.do_.len()).unwrap())
        .filter(|&p| reg.do_.names[usize::from(p)].starts_with("raise:"))
        .collect();
    assert!(raises.len() <= 12, "one row per raise kind");

    // Each branch states, per raise kind, whether it fires: as a truth
    // (frequency 1 or 0 at the branch's confidence) and as a Boolean.
    let mut thoughts = Vec::new();
    let mut fired: BTreeMap<u16, u32> = BTreeMap::new();
    for (i, a) in all.iter().enumerate() {
        let hit = naive(&live, CLASS, on, a);
        let mut writes = Vec::new();
        for (row, &p) in raises.iter().enumerate() {
            let yes = hit.contains(&p);
            *fired.entry(p).or_default() += u32::from(yes);
            let truth_v = NarsTruth::new(if yes { 1.0 } else { 0.0 }, BRANCH_CONFIDENCE);
            let row_u = u32::try_from(row).unwrap();
            writes.push((cell(row_u, TRUTH_LANE), encode(truth_v)));
            writes.push((cell(row_u, BOOL_LANE), u64::from(yes)));
        }
        thoughts.push(Thought::new(
            u32::try_from(i + 1).unwrap(),
            writes,
            Outcome::Commit,
        ));
    }
    let truth: BTreeSet<TgCell> = (0..raises.len())
        .map(|row_u| cell(u32::try_from(row_u).unwrap(), TRUTH_LANE))
        .collect();
    let landed = land_with_revision(&thoughts, &truth);

    let branch_n = f32::from(u16::try_from(all.len()).unwrap());
    let mut revised = 0;
    for (row, p) in raises.iter().enumerate() {
        let row_u = u32::try_from(row).unwrap();
        let fired_n = fired[p];
        let disagree = fired_n != 0 && fired_n != u32::try_from(all.len()).unwrap();
        let bool_divergent = landed
            .divergences
            .iter()
            .any(|d| d.cell == cell(row_u, BOOL_LANE));
        assert_eq!(
            bool_divergent,
            disagree,
            "{}",
            reg.do_.names[usize::from(*p)]
        );
        assert!(landed.divergences.iter().all(|d| d.cell.lane != TRUTH_LANE));
        if disagree {
            revised += 1;
            let truth_v = decode(landed.cells[&cell(row_u, TRUTH_LANE)]);
            // Equal confidences: revision weights the branches equally, so the
            // revised frequency is the share of branches in which it fires.
            let share = f32::from(u16::try_from(fired_n).unwrap()) / branch_n;
            assert!(
                (truth_v.frequency - share).abs() < 1e-4,
                "{} vs {share}",
                truth_v.frequency
            );
            assert!(
                truth_v.confidence > BRANCH_CONFIDENCE,
                "revision accumulates evidence"
            );
            eprintln!(
                "{}: fires in {fired_n} of {} branches, revised to f={:.4} c={:.4}",
                reg.do_.names[usize::from(*p)],
                all.len(),
                truth_v.frequency,
                truth_v.confidence
            );
        }
    }
    assert!(revised > 0, "some raise depends on the unknown guards");

    // Order independence of the fold: reversing the landing order revises to
    // the same truth within float error.
    let mut rev = thoughts.clone();
    rev.reverse();
    let v_rev = land_with_revision(&rev, &truth);
    for c in &truth {
        let (a, b) = (decode(landed.cells[c]), decode(v_rev.cells[c]));
        assert!(
            (a.frequency - b.frequency).abs() < 1e-6 && (a.confidence - b.confidence).abs() < 1e-6
        );
    }
}
