//! Lab probe: a method registry as an ON × WHEN × DO bit tensor, built from
//! the Odoo slice-1 corpus. Design and source mapping:
//! `docs/DT64-ON-WHEN-DO-REGISTRY.md`.
//!
//! - ON (events): `depends_on` objects of the fields a compute method emits;
//!   `unattested` for methods whose trigger the corpus does not carry
//!   (onchange, constraints, other).
//! - WHEN (guards): `always`, or the `validation_kind` of a `_check_*` method
//!   (`check:unclassified` without one). Provisional: the corpus has no state
//!   preconditions.
//! - DO (capabilities): the field a method emits (`emitted_by`), or
//!   `raise:<exception>` (`raises`).
//!
//! Registrations are an append-only log of versioned rows. The tensor holds
//! presence bits only; method identities live in a separate column keyed by
//! cell. Each test checks the tensor against an independent computation.

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{parse_ndjson, MethodKind, Mro, RecomputeDag};

mod common;
use common::cube::{
    bits, facts, guard_words, naive, registry, strip, Cell, Cube, Live, Op, ALWAYS, CLASS,
    MANIFEST, UNATTESTED,
};
use common::dr64::{compose_blocked, store, Blocked, Dom, Operand2, W};

#[test]
fn dispatch_fold_equals_naive_scan_and_the_guard_axis_bites() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let cube = Cube::build(&reg, &live, CLASS);
    let n_when = reg.when.len();
    assert!(n_when <= 16, "every guard subset is enumerable");
    let mut nonempty = 0;
    let mut guard_changed = 0;
    for on in 0..u16::try_from(reg.on.len()).unwrap() {
        let mut results = BTreeSet::new();
        for subset in 0u32..(1 << n_when) {
            let guards: BTreeSet<u16> = (0..n_when)
                .filter(|g| subset >> g & 1 == 1)
                .map(|g| u16::try_from(g).unwrap())
                .collect();
            let fold = bits(&cube.dispatch(on, &guard_words(&reg, &guards)));
            assert_eq!(
                fold,
                naive(&live, CLASS, on, &guards),
                "event {}",
                reg.on.names[usize::from(on)]
            );
            if !fold.is_empty() {
                nonempty += 1;
            }
            results.insert(fold);
        }
        if results.len() > 2 {
            guard_changed += 1;
        }
    }
    assert!(nonempty > 0, "some dispatch is non-empty");
    assert!(
        guard_changed > 0,
        "some event's result depends on more than always/never"
    );
    eprintln!(
        "cube: ON {} x WHEN {} x DO {} positions; {} live registrations; {guard_changed} events whose result the guards change",
        reg.on.len(),
        reg.when.len(),
        reg.do_.len(),
        live.values().map(BTreeSet::len).sum::<usize>()
    );
}

#[test]
fn coexisting_writers_all_appear() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let field = "account_move.currency_id";
    let d = reg.do_.get(field).expect("currency_id is a DO position");
    let from_cube: BTreeSet<&str> = live
        .iter()
        .filter(|((c, cell), _)| c == CLASS && cell.do_ == d)
        .flat_map(|(_, ms)| ms.iter().map(String::as_str))
        .collect();
    let from_triples: BTreeSet<&str> = f
        .triples
        .iter()
        .filter(|t| t.p == "emitted_by" && strip(&t.s) == field)
        .map(|t| strip(&t.o))
        .collect();
    assert!(
        from_triples.len() >= 2,
        "the fixture field has several writers"
    );
    assert_eq!(from_cube, from_triples);
}

#[test]
fn replay_separates_replacement_from_coexistence() {
    let f = facts();
    let mut reg = registry(&f);
    let before = reg.live(1);
    let old = "account_move._onchange_invoice_vendor_bill";
    let new = "account_move._onchange_invoice_vendor_bill_v2";
    let keep = "account_move._compute_currency_id";
    let cells: Vec<Cell> = before
        .iter()
        .filter(|(_, ms)| ms.contains(old))
        .map(|((_, cell), _)| *cell)
        .collect();
    assert_ne!(cells.len(), 0, "the overridden method holds cells");
    // Version 2 overrides `old` with `new` at the same cells (synthetic).
    for &cell in &cells {
        reg.push(2, Op::Retract, CLASS, old, cell);
        reg.push(2, Op::Register, CLASS, new, cell);
    }
    assert_eq!(
        reg.live(1),
        before,
        "a later version does not change an earlier replay"
    );
    let after = reg.live(2);
    for &cell in &cells {
        let ms = &after[&(CLASS.to_owned(), cell)];
        assert!(ms.contains(new) && !ms.contains(old));
    }
    let currency = reg.do_.get("account_move.currency_id").unwrap();
    let writers: BTreeSet<&str> = after
        .iter()
        .filter(|((_, cell), _)| cell.do_ == currency)
        .flat_map(|(_, ms)| ms.iter().map(String::as_str))
        .collect();
    assert!(
        writers.contains(keep),
        "the coexisting writer survives the override"
    );
    assert!(writers.contains(new) && !writers.contains(old));
}

#[test]
fn equal_specificity_collisions_are_reported() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    let reported: Vec<(Cell, Vec<&str>)> = live
        .iter()
        .filter(|((c, _), ms)| c == CLASS && ms.len() > 1)
        .map(|((_, cell), ms)| (*cell, ms.iter().map(String::as_str).collect()))
        .collect();
    // Independent count: walk fields first, not the registry.
    let mut expected = 0;
    let mut by_field: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (m, fields) in &f.emits {
        for field in fields {
            by_field.entry(field).or_default().push(m);
        }
    }
    for (field, ms) in &by_field {
        let mut cells: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for m in ms {
            let compute = MethodKind::classify(m) == MethodKind::Compute;
            match f.deps.get(*field) {
                Some(d) if compute => {
                    for on in d {
                        cells.entry(on).or_default().insert(m);
                    }
                }
                _ => {
                    cells.entry(UNATTESTED).or_default().insert(m);
                }
            }
        }
        expected += cells.values().filter(|s| s.len() > 1).count();
    }
    let mut raise_cells: BTreeMap<(String, String), BTreeSet<&str>> = BTreeMap::new();
    for (m, excs) in &f.raises {
        let when = f.kind.get(m).map_or("unclassified", String::as_str);
        for e in excs {
            raise_cells
                .entry((when.to_owned(), e.clone()))
                .or_default()
                .insert(m);
        }
    }
    expected += raise_cells.values().filter(|s| s.len() > 1).count();
    assert_eq!(reported.len(), expected);
    assert!(!reported.is_empty(), "the corpus has collisions to report");
    for (cell, ms) in &reported {
        eprintln!(
            "collision on={} when={} do={}: {ms:?}",
            reg.on.names[usize::from(cell.on)],
            reg.when.names[usize::from(cell.when)],
            reg.do_.names[usize::from(cell.do_)]
        );
    }
}

/// First class along `class`'s C3 linearization with a registration at `cell`.
fn resolve<'a>(
    lin: &'a [String],
    live: &'a Live,
    cell: Cell,
) -> Option<(&'a str, &'a BTreeSet<String>)> {
    lin.iter()
        .find_map(|c| live.get(&(c.clone(), cell)).map(|ms| (c.as_str(), ms)))
}

#[test]
fn subtype_fallback() {
    let manifest = parse_ndjson(MANIFEST).expect("manifest parses");
    let mro = Mro::from_triples(&manifest);
    let lin = mro.linearize(CLASS).expect("account_move linearizes");
    assert_eq!(lin[0], CLASS);
    let mixin = "mail_activity_mixin";
    assert!(
        lin.iter().any(|c| c == mixin),
        "account_move _inherit's the mixin"
    );
    let own_mixin = mro.own_method_names(mixin).count();
    assert!(own_mixin > 0, "the manifest lists the mixin's own methods");

    let f = facts();
    let mut reg = registry(&f);
    // Synthetic: the manifest lists the mixin's methods but not their fields,
    // so the mixin's registrations are placed by hand.
    let mixin_only = reg.cell(UNATTESTED, ALWAYS, "mail_activity_mixin.activity_state");
    reg.push(
        1,
        Op::Register,
        mixin,
        "mail_activity_mixin._compute_activity_state",
        mixin_only,
    );
    let shadowed = Cell {
        on: reg.on.get(UNATTESTED).unwrap(),
        when: reg.when.get(ALWAYS).unwrap(),
        do_: reg.do_.get("account_move.currency_id").unwrap(),
    };
    reg.push(
        1,
        Op::Register,
        mixin,
        "mail_activity_mixin._shadowed",
        shadowed,
    );
    let live = reg.live(1);

    let (who, _) = resolve(&lin, &live, mixin_only).expect("falls back to the mixin");
    assert_eq!(who, mixin);
    let (who, ms) = resolve(&lin, &live, shadowed).expect("account_move has its own");
    assert_eq!(who, CLASS, "the subclass's registration shadows the base's");
    assert!(!ms.contains("mail_activity_mixin._shadowed"));
    let mixin_lin = mro.linearize(mixin).expect("mixin linearizes");
    let (who, _) = resolve(&mixin_lin, &live, shadowed).expect("the mixin sees its own");
    assert_eq!(who, mixin);
}

#[test]
fn projection_composes_through_relcomp() {
    let f = facts();
    let reg = registry(&f);
    let live = reg.live(1);
    // Methods that both write fields and are triggered by declared events.
    let mut methods: BTreeSet<&str> = BTreeSet::new();
    let mut fields: BTreeSet<&str> = BTreeSet::new();
    for ((_, cell), ms) in &live {
        let on = reg.on.names[usize::from(cell.on)].as_str();
        let d = reg.do_.names[usize::from(cell.do_)].as_str();
        if d.starts_with("raise:") {
            continue;
        }
        methods.extend(ms.iter().map(String::as_str));
        fields.insert(d);
        if on != UNATTESTED {
            fields.insert(on);
        }
    }
    let methods: Vec<&str> = methods.into_iter().collect();
    let fields: Vec<&str> = fields.into_iter().collect();
    let mi = |m: &str| methods.binary_search(&m).unwrap();
    let fi = |x: &str| fields.binary_search(&x).unwrap();
    let mut writes = Blocked::new(methods.len(), fields.len());
    let mut triggers = Blocked::new(fields.len(), methods.len());
    for ((_, cell), ms) in &live {
        let on = reg.on.names[usize::from(cell.on)].as_str();
        let d = reg.do_.names[usize::from(cell.do_)].as_str();
        if d.starts_with("raise:") {
            continue;
        }
        for m in ms {
            writes.set(mi(m), fi(d));
            if on != UNATTESTED {
                triggers.set(fi(on), mi(m));
            }
        }
    }
    let st = store();
    let a = Operand2 {
        rel: &writes,
        row: Dom::Method,
        col: Dom::Field,
        ordinal: 0,
    };
    let b = Operand2 {
        rel: &triggers,
        row: Dom::Field,
        col: Dom::Method,
        ordinal: 1,
    };
    let (comp, _) = compose_blocked(&st, &a, &b);
    let comp = comp.without_diagonal();
    assert!(methods.len() > W, "the projection needs block composition");

    // Naive enumeration of the same relation.
    let mut naive_edges = 0;
    for (x, &m1) in methods.iter().enumerate() {
        for (y, &m2) in methods.iter().enumerate() {
            let hit = x != y && (0..fields.len()).any(|k| writes.get(x, k) && triggers.get(k, y));
            assert_eq!(comp.get(x, y), hit, "{m1} -> {m2}");
            naive_edges += usize::from(hit);
        }
    }
    assert!(naive_edges > 0);

    // Overlap with #44's reads-based relation (a different question).
    let dag = RecomputeDag::from_triples(&f.triples);
    let both = (0..methods.len())
        .flat_map(|x| (0..methods.len()).map(move |y| (x, y)))
        .filter(|&(x, y)| comp.get(x, y) && dag.has_edge(methods[x], methods[y]))
        .count();
    eprintln!(
        "projection: {} methods, {} fields, {naive_edges} declared-dependency edges, {} RecomputeDag edges, {both} in both",
        methods.len(),
        fields.len(),
        dag.edge_count()
    );
}
