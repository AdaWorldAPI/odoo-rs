//! **Klickweg structure parity** — the odoo → odoo-rs navigation-topology
//! closure, consuming ruff's Odoo arms (ruff #66/#67) over COMMITTED verbatim
//! fixtures.
//!
//! # What "finalized" means here
//!
//! 1. **Harvest parity** — the upstream `extract_odoo_nav_edges` arm runs
//!    over this repo's committed real-source fixtures (`data/
//!    account_move_real.py`, verbatim, 7 380 lines; `data/nav/*.xml`,
//!    verbatim from the real `account` addon) and its edge set is PINNED.
//!    Both shapes are exercised: Shape A (code-side `act_window` dict
//!    return) and Shape B (data-side action record + menuitem, joined
//!    CROSS-FILE — the menu file and the action records are separate real
//!    files, the exact condition ruff #66 fixed).
//! 2. **Corpus carriage** — the edges lift to `navigates_to` triples that
//!    round-trip through the closed-vocabulary `from_ndjson` (proving
//!    `Predicate::NavigatesTo` rides the same SPO corpus as the core-7),
//!    and `data/account_nav.spo.ndjson` is the committed artifact, byte-
//!    equal to a fresh harvest (drift fuse: fixture or arm moves → this
//!    fails → re-measure, re-pin, say so in the commit).
//! 3. **Connectivity probe** — menu-rooted reachability over the harvested
//!    graph (the odoo-rs analog of op-nexgen's boot-time klickweg check).
//!    lance-graph-contract exposes no nav-connectivity API today, so the
//!    BFS lives in this probe; if/when the contract grows one, this probe
//!    delegates (named ask, not hidden).
//! 4. **View-skin parity** — the upstream `extract_odoo_view_field_sets`
//!    (fourth skin, hop-exact since ruff #67) agrees with this repo's
//!    hop-aware `extract_view_fields` on the shared surface: top-level
//!    this-model fields of the real `account_move_form_view.xml`. The two
//!    harvests can never silently disagree; `relation_hops` stays the
//!    local refinement (see `view_mask.rs`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use od_ontology::extract_view_fields;
use ruff_python_spo::{
    NavVocab, ViewTarget, extract_odoo_nav_edges_with_report,
    extract_odoo_view_field_sets,
};

/// The closed screen vocabulary for the nav harvest: the corpus models plus
/// the menu-wired screens of the vendored real menu file.
const SCREENS: &[&str] = &[
    "account_move",
    "account_move_line",
    "account_account",
    "account_analytic_line",
    "account_tax",
    "res_partner",
    "res_company",
    "account_journal",
    "account_payment",
    "account_bank_statement",
    "account_fiscal_position",
    "account_journal_group",
    "account_incoterms",
];

/// The workspace-root `data/` directory, resolved from this crate's manifest
/// dir (tests run with CWD = the package root, not the workspace root).
fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn vocab() -> NavVocab {
    NavVocab {
        screens: SCREENS.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// 1 + 2 — harvest pins + corpus carriage.
#[test]
fn klickweg_harvest_pins_and_corpus_carriage() {
    let (edges, report) = extract_odoo_nav_edges_with_report(&data_root(), &vocab());

    // ── Harvest pins (2026-07-09 fuses; re-pin deliberately on change) ──
    assert_eq!(report.py_files, 1, "the vendored real account_move.py");
    assert_eq!(report.xml_files, 4, "form-view fixture + 3 vendored nav files");
    assert_eq!(
        report.raw_act_window_refs, 9,
        "act_window dicts in the .py + action records in the nav XMLs"
    );
    let flat: Vec<String> = edges
        .iter()
        .map(|e| format!("{} -> {} [{}]", e.source, e.target, e.via))
        .collect();
    assert_eq!(
        flat,
        vec![
            "account_move -> account_move [act_window_return]",
            "menu -> account_account [menuitem]",
            "menu -> account_analytic_line [menuitem]",
        ],
        "Shape A (code-side) + Shape B (cross-file menu joins) — the pinned \
         Klickweg of the committed fixture slice"
    );

    // ── Corpus carriage: triples round-trip the closed vocabulary ──
    let triples: Vec<_> = edges.iter().map(|e| e.to_triple("odoo")).collect();
    let ndjson = ruff_spo_triplet::to_ndjson(&triples);
    let parsed = ruff_spo_triplet::from_ndjson(&ndjson)
        .expect("navigates_to must be in the closed predicate vocabulary");
    assert_eq!(parsed, triples, "lossless ndjson round-trip");

    // ── Committed-artifact fuse ──
    let committed = std::fs::read_to_string(data_root().join("account_nav.spo.ndjson"))
        .expect("data/account_nav.spo.ndjson is committed");
    assert_eq!(
        ndjson, committed,
        "fresh harvest must byte-match the committed nav corpus — fixture or \
         upstream arm moved; re-generate (examples/kw_measure.rs), re-pin, \
         and say so in the commit"
    );
}

/// 3 — menu-rooted reachability (the boot-check analog, as a probe).
#[test]
fn klickweg_menu_rooted_reachability() {
    let (edges, _) = extract_odoo_nav_edges_with_report(&data_root(), &vocab());

    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &edges {
        adj.entry(e.source.as_str()).or_default().push(e.target.as_str());
    }
    let mut reachable: BTreeSet<&str> = BTreeSet::new();
    let mut queue: VecDeque<&str> = VecDeque::from(["menu"]);
    while let Some(node) = queue.pop_front() {
        for &next in adj.get(node).into_iter().flatten() {
            if reachable.insert(next) {
                queue.push_back(next);
            }
        }
    }

    // Every menuitem-wired screen of the vendored real menu file is
    // menu-reachable (pinned set for this fixture slice).
    assert_eq!(
        reachable.iter().copied().collect::<Vec<_>>(),
        vec!["account_account", "account_analytic_line"],
        "menu-rooted reachable set (2026-07-09 fuse)"
    );

    // Honest residue, reported not hidden: Shape-A-only screens are NOT
    // menu-reachable within this fixture slice — their menu wiring lives in
    // addon files not vendored here (e.g. account_move's own actions in
    // account_move_views.xml). The full-addon probe (ruff
    // examples/odoo_nav_probe.rs) shows menu -> account_move there.
    let sources: BTreeSet<&str> = edges.iter().map(|e| e.source.as_str()).collect();
    assert!(
        sources.contains("account_move") && !reachable.contains("account_move"),
        "the Shape-A component exists and is disjoint from the menu root in \
         THIS slice — if this starts failing, the fixture set grew menu \
         wiring for account_move; update the reachability pin"
    );
}

/// 4 — the fourth skin agrees with the local hop-aware extractor on the
/// shared surface (top-level this-model fields of the REAL form view).
#[test]
fn upstream_view_skin_matches_local_top_level_fields() {
    // Local (hop-aware) harvest of the committed real form view.
    let xml = include_str!("../../../data/account_move_form_view.xml");
    let local = extract_view_fields(xml);
    let form = local
        .iter()
        .find(|v| v.model == "account.move")
        .expect("the fixture carries the account.move form view");
    let local_top: BTreeSet<String> = form.fields.iter().cloned().collect();
    assert!(
        !form.relation_hops.is_empty(),
        "the real form view nests line grids — hops must be present locally \
         (they are the local refinement the upstream set-level arm omits)"
    );

    // Upstream (set-level, hop-exact since ruff #67) harvest of data/.
    let target = ViewTarget {
        model: "account_move".to_string(),
        receivers: vec![], // record-scoped arm; receivers deliberately unused
        fields: local_top.iter().cloned().collect(),
    };
    let sets = extract_odoo_view_field_sets(&data_root(), &[target]);
    let upstream = sets
        .iter()
        .find(|s| s.view.starts_with("account_move_form_view.xml#"))
        .expect("upstream arm harvests the same committed form view");

    let upstream_ref: BTreeSet<String> = upstream.referenced.iter().cloned().collect();
    assert_eq!(
        upstream_ref, local_top,
        "PARITY: upstream `referenced` (depth-0 arch fields) must equal the \
         local extractor's top-level `fields` on the same real view — the \
         two harvests may never silently disagree on the shared surface"
    );
    // And on this input the closed vocab IS the local set, so fields == referenced.
    assert_eq!(upstream.fields, upstream.referenced);
}
