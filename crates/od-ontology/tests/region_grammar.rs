//! **Region-grammar config pin** — proof-by-execution for the knowledge
//! transfer of ruff PR #76's region grammar into the odoo-rs transcode.
//!
//! `docs/knowledge/ODOO-REGION-GRAMMAR.md` carries #76's six-region pattern
//! (`docked_at`/`tab_order`/`opens_popup` → `{top_bar, left_nav, center,
//! right_panel, bottom_bar, popup}`) onto Odoo's XML view arch. The mapping
//! lives as data — `data/nav/odoo_regions.conf` — consumed by ruff's own
//! `region=` directive (`ruff_spo_triplet::parse`). This test proves the
//! config PARSES through that real directive and produces the pinned
//! arch-token → region map, and that every region is one of the six
//! canonical names (a malformed row can never silently mis-dock).
//!
//! This is the config half of the three-edit recipe (Edit 1 vocab = DONE on
//! ruff main; this pins the convention config). Edits 2 (the ruff Odoo
//! `docked_at` harvester arm) + 3 (the `[regions]` digest section) are the
//! remaining [H] work named in the doc — a full arch→region harvest test
//! lands with the arm.

use ruff_spo_triplet::parse;

/// The six canonical regions (`exam_config.rs:36-37`, verbatim on ruff main).
const CANONICAL_REGIONS: &[&str] = &[
    "top_bar",
    "left_nav",
    "center",
    "right_panel",
    "bottom_bar",
    "popup",
];

/// The committed Odoo arch-token → region convention config.
const ODOO_REGIONS_CONF: &str = include_str!("../../../data/nav/odoo_regions.conf");

/// The full pinned map (drift fuse — a hand-edit to the conf that changes a
/// mapping or adds/drops a token fails here loudly).
const EXPECTED: &[(&str, &str)] = &[
    // top_bar — top action/filter strip
    ("header", "top_bar"),
    ("search", "top_bar"),
    ("filter", "top_bar"),
    // left_nav — left category rail
    ("searchpanel", "left_nav"),
    // center — main body / primary data view
    ("sheet", "center"),
    ("form", "center"),
    ("group", "center"),
    ("field", "center"),
    ("separator", "center"),
    ("list", "center"),
    ("tree", "center"),
    ("kanban", "center"),
    ("notebook", "center"),
    ("page", "center"),
    // right_panel — chatter / activity
    ("chatter", "right_panel"),
    // bottom_bar — wizard dialog action bar
    ("footer", "bottom_bar"),
    // popup — dropdown / cog / act_window target="new"
    ("action_menu", "popup"),
];

#[test]
fn odoo_regions_conf_parses_through_the_ruff_region_directive() {
    let cfg = parse(ODOO_REGIONS_CONF);

    // Every pinned (token -> region) pair is present, exactly once, verbatim.
    for (tok, region) in EXPECTED {
        let hits: Vec<&String> = cfg
            .regions
            .iter()
            .filter(|(t, _)| t == tok)
            .map(|(_, r)| r)
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "dock token {tok:?} must map exactly once (found {})",
            hits.len()
        );
        assert_eq!(hits[0], region, "dock token {tok:?} maps to the wrong region");
    }

    // No extra rows leaked in (comments/blanks dropped, malformed rows dropped
    // by the `:`-split — so the parsed count is exactly the pinned count).
    assert_eq!(
        cfg.regions.len(),
        EXPECTED.len(),
        "odoo_regions.conf has {} region rows; pin expects {}",
        cfg.regions.len(),
        EXPECTED.len()
    );
}

#[test]
fn every_mapped_region_is_one_of_the_six_canonical() {
    let cfg = parse(ODOO_REGIONS_CONF);
    for (tok, region) in &cfg.regions {
        assert!(
            CANONICAL_REGIONS.contains(&region.as_str()),
            "token {tok:?} docks to non-canonical region {region:?} (allowed: {CANONICAL_REGIONS:?})"
        );
    }
}

#[test]
fn all_six_canonical_regions_are_covered_by_at_least_one_token() {
    let cfg = parse(ODOO_REGIONS_CONF);
    for region in CANONICAL_REGIONS {
        assert!(
            cfg.regions.iter().any(|(_, r)| r == region),
            "no Odoo arch token maps to canonical region {region:?} — the six-region \
             frame must be fully reachable from Odoo arch"
        );
    }
}
