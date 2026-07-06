//! **PROBE — body triage / accidentally-imperative ratio** (OGAR F17,
//! `PROBE-OGAR-BODY-TRIAGE`, the Odoo **control leg**).
//!
//! # The question F17 asks
//!
//! Run the body pass over real lifecycle hooks, reduce each hook to
//! `(target, verb-class, order-signature)`, then **round-trip order-free**:
//! does the order-free `(verb, criteria)` fact-set reproduce the source
//! behaviour? PASS = the body was only *accidentally* imperative (its meaning
//! is the unordered fact-set — recoverable into the declarative recipe);
//! FAIL = genuinely order-dependent (the preserve/escape tail).
//!
//! # The Odoo leg is the CONTROL
//!
//! Odoo hooks are *already declarative at the rim* (`@api.depends`,
//! `@api.constrains`, `compute=`), so the write target arrives declaratively —
//! `Field::emitted_by` in the corpus — and the ruff Python frontend
//! deliberately leaves `Function::writes`/`calls` empty (see the F17 ledger
//! line). The control expectation: a HIGH pass-rate, with a small honest
//! order-dependent tail. The *test* leg is Rails `before_*`/`after_*`
//! (`ruff_ruby_spo` populates `writes`/`calls` from the AST there).
//!
//! # Static order-signature (what is decidable from the harvested facts)
//!
//! Per hook: `R` = `reads_field` set, `W` = inverted `emitted_by` set,
//! `raise` = has a `raises` triple. The order-free reconstruction is
//! `verb ∈ {assign W from R, reject on criteria(R)}`:
//!
//!   * **guard-pure** (`raise ∧ W = ∅`) — a pure predicate over `R`; raising
//!     is idempotent and read-only, so evaluation order inside the body cannot
//!     change the outcome → **PASS**.
//!   * **compute-pure** (`W ≠ ∅ ∧ ¬raise ∧ W ∩ R = ∅`) — a (multi-)assignment
//!     that is a pure function of its reads; simultaneous assignment
//!     reproduces it → **PASS**.
//!   * **self-feedback** (`W ∩ R ≠ ∅`) — the hook reads a field it also
//!     emits: a read-modify-write signature, where intra-body order is
//!     load-bearing (which value of the field does the read see?) → **FAIL**.
//!   * **write+raise** (`W ≠ ∅ ∧ raise`) — the raise's position relative to
//!     the writes decides whether a partial write escapes; order-free
//!     reconstruction cannot pick a side → **FAIL**.
//!
//! Cross-hook order is NOT counted against a hook: the recompute DAG already
//! recovers it declaratively (Kahn over `emitted_by → reads_field`), which the
//! probe re-asserts at the end. F17 measures the *intra-body* residue only.
//!
//! # Honest bounds
//!
//!   1. Hooks with no captured facts at all (no reads, no writes, no raises)
//!      are a data gap, not evidence — excluded from the headline as
//!      "unresolved", exactly like the F15 probe treats empty path-sets.
//!   2. `reads_field` mixes body reads with `@api.depends` lifts, so a
//!      self-feedback hit can be an extractor artifact (the recompute DAG
//!      drops such self-loops for *cross-hook* ordering for that reason).
//!      Counting them as FAIL is therefore conservative: the true
//!      order-dependent tail is ≤ the measured one, the pass-rate a LOWER
//!      bound.
//!   3. Method bodies are not content-captured (no statement list), so the
//!      round-trip is over the harvested fact-set, not a token-level replay.
//!
//! Default build — no `ogar-emit`, no git deps — runs offline and in CI.
//! Ledger home: OGAR `docs/INTEGRATION-MAP.md` F17 + `D-ACCIDENTAL-IMPERATIVE`.

// Pedantic lints relaxed for this reporting probe: it prints a detailed
// human-facing report (long fn), computes display percentages from small counts
// (benign usize→f64), and its module docs are narrative (bare identifiers).
#![allow(clippy::too_many_lines, clippy::cast_precision_loss, clippy::doc_markdown)]

use std::collections::{BTreeMap, BTreeSet};

use od_ontology::{parse_ndjson, MethodKind, RecomputeDag, Triple};

/// Strip a known namespace prefix (`odoo:` / `ogit:` / `exc:`) — mirrors the
/// crate-private `triple::strip_ns` (not re-exported), used only for map keys.
fn strip_ns(iri: &str) -> &str {
    iri.split_once(':').map_or(iri, |(_, r)| r)
}

fn corpus() -> Vec<Triple> {
    // Same richest committed corpus the F15 probe uses: account_move +
    // account_move_line + res_partner + res_company.
    let ndjson = include_str!("../../../data/slice_2.spo.ndjson");
    parse_ndjson(ndjson).expect("slice 2 corpus parses")
}

/// The order-free round-trip verdict for one hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    /// Order-free reconstruction reproduces the hook (accidentally imperative).
    Pass,
    /// Intra-body order is load-bearing (genuinely imperative tail).
    Fail,
    /// No facts captured — a data gap, excluded from the headline.
    Unresolved,
}

/// The static verb-class of a hook, from its harvested fact shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VerbClass {
    GuardPure,
    ComputePure,
    SelfFeedback,
    WriteRaise,
    ReadOnly,
    NoFacts,
}

struct Hook {
    kind: MethodKind,
    verb: VerbClass,
    verdict: Verdict,
}

fn triage(triples: &[Triple]) -> BTreeMap<String, Hook> {
    let mut methods: BTreeSet<String> = BTreeSet::new();
    let mut raises: BTreeSet<String> = BTreeSet::new();
    let mut reads: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut writes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for t in triples {
        match t.p.as_str() {
            "has_function" => {
                methods.insert(strip_ns(&t.o).to_string());
            }
            "raises" => {
                raises.insert(strip_ns(&t.s).to_string());
            }
            "reads_field" => {
                reads
                    .entry(strip_ns(&t.s).to_string())
                    .or_default()
                    .insert(strip_ns(&t.o).to_string());
            }
            // (field, emitted_by, method) — the DECLARATIVE write target
            // (Odoo `compute=`), inverted to a per-method write set.
            "emitted_by" => {
                writes
                    .entry(strip_ns(&t.o).to_string())
                    .or_default()
                    .insert(strip_ns(&t.s).to_string());
            }
            _ => {}
        }
    }

    let empty = BTreeSet::new();
    let mut out: BTreeMap<String, Hook> = BTreeMap::new();
    for m in &methods {
        let kind = MethodKind::classify(m);
        // The hook set = the lifecycle families + anything that raises (the
        // guard arm regardless of prefix, matching `corpus_to_actions`).
        if kind == MethodKind::Other && !raises.contains(m) {
            continue;
        }
        let r = reads.get(m).unwrap_or(&empty);
        let w = writes.get(m).unwrap_or(&empty);
        let raising = raises.contains(m);
        let overlap = !w.is_disjoint(r);

        let verb = if w.is_empty() && r.is_empty() && !raising {
            VerbClass::NoFacts
        } else if overlap {
            VerbClass::SelfFeedback
        } else if raising && w.is_empty() {
            VerbClass::GuardPure
        } else if raising {
            VerbClass::WriteRaise
        } else if w.is_empty() {
            VerbClass::ReadOnly
        } else {
            VerbClass::ComputePure
        };

        let verdict = match verb {
            VerbClass::GuardPure | VerbClass::ComputePure => Verdict::Pass,
            VerbClass::SelfFeedback | VerbClass::WriteRaise => Verdict::Fail,
            // Read-only non-raising hooks: for a Compute the write target
            // wasn't captured (data gap); for Onchange/Search the shape is
            // legitimately read-only but the cooperative/write half is
            // uncaptured — either way, not decidable → unresolved.
            VerbClass::ReadOnly | VerbClass::NoFacts => Verdict::Unresolved,
        };

        out.insert(m.clone(), Hook { kind, verb, verdict });
    }
    out
}

#[test]
fn body_triage_accidentally_imperative_ratio() {
    let triples = corpus();
    let hooks = triage(&triples);

    let count_verb = |v: VerbClass| hooks.values().filter(|h| h.verb == v).count();
    let n_guard_pure = count_verb(VerbClass::GuardPure);
    let n_compute_pure = count_verb(VerbClass::ComputePure);
    let n_self_feedback = count_verb(VerbClass::SelfFeedback);
    let n_write_raise = count_verb(VerbClass::WriteRaise);
    let n_read_only = count_verb(VerbClass::ReadOnly);
    let n_no_facts = count_verb(VerbClass::NoFacts);

    // Headline = the behavioural arm F15 measures: guards (anything raising,
    // regardless of prefix — matching `corpus_to_actions`) + computes
    // (`_compute_*`). Onchange/inverse/search are context rows.
    let arm: Vec<(&String, &Hook)> = hooks
        .iter()
        .filter(|(_, h)| {
            h.kind == MethodKind::Compute
                || matches!(h.verb, VerbClass::GuardPure | VerbClass::WriteRaise)
        })
        .collect();

    let resolved: Vec<&Hook> = arm
        .iter()
        .map(|(_, h)| *h)
        .filter(|h| h.verdict != Verdict::Unresolved)
        .collect();
    let n_resolved = resolved.len();
    let n_pass = resolved.iter().filter(|h| h.verdict == Verdict::Pass).count();
    let n_fail = resolved.iter().filter(|h| h.verdict == Verdict::Fail).count();
    let n_unresolved = arm.len() - n_resolved;

    let pct = |num: usize, den: usize| -> f64 {
        if den == 0 { 0.0 } else { 100.0 * num as f64 / den as f64 }
    };
    let pass_pct = pct(n_pass, n_resolved);
    let fail_pct = pct(n_fail, n_resolved);

    // Per-kind rows (context: where the order-dependence lives).
    let mut per_kind: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();
    for h in hooks.values() {
        let k = match h.kind {
            MethodKind::Compute => "compute",
            MethodKind::Check => "check",
            MethodKind::Onchange => "onchange",
            MethodKind::Inverse => "inverse",
            MethodKind::Search => "search",
            MethodKind::Other => "other(raising)",
        };
        let e = per_kind.entry(k).or_default();
        match h.verdict {
            Verdict::Pass => e.0 += 1,
            Verdict::Fail => e.1 += 1,
            Verdict::Unresolved => e.2 += 1,
        }
    }

    eprintln!("──── F17 body triage / accidentally-imperative ratio (slice_2, Odoo control) ────");
    eprintln!("lifecycle hooks        : {}", hooks.len());
    eprintln!("verb-classes           : guard-pure {n_guard_pure} · compute-pure {n_compute_pure} · self-feedback {n_self_feedback} · write+raise {n_write_raise} · read-only {n_read_only} · no-facts {n_no_facts}");
    eprintln!("── per kind (pass / fail / unresolved) ──");
    for (k, (p, f, u)) in &per_kind {
        eprintln!("  {k:<16} {p:>4} / {f:>4} / {u:>4}");
    }
    eprintln!("── headline (behavioural arm: guards + computes, {} hooks, {n_resolved} resolved) ──", arm.len());
    eprintln!("PASS (accidentally imperative, order-free recoverable): {n_pass}  ({pass_pct:.1}%)");
    eprintln!("FAIL (order-dependent tail)                           : {n_fail}  ({fail_pct:.1}%)");
    eprintln!("unresolved (no facts captured — excluded)             : {n_unresolved}");
    eprintln!("────────────────────────────────────────────────────────────────");
    eprintln!("Reading (Odoo = CONTROL leg): Odoo's rim is already declarative, so the");
    eprintln!("pass-rate is expected HIGH; the FAIL tail decomposes into self-feedback");
    eprintln!("(read-modify-write inside one hook — conservative: includes @api.depends");
    eprintln!("extractor artifacts, so true tail ≤ measured) and write+raise (partial-write");
    eprintln!("escape order). Cross-hook order is NOT counted: the recompute DAG recovers");
    eprintln!("it declaratively (asserted below). Test leg = Rails before_*/after_* hooks");
    eprintln!("via ruff_ruby_spo writes/calls.");

    // ── Drift-fuses (exact, from the 2026-07-06 RUN on the committed corpus;
    // a change here means the fixture or the triage semantics moved — re-run
    // and REGRADE the OGAR F17 ledger line, don't just bump the numbers) ──
    assert_eq!(hooks.len(), 393, "lifecycle hook count drifted from the fixture");
    assert_eq!(n_resolved, 354, "resolved behavioural arm drifted");
    assert_eq!(n_pass, 336, "PASS count drifted (was 94.9%)");
    assert_eq!(n_fail, 18, "FAIL count drifted (was 5.1%)");
    assert_eq!(
        (n_self_feedback, n_write_raise),
        (30, 2),
        "order-dependent decomposition drifted (self-feedback, write+raise)"
    );

    // ── Structural invariants ──
    assert!(n_resolved > 100, "expected a substantial resolved arm, saw {n_resolved}");
    assert_eq!(n_pass + n_fail, n_resolved, "pass + fail must cover resolved");
    assert!((pass_pct + fail_pct - 100.0).abs() < 1e-6);
    // Guards are order-free by construction here: a guard that also writes is
    // classified write+raise, never guard-pure.
    assert!(
        hooks.values().all(|h| h.verb != VerbClass::GuardPure || h.verdict == Verdict::Pass),
        "guard-pure hooks must PASS"
    );
    // The Odoo control expectation: mostly declarative already. Loose gate so
    // the fixture can drift a little; the exact ratio is the ledger's number.
    assert!(
        pass_pct > 50.0,
        "Odoo control leg should be majority order-free (saw {pass_pct:.1}%)"
    );
    // Cross-hook order is declaratively recoverable: the compute subset of the
    // recompute DAG has a Kahn order (no cycles) — the reason cross-hook
    // ordering does not count against any single hook's verdict.
    let dag = RecomputeDag::from_triples(&triples).restrict_to(&[MethodKind::Compute]);
    assert!(
        dag.topological_order().is_ok(),
        "compute subset must stay Kahn-orderable (cross-hook order is declarative)"
    );
}
