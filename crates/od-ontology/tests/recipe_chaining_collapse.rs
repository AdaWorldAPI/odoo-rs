//! **PROBE — constructor-chaining collapse** (the second axis of `D-RECIPE-BITMASK`).
//!
//! The slice_2 probe (`recipe_redundancy_probe.rs`) measured ONE collapse axis:
//! within a class's own behavioural methods, shape + dependency-set dedup →
//! 45.7% collapse / **54.3% leftover**. It explicitly could NOT measure the
//! *inheritance* axis, because slice_2's base mixins (`mail_thread`, …) were
//! out-of-slice.
//!
//! Constructor chaining over `LazyLock` ClassViews is that second axis: a
//! derived class's ClassView is built by chaining its base ClassViews (each a
//! cached constant) + its own delta, so **inherited recipe parts are stored once
//! at the base and shared by every subclass** — they are not per-class leftover.
//! This probe measures that axis on the FULL Odoo corpus's inheritance manifest
//! (where the bases ARE present), so the chaining benefit the slice could not
//! see becomes a real number.
//!
//! # Metric (storage of the materialised method recipe)
//!
//!   * **naive flatten** (no chaining): each class materialises its full recipe =
//!     own methods + a COPY of every transitive ancestor's methods.
//!     `naive = Σ_M (|own(M)| + Σ_{A∈anc(M)} |own(A)|)`.
//!   * **chained**: every method is stored once at its defining class; subclasses
//!     reach inherited methods through the shared base constant.
//!     `chained = Σ_M |own(M)|` = total `has_function`.
//!   * **collapse% = 1 − chained/naive** = the inherited mass that naive flatten
//!     repeats and chaining shares. **leftover% = chained/naive** = the genuine
//!     own fraction.
//!
//! This is ORTHOGONAL to the slice_2 within-class 54.3%: the two axes stack
//! (a class's genuine leftover is its own-after-shape-dedup methods, and even
//! those are stored once and shared wherever inherited). Default build, offline.
//!
//! # Honest bound
//!
//! The corpus's mixin harvest is shallow (popular bases carry few captured
//! methods — e.g. `mail_thread` has a handful, not its real ~100), so the
//! measured collapse is a **lower bound** on the real-world chaining benefit:
//! richer base extraction can only raise it. Fixture provenance:
//! `scratchpad/extract_manifest.rs` projected `lance-graph`'s
//! `odoo_ontology.spo.ndjson` to its `has_function` + `inherits_from` lines.

// Pedantic lints relaxed for this reporting probe: it prints a detailed
// human-facing report (long fn), computes display percentages from small counts
// (benign usize→f64), and its module docs are narrative (bare identifiers).
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, clippy::doc_markdown)]

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{parse_ndjson, MethodKind, Triple};

fn strip_ns(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, r)| r)
}

fn manifest() -> Vec<Triple> {
    let ndjson = include_str!("../../../data/odoo_inheritance_manifest.ndjson");
    parse_ndjson(ndjson).expect("inheritance manifest parses")
}

/// Transitive ancestors of `m` over the `inherits_from` DAG, cycle-safe.
fn ancestors(m: &str, parents: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = parents.get(m).into_iter().flatten().cloned().collect();
    while let Some(p) = stack.pop() {
        if p == m {
            continue;
        }
        if seen.insert(p.clone()) {
            if let Some(ps) = parents.get(&p) {
                for pp in ps {
                    if !seen.contains(pp) {
                        stack.push(pp.clone());
                    }
                }
            }
        }
    }
    seen.remove(m);
    seen
}

fn is_behavioral(method_iri: &str) -> bool {
    matches!(
        MethodKind::classify(method_iri),
        MethodKind::Compute | MethodKind::Check
    )
}

#[test]
fn constructor_chaining_collapse() {
    let triples = manifest();

    // own methods per class, and the inherits_from DAG.
    let mut own: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut own_beh: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut parents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all: BTreeSet<String> = BTreeSet::new();
    let (mut n_hf, mut n_inh) = (0usize, 0usize);

    for t in &triples {
        match t.p.as_str() {
            "has_function" => {
                n_hf += 1;
                let owner = strip_ns(&t.s).to_string();
                let method = strip_ns(&t.o).to_string();
                if is_behavioral(&method) {
                    own_beh.entry(owner.clone()).or_default().insert(method.clone());
                }
                own.entry(owner.clone()).or_default().insert(method);
                all.insert(owner);
            }
            "inherits_from" => {
                n_inh += 1;
                let child = strip_ns(&t.s).to_string();
                let base = strip_ns(&t.o).to_string();
                parents.entry(child.clone()).or_default().insert(base.clone());
                all.insert(child);
                all.insert(base);
            }
            _ => {}
        }
    }

    let count = |m: &BTreeMap<String, BTreeSet<String>>, k: &str| m.get(k).map_or(0, BTreeSet::len);

    // Per-class naive (own + inherited) vs chained (own once).
    let mut chained = 0usize;
    let mut naive = 0usize;
    let mut chained_beh = 0usize;
    let mut naive_beh = 0usize;
    let mut with_ancestors = 0usize;
    let mut max_depth = 0usize;
    // base → number of subclasses inheriting it (transitively) × its own size.
    let mut base_inherited_copies: BTreeMap<String, usize> = BTreeMap::new();

    for m in &all {
        let o = count(&own, m);
        let ob = count(&own_beh, m);
        chained += o;
        chained_beh += ob;
        let anc = ancestors(m, &parents);
        if !anc.is_empty() {
            with_ancestors += 1;
            max_depth = max_depth.max(anc.len());
        }
        let inh: usize = anc.iter().map(|a| count(&own, a)).sum();
        let inh_b: usize = anc.iter().map(|a| count(&own_beh, a)).sum();
        naive += o + inh;
        naive_beh += ob + inh_b;
        for a in &anc {
            *base_inherited_copies.entry(a.clone()).or_default() += count(&own, a);
        }
    }

    let pct = |num: usize, den: usize| -> f64 {
        if den == 0 {
            0.0
        } else {
            100.0 * num as f64 / den as f64
        }
    };
    let collapse = 100.0 - pct(chained, naive);
    let leftover = pct(chained, naive);
    let collapse_beh = 100.0 - pct(chained_beh, naive_beh);
    let leftover_beh = pct(chained_beh, naive_beh);

    // Top bases by inherited-copy contribution (where the collapse comes from).
    let mut top: Vec<(&String, &usize)> = base_inherited_copies
        .iter()
        .filter(|(_, &c)| c > 0)
        .collect();
    top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    eprintln!("──── constructor-chaining collapse (full Odoo inheritance manifest) ────");
    eprintln!("classes (with methods or inheritance) : {}", all.len());
    eprintln!("  with ≥1 ancestor                    : {with_ancestors}  (max chain depth {max_depth})");
    eprintln!("inherits_from edges                   : {n_inh}");
    eprintln!("has_function (methods)                : {n_hf}");
    eprintln!("── full method recipe ──");
    eprintln!("chained (stored once)                 : {chained}");
    eprintln!("naive flatten (own + inherited copies): {naive}");
    eprintln!("CHAINING COLLAPSE                     : {collapse:.1}%   (leftover {leftover:.1}%)");
    eprintln!("── behavioural subset (Compute + Check) ──");
    eprintln!("chained                               : {chained_beh}");
    eprintln!("naive flatten                         : {naive_beh}");
    eprintln!("CHAINING COLLAPSE (behavioural)       : {collapse_beh:.1}%   (leftover {leftover_beh:.1}%)");
    eprintln!("── top bases by inherited copies (where the collapse lives) ──");
    for (b, c) in top.iter().take(6) {
        eprintln!("  {b:<32} {c} inherited copies");
    }
    eprintln!("────────────────────────────────────────────────────────────────────");
    eprintln!("vs the slice_2 within-class baseline (54.3% own-only behavioural leftover,");
    eprintln!("NO inheritance): this is the ORTHOGONAL inheritance axis the slice could not");
    eprintln!("see — it stacks with the within-class dedup. LOWER bound: the corpus mixin");
    eprintln!("harvest is shallow (real mail.thread ~100 methods, captured handful).");

    // ── Structural invariants (true regardless of the measured ratio) ──
    assert_eq!(n_hf, 3328, "manifest has_function count drifted from the fixture");
    assert_eq!(n_inh, 166, "manifest inherits_from count drifted from the fixture");
    assert!(all.len() > 100, "full corpus spans many classes, saw {}", all.len());
    assert!(with_ancestors > 0, "expected classes with inheritance, saw {with_ancestors}");
    assert!(naive >= chained, "naive flatten must be ≥ chained storage");
    assert_eq!(chained, n_hf, "chained storage = every method once = total has_function");
    assert!(
        collapse > 0.0,
        "inheritance present → chaining must collapse some inherited mass (saw {collapse:.1}%)"
    );
    assert!((0.0..=100.0).contains(&leftover));
    assert!((0.0..=100.0).contains(&leftover_beh));
}
