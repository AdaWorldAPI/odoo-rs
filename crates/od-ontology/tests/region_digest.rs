//! **Region digest (Edit 3)** — the consumer half of the region-grammar
//! knowledge transfer, closed end-to-end over REAL Odoo `account` views.
//!
//! Edit 2 (ruff `ruff_python_spo::extract_odoo_view_regions`, branch
//! `claude/odoo-region-grammar-arm`) harvested the vendored `data/nav/*.xml`
//! into `docked_at`/`tab_order`/`opens_popup` triples; the byte-frozen result
//! is carried here as `data/nav/account_regions.spo.ndjson` (corpus carriage,
//! same pattern as `account_nav.spo.ndjson`). This test folds it through the
//! `region=` convention (`odoo_regions.conf`) using ruff's OWN
//! `build_nav_digest` — the consumer REUSES the digest, never reimplements it
//! (no parallel structure).
//!
//! The load-bearing proof: **every dock token the real arm emitted over real
//! Odoo source is covered by the config** (no `unmapped:` leak) and resolves
//! to one of the six canonical regions. That is the structure-oracle render
//! half proven on real data, decoupled from the ruff merge (float on main;
//! when Edit 2 lands, the live harvest replaces the committed corpus).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ruff_python_spo::{extract_odoo_view_regions, region_triples};
use ruff_spo_triplet::{build_nav_digest, from_ndjson, parse, to_ndjson};

/// Repo `data/` root (mirror of `klickweg_parity.rs`).
fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

const REGIONS_NDJSON: &str = include_str!("../../../data/nav/account_regions.spo.ndjson");
const REGIONS_CONF: &str = include_str!("../../../data/nav/odoo_regions.conf");

const CANONICAL_REGIONS: &[&str] = &[
    "top_bar",
    "left_nav",
    "center",
    "right_panel",
    "bottom_bar",
    "popup",
];

#[test]
fn live_harvest_reproduces_the_frozen_corpus() {
    // The loop, closed LIVE (ruff #79 on main): run the real arm over the
    // vendored `account` views TODAY and assert it reproduces the committed
    // corpus byte-for-byte. The frozen ndjson is no longer a hand-authored
    // stand-in — it is exactly what `extract_odoo_view_regions` emits. If the
    // arm intentionally changes, regenerate the corpus (this fails loudly).
    let facts = extract_odoo_view_regions(&data_root().join("nav"));
    let live = to_ndjson(&region_triples(&facts));
    assert_eq!(
        live, REGIONS_NDJSON,
        "the live ruff arm output must equal data/nav/account_regions.spo.ndjson"
    );
}

#[test]
fn region_corpus_round_trips_byte_for_byte() {
    // Corpus carriage drift fuse: the committed ndjson survives a closed-vocab
    // from_ndjson -> to_ndjson round-trip unchanged (a hand-edit or a harvester
    // regression that reshapes a triple fails here).
    let triples = from_ndjson(REGIONS_NDJSON).expect("committed region corpus parses");
    assert!(!triples.is_empty(), "region corpus is non-empty");
    assert_eq!(
        to_ndjson(&triples),
        REGIONS_NDJSON,
        "data/nav/account_regions.spo.ndjson must be byte-stable through the closed vocab"
    );
}

#[test]
fn every_real_dock_token_is_covered_by_the_config() {
    // THE proof: fold the REAL harvest through the REAL config and assert no
    // token leaks. Every `docked_at` object emitted over the vendored account
    // views must be a key in odoo_regions.conf — the render frame is total.
    let triples = from_ndjson(REGIONS_NDJSON).expect("corpus parses");
    let cfg = parse(REGIONS_CONF);
    let mapped: BTreeSet<&str> = cfg.regions.iter().map(|(t, _)| t.as_str()).collect();

    let dock_tokens: BTreeSet<String> = triples
        .iter()
        .filter(|t| t.p == "docked_at")
        .map(|t| t.o.clone())
        .collect();
    assert!(!dock_tokens.is_empty(), "corpus carries docked_at facts");

    for tok in &dock_tokens {
        assert!(
            mapped.contains(tok.as_str()),
            "real dock token {tok:?} is NOT in odoo_regions.conf — it would render \
             `unmapped:{tok}`; add a `region={tok}:<region>` row"
        );
    }
}

#[test]
fn every_resolved_region_is_canonical() {
    let triples = from_ndjson(REGIONS_NDJSON).expect("corpus parses");
    let cfg = parse(REGIONS_CONF);
    let token_to_region: std::collections::HashMap<&str, &str> = cfg
        .regions
        .iter()
        .map(|(t, r)| (t.as_str(), r.as_str()))
        .collect();

    let regions: BTreeSet<&str> = triples
        .iter()
        .filter(|t| t.p == "docked_at")
        .filter_map(|t| token_to_region.get(t.o.as_str()).copied())
        .collect();

    for region in &regions {
        assert!(
            CANONICAL_REGIONS.contains(region),
            "real harvest resolves to non-canonical region {region:?}"
        );
    }
    // The real account views populate more than one region (a genuine frame,
    // not everything-in-center).
    assert!(
        regions.len() >= 2,
        "real account views should populate multiple regions, got {regions:?}"
    );
}

#[test]
fn build_nav_digest_folds_the_regions_section() {
    // Consume ruff's OWN digest builder (no reimplementation). It must produce
    // a non-empty digest that carries the [regions] section for our facts.
    let triples = from_ndjson(REGIONS_NDJSON).expect("corpus parses");
    let cfg = parse(REGIONS_CONF);
    let digest = build_nav_digest(&triples, &cfg);
    assert!(!digest.is_empty(), "digest is non-empty");
    assert!(
        digest.contains("[regions]"),
        "digest must carry the [regions] section (build_nav_digest folds docked_at)"
    );
    // No token leaked as `unmapped:` (the config is total over the real harvest).
    assert!(
        !digest.contains("unmapped:"),
        "no dock token should render `unmapped:` — the config covers the real harvest"
    );
}
