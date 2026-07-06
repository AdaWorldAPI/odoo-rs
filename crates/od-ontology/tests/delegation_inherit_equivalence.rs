//! **PROBE — OGAR falsifier F1: "Delegation ≡ Odoo `_inherit`" as a
//! graph-equivalence check.**
//!
//! Spec (OGAR `docs/INTEGRATION-MAP.md` §6, gate F1): the fixture MUST include
//! **(a)** the single chain (`mail.thread`→`account.move`: `message_post`
//! escalates and fires on the parent; `action_post` fires locally) **AND (b) a
//! diamond** D(B,C), B(A), C(A) with the method on A and C — C3-over-
//! `LastOrderedSet` picks C, naive parent-first picks A; replicate Odoo's
//! declaration-order base assembly, not source-order C3. Verdict semantics:
//! D-DELEG-INHERIT `[H]→[G]`, or the probe names the mixins-ORDERING fix.
//!
//! The grounded sharpening (INTEGRATION-MAP §3, "Open falsification"): *Odoo
//! is NOT naive C3 over the source hierarchy* — `_build_model` assembles bases
//! in `LastOrderedSet` declaration/install order, then Python's C3 runs over
//! THAT tuple; a single-chain `mail.thread` test cannot falsify. The named
//! failure mode if the orders diverge: the fix is `Class.mixins` **ordering**
//! carrying the linearization — not the delegation walk itself.
//!
//! # What this probe found (honest findings)
//!
//! 1. **The diamond fixture carries the falsification** (test 2): C3 resolves
//!    the diamond's method to C, naive parent-first DFS to A — a delegation
//!    chain built parent-first-DFS is NOT ≡ Odoo `_inherit`.
//! 2. **The committed corpus cannot witness order-sensitivity head-to-head**:
//!    every multi-base child's declaration order equals alphabetical order,
//!    and the sweep measures **0** resolution divergences between C3 and
//!    naive DFS across all linearizable classes. The 0 is pinned as a
//!    drift-fuse; the diamond fixture is the falsification arm.
//! 3. **3 corpus hierarchies are C3-INCONSISTENT** (`discuss_channel`,
//!    `product_product`, `product_template`): each declares `mail_thread`
//!    BEFORE a base whose own MRO already contains `mail_thread`
//!    (`rating_mixin` resp. `product_template`). CPython refuses the same
//!    shape ("Cannot create a consistent method resolution order"), while
//!    naive DFS silently resolves — a second, failure-mode divergence between
//!    the two walks. This is exactly the mixins-ORDERING territory the F1 row
//!    names: order carries semantics; C3 surfaces a bad order loudly.

// Pedantic lints relaxed for this reporting probe: it prints a detailed
// human-facing report (long fn) and its module docs are narrative (bare
// identifiers).
#![allow(clippy::too_many_lines, clippy::doc_markdown)]

use std::collections::BTreeSet;

use od_ontology::{parse_ndjson, Mro, MroError, Triple};

fn t(s: &str, p: &str, o: &str) -> Triple {
    Triple {
        s: s.into(),
        p: p.into(),
        o: o.into(),
        f: 0.95,
        c: 0.9,
    }
}

fn manifest() -> Vec<Triple> {
    let ndjson = include_str!("../../../data/odoo_inheritance_manifest.ndjson");
    parse_ndjson(ndjson).expect("inheritance manifest parses")
}

/// F1(a) — the single chain. SYNTHETIC fixture: the committed manifest lacks
/// `message_post` entirely (verified: 0 occurrences), so the chain is built by
/// hand to the F1 row's shape.
#[test]
fn f1a_single_chain_escalates_to_parent_and_fires_locally() {
    let triples = vec![
        t("odoo:account_move", "inherits_from", "odoo:mail_thread"),
        t("odoo:mail_thread", "has_function", "odoo:mail_thread.message_post"),
        t("odoo:account_move", "has_function", "odoo:account_move.action_post"),
    ];
    let mro = Mro::from_triples(&triples);

    let lin = mro.linearize("account_move").expect("chain linearizes");
    assert_eq!(lin, ["account_move", "mail_thread"]);

    // `message_post` escalates and fires on the parent.
    assert_eq!(
        mro.resolve_c3("account_move", "message_post"),
        Some("mail_thread")
    );
    // `action_post` fires locally.
    assert_eq!(
        mro.resolve_c3("account_move", "action_post"),
        Some("account_move")
    );

    // Chains without diamonds are safe: naive parent-first DFS AGREES with C3
    // here — which is precisely why F1 says a single-chain test cannot
    // falsify. The divergence needs the diamond (next test).
    assert_eq!(
        mro.resolve_naive_dfs("account_move", "message_post"),
        Some("mail_thread")
    );
    assert_eq!(
        mro.resolve_naive_dfs("account_move", "action_post"),
        Some("account_move")
    );
}

/// F1(b) — the diamond: D declares bases [B, C] in THAT declaration order;
/// B(A), C(A); method `m` defined on A and on C.
///
/// CPython ground truth (run 2026-07-06, python3 in this environment):
///
/// ```text
/// $ python3 - <<'EOF'
/// class A:
///     def m(self): return "A"
/// class B(A): pass
/// class C(A):
///     def m(self): return "C"
/// class D(B, C): pass
/// print([k.__name__ for k in D.__mro__])
/// print(D().m())
/// EOF
/// ['D', 'B', 'C', 'A', 'object']
/// C
/// ```
///
/// (The trailing `object` is CPython's implicit root; the corpus graph has no
/// implicit root, so the expected linearization here is `[D, B, C, A]`.)
#[test]
fn f1b_diamond_c3_diverges_from_naive_dfs() {
    let triples = vec![
        // Declaration order of D's bases is load-bearing: B first, then C.
        t("odoo:d", "inherits_from", "odoo:b"),
        t("odoo:d", "inherits_from", "odoo:c"),
        t("odoo:b", "inherits_from", "odoo:a"),
        t("odoo:c", "inherits_from", "odoo:a"),
        t("odoo:a", "has_function", "odoo:a.m"),
        t("odoo:c", "has_function", "odoo:c.m"),
    ];
    let mro = Mro::from_triples(&triples);

    // Pin the full C3 linearization — matches CPython's D.__mro__ above.
    assert_eq!(
        mro.linearize("d").expect("diamond linearizes"),
        ["d", "b", "c", "a"]
    );

    // C3 resolution: first definer along [d, b, c, a] is C.
    let c3 = mro.resolve_c3("d", "m");
    assert_eq!(c3, Some("c"), "C3 must pick C — matches CPython D().m");

    // Naive parent-first DFS: d → b → a (defines m) — never reaches c.
    let dfs = mro.resolve_naive_dfs("d", "m");
    assert_eq!(dfs, Some("a"), "naive DFS reaches the apex through B first");

    // THE FALSIFICATION ARM: the two resolutions DIVERGE on the diamond. A
    // delegation chain built parent-first-DFS is NOT ≡ Odoo `_inherit`; only
    // C3 over the declaration-order base tuple is.
    assert_ne!(c3, dfs, "diamond must expose C3 vs naive-DFS divergence");
}

/// The corpus sweep — the graph-equivalence check over the full committed
/// inheritance manifest (166 `inherits_from` edges, 3328 `has_function`).
#[test]
fn f1_corpus_sweep_c3_vs_naive_dfs() {
    let triples = manifest();

    // Manifest shape fuses (same fixture as recipe_chaining_collapse.rs).
    let n_inh = triples.iter().filter(|t| t.p == "inherits_from").count();
    let n_hf = triples.iter().filter(|t| t.p == "has_function").count();
    assert_eq!(n_inh, 166, "manifest inherits_from count drifted");
    assert_eq!(n_hf, 3328, "manifest has_function count drifted");

    let mro = Mro::from_triples(&triples);

    let classes: Vec<&str> = mro.classes().collect();
    let multi_base = classes
        .iter()
        .filter(|c| mro.declared_bases(c).len() > 1)
        .count();

    // Every multi-base child's declaration order equals alphabetical order in
    // this manifest (verified independently pre-authoring) — so the corpus
    // CANNOT witness order-sensitivity head-to-head. Pin that property: if a
    // regenerated corpus ever carries a non-alphabetical declaration order,
    // this fuse fires and the sweep gains real discriminating power.
    let non_alphabetical = classes
        .iter()
        .filter(|c| {
            let declared = mro.declared_bases(c);
            let mut sorted = declared.to_vec();
            sorted.sort();
            declared != sorted
        })
        .count();

    // Linearize EVERY class; collect inconsistent hierarchies honestly
    // instead of unwrapping.
    let mut linearized = 0usize;
    let mut inconsistent: Vec<&str> = Vec::new();
    for c in &classes {
        match mro.linearize(c) {
            Ok(_) => linearized += 1,
            Err(MroError::Inconsistent { .. }) => inconsistent.push(c),
            Err(e @ MroError::Cycle { .. }) => panic!("unexpected cycle: {e}"),
        }
    }

    // For every method NAME visible anywhere in a class's linearization,
    // compare C3 resolution vs naive parent-first DFS.
    let mut points = 0usize;
    let mut divergences = 0usize;
    let mut divergent_samples: Vec<(String, String)> = Vec::new();
    for c in &classes {
        let Ok(lin) = mro.linearize(c) else {
            continue; // inconsistent — counted + reported above.
        };
        let visible: BTreeSet<&str> = lin
            .iter()
            .flat_map(|k| mro.own_method_names(k))
            .collect();
        for m in visible {
            points += 1;
            if mro.resolve_c3(c, m) != mro.resolve_naive_dfs(c, m) {
                divergences += 1;
                if divergent_samples.len() < 5 {
                    divergent_samples.push(((*c).to_string(), m.to_string()));
                }
            }
        }
    }

    eprintln!("──── F1 sweep: C3 vs naive parent-first DFS (full inheritance manifest) ────");
    eprintln!("classes                                : {}", classes.len());
    eprintln!("  multi-base (>1 declared base)        : {multi_base}");
    eprintln!("  non-alphabetical declaration order   : {non_alphabetical}");
    eprintln!("linearized (C3 ok)                     : {linearized}");
    eprintln!("C3-INCONSISTENT hierarchies            : {} {inconsistent:?}", inconsistent.len());
    eprintln!("resolution points compared             : {points}");
    eprintln!("C3 vs naive-DFS DIVERGENCES            : {divergences} {divergent_samples:?}");
    eprintln!("──────────────────────────────────────────────────────────────────────────");
    eprintln!("HONEST FINDING: with every multi-base declaration order alphabetical, the");
    eprintln!("committed corpus cannot witness order-sensitivity — 0 divergences here does");
    eprintln!("NOT promote F1 on its own; the diamond fixture carries the falsification.");
    eprintln!("The 3 C3-inconsistent hierarchies each declare mail_thread BEFORE a base");
    eprintln!("whose own MRO contains mail_thread (rating_mixin / product_template) —");
    eprintln!("CPython rejects that shape; naive DFS silently resolves it. Order carries");
    eprintln!("semantics: the named fix is Class.mixins ORDERING, not the delegation walk.");

    // ── Drift-fuses (pinned from this run; a corpus regen re-opens them) ──
    assert_eq!(classes.len(), 388, "class census drifted");
    assert_eq!(multi_base, 53, "multi-base census drifted");
    assert_eq!(
        non_alphabetical, 0,
        "a NON-alphabetical declaration order appeared — the corpus can now \
         witness order-sensitivity; extend the sweep's verdict"
    );
    assert_eq!(linearized, 385, "linearizable census drifted");
    assert_eq!(
        inconsistent,
        ["discuss_channel", "product_product", "product_template"],
        "the C3-inconsistent set drifted"
    );
    assert_eq!(points, 3986, "resolution-point census drifted");
    assert_eq!(
        divergences, 0,
        "the corpus began witnessing C3 vs naive-DFS divergence — F1's \
         corpus arm just gained teeth; report it, don't paper over it"
    );
}
