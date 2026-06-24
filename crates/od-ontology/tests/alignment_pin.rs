//! Invariant pins for the Odoo OWL pivot alignment.
//!
//! Cross-validates the static [`ODOO_SEED`] (muscle memory pulled from
//! `lance_graph_callcenter::odoo_alignment` per `specs/REPATRIATION-FRAME.md`
//! Phase 2) against:
//!
//! 1. The SPO corpus — every seeded Odoo class must appear as a Subject
//!    (`rdf:type ogit:ObjectType`) in at least one shipped slice.
//! 2. The documented family-byte registry (`0x61` BillingCore, `0x62`
//!    SMBAccounting, `0x64` ProductCatalog, `0x80` SmbFoundryCustomer,
//!    `0x81` SmbFoundryInvoice, `0x90` HRFoundation).
//! 3. The pivot-URI namespace conventions (`fibo:` / `schema:` / `qudt:` /
//!    `vcard:` / `org:`).
//! 4. The DOLCE classifier behavior pinned to its documented suffix rules.
//! 5. The lookup surface — exact match + `product.*` prefix fallback.

use od_ontology::{
    dolce_odoo, parse_ndjson, resolve_odoo, strip_odoo_prefix, DolceMarker, FAMILY_BILLING_CORE,
    FAMILY_HR_FOUNDATION, FAMILY_PRODUCT_CATALOG, FAMILY_SMB_ACCOUNTING,
    FAMILY_SMB_FOUNDRY_CUSTOMER, FAMILY_SMB_FOUNDRY_INVOICE, ODOO_BUNDLE_ID, ODOO_EDGE_WHITELIST,
    ODOO_INHERITS_FROM_FIBOFND_V1, ODOO_NAMESPACE_IRI, ODOO_SEED, ODOO_TTL_SOURCES,
};

const SLICE_1: &str = include_str!("../../../data/account_move.spo.ndjson");
const SLICE_2: &str = include_str!("../../../data/slice_2.spo.ndjson");

// ── §1: every seeded class is a real Odoo class in at least one slice ──────

/// Every `ODOO_SEED.odoo_class` must appear as a Subject of `rdf:type
/// ogit:ObjectType` in at least one shipped slice — or, for `account.*`-
/// flavoured classes that the corpus emits under the underscore form
/// (`account_move`), via the canonical `<model>.<member>` shape's model prefix.
/// The seed is only useful if its keys refer to real Odoo classes; this catches
/// the case where a seed row goes stale because Odoo renamed the model.
#[test]
fn every_seeded_class_appears_in_a_corpus_slice() {
    let s1 = parse_ndjson(SLICE_1).expect("slice-1 parses");
    let s2 = parse_ndjson(SLICE_2).expect("slice-2 parses");
    let in_any_slice = |odoo_class: &str| -> bool {
        let dotted = odoo_class; // raw form used in the corpus values (target / inverse_name)
        let underscored = odoo_class.replace('.', "_"); // form used in `odoo:<model>` IRIs
        let needles_iri = format!("odoo:{}", underscored);
        let appears = |triples: &[od_ontology::Triple]| -> bool {
            triples.iter().any(|t| {
                // model prefix match in IRI subjects (account_move family covers
                // account_move.amount_total, account_move._compute_amount, …)
                t.s.starts_with(&needles_iri)
                    || t.o == dotted
                    || t.o.starts_with(&format!("odoo:{}", underscored))
            })
        };
        appears(&s1) || appears(&s2)
    };

    let mut missing: Vec<&str> = ODOO_SEED
        .iter()
        .filter(|r| !in_any_slice(r.odoo_class))
        .map(|r| r.odoo_class)
        .collect();
    missing.sort_unstable();

    // We allow seeded classes to NOT appear in our LIMITED slice corpus — the
    // seed reflects the FULL Odoo catalogue, the slices are subsets — but at
    // least the load-bearing financial / partner classes (the ones actually
    // exercised by W3) MUST appear.
    let load_bearing: &[&str] = &[
        "res.partner",
        "account.move",
        "account.move.line",
        "account.account",
        "product.template",
    ];
    for cls in load_bearing {
        assert!(
            in_any_slice(cls),
            "load-bearing seeded class `{cls}` does not appear in any shipped \
             slice — the seed has drifted from the corpus"
        );
    }
    // Informational: which seeded classes are NOT in the slice corpus (HR
    // classes, hr.*, etc. — they're in the seed for the full-corpus consumer,
    // not in our slices).
    eprintln!(
        "informational: {} of {} seeded classes are not in the shipped slice \
         corpora (full-corpus consumers see them): {:?}",
        missing.len(),
        ODOO_SEED.len(),
        missing
    );
}

// ── §2: family bytes match the documented registry ─────────────────────────

#[test]
fn family_bytes_match_documented_registry() {
    // From lance-graph data/family_registry.ttl, Option B binding.
    assert_eq!(FAMILY_BILLING_CORE.raw(), 0x61, "BillingCore = 97");
    assert_eq!(FAMILY_SMB_ACCOUNTING.raw(), 0x62, "SMBAccounting = 98");
    assert_eq!(
        FAMILY_PRODUCT_CATALOG.raw(),
        0x64,
        "ProductCatalog = 100 (NOT 0x63 = MRORepair)"
    );
    assert_eq!(
        FAMILY_SMB_FOUNDRY_CUSTOMER.raw(),
        0x80,
        "SmbFoundryCustomer = 128"
    );
    assert_eq!(
        FAMILY_SMB_FOUNDRY_INVOICE.raw(),
        0x81,
        "SmbFoundryInvoice = 129"
    );
    assert_eq!(FAMILY_HR_FOUNDATION.raw(), 0x90, "HRFoundation = 144");
}

#[test]
fn no_seed_row_uses_an_unregistered_family() {
    const REGISTERED: &[u8] = &[0x61, 0x62, 0x64, 0x80, 0x81, 0x90];
    for row in ODOO_SEED {
        assert!(
            REGISTERED.contains(&row.family.raw()),
            "seed row `{}` uses an unregistered family byte 0x{:02X}",
            row.odoo_class,
            row.family.raw()
        );
    }
}

// ── §3: pivot URIs respect the documented namespace conventions ─────────────

#[test]
fn every_seed_pivot_uri_is_in_a_recognised_namespace() {
    const NAMESPACES: &[&str] = &["fibo:", "schema:", "qudt:", "vcard:", "org:"];
    for row in ODOO_SEED {
        assert!(
            NAMESPACES.iter().any(|ns| row.pivot_uri.starts_with(ns)),
            "seed row `{}` has pivot_uri `{}` outside the recognised \
             namespaces (fibo/schema/qudt/vcard/org)",
            row.odoo_class,
            row.pivot_uri
        );
    }
}

#[test]
fn pivot_namespace_and_family_are_consistent() {
    // Soft consistency: customers land on SmbFoundryCustomer, transactions on
    // SmbFoundryInvoice, accounts on SMBAccounting, billable goods on
    // BillingCore, catalogue structure on ProductCatalog, HR data on
    // HRFoundation. Catches a seed row that puts e.g. a vcard pivot on
    // BillingCore.
    for row in ODOO_SEED {
        let ok = match (row.family, row.pivot_uri) {
            (FAMILY_SMB_FOUNDRY_CUSTOMER, p) => p.starts_with("fibo:"),
            (FAMILY_SMB_FOUNDRY_INVOICE, p) => p.starts_with("fibo:"),
            (FAMILY_SMB_ACCOUNTING, p) => p.starts_with("fibo:"),
            (FAMILY_BILLING_CORE, p) => p.starts_with("schema:"),
            (FAMILY_PRODUCT_CATALOG, p) => p.starts_with("schema:") || p.starts_with("qudt:"),
            (FAMILY_HR_FOUNDATION, p) => {
                p.starts_with("vcard:") || p.starts_with("org:") || p.starts_with("fibo:")
            }
            _ => true,
        };
        assert!(
            ok,
            "seed `{}` mismatch: pivot `{}` on family 0x{:02X}",
            row.odoo_class,
            row.pivot_uri,
            row.family.raw()
        );
    }
}

// ── §4: DOLCE classifier honors the documented suffix rules ────────────────

#[test]
fn dolce_classifier_perdurant_suffix_rules() {
    assert_eq!(dolce_odoo("account.move"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("account.move.line"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("account.payment"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("sale.order"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("sale.order.line"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("stock.move"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("stock.picking"), DolceMarker::Perdurant);
}

#[test]
fn dolce_classifier_abstract_suffix_rules() {
    assert_eq!(dolce_odoo("account.tax"), DolceMarker::Abstract);
    assert_eq!(dolce_odoo("account.fiscal.position"), DolceMarker::Abstract);
    assert_eq!(dolce_odoo("account.reconcile.model"), DolceMarker::Abstract);
    assert_eq!(dolce_odoo("account.payment.term"), DolceMarker::Abstract);
}

#[test]
fn dolce_classifier_quality_and_endurant_rules() {
    assert_eq!(dolce_odoo("uom.uom"), DolceMarker::Quality);
    assert_eq!(dolce_odoo("uom.category"), DolceMarker::Quality);
    assert_eq!(dolce_odoo("res.partner"), DolceMarker::Endurant);
    assert_eq!(dolce_odoo("res.users"), DolceMarker::Endurant);
    assert_eq!(dolce_odoo("account.account"), DolceMarker::Endurant);
    assert_eq!(dolce_odoo("product.template"), DolceMarker::Endurant);
    assert_eq!(dolce_odoo("product.product"), DolceMarker::Endurant);
}

#[test]
fn dolce_classifier_unknown_for_unmatched() {
    assert_eq!(dolce_odoo("ir.cron"), DolceMarker::Unknown);
    assert_eq!(dolce_odoo("ir.actions.act_window"), DolceMarker::Unknown);
}

#[test]
fn every_seed_rows_dolce_matches_its_classifier_assignment() {
    // Soft pin: if the seed row's DOLCE marker disagrees with the classifier's
    // marker for the same class, one of them is wrong. The seed is authoritative
    // (declared in the row); a divergence means the classifier needs the rule.
    // Tracks classifier-coverage gaps for the deferred Phase-3 universal
    // empowerment push.
    let mut gaps: Vec<(&'static str, DolceMarker, DolceMarker)> = Vec::new();
    for row in ODOO_SEED {
        let classifier = dolce_odoo(row.odoo_class);
        if classifier != row.dolce && classifier != DolceMarker::Unknown {
            gaps.push((row.odoo_class, row.dolce, classifier));
        }
    }
    assert!(
        gaps.is_empty(),
        "classifier disagrees with the seed (NOT Unknown): {gaps:?}"
    );
}

// ── §5: resolve_odoo lookup surface ─────────────────────────────────────────

#[test]
fn resolve_odoo_exact_match_returns_seed_row() {
    let p = resolve_odoo("res.partner").expect("seeded");
    assert_eq!(p.pivot_uri, "fibo:LegalEntity");
    assert_eq!(p.family, FAMILY_SMB_FOUNDRY_CUSTOMER);
    assert_eq!(p.slot, 1);
    assert_eq!(p.dolce, DolceMarker::Endurant);

    let p = resolve_odoo("account.move").expect("seeded");
    assert_eq!(p.pivot_uri, "fibo:Transaction");
    assert_eq!(p.family, FAMILY_SMB_FOUNDRY_INVOICE);

    let p = resolve_odoo("uom.uom").expect("seeded");
    assert_eq!(p.pivot_uri, "qudt:Unit");
    assert_eq!(p.family, FAMILY_PRODUCT_CATALOG);
    assert_eq!(p.dolce, DolceMarker::Quality);
}

#[test]
fn resolve_odoo_unseen_product_subtype_inherits_generic_slot() {
    // product.attribute hits the product.* prefix fallback. Classifier returns
    // Endurant (default product.* rule after no Perdurant/Abstract/Quality
    // match), so the pivot's DOLCE matches the master-record interpretation.
    let p = resolve_odoo("product.attribute").expect("product.* prefix fallback");
    assert_eq!(p.pivot_uri, "schema:Product");
    assert_eq!(p.family, FAMILY_BILLING_CORE);
    assert_eq!(p.slot, 1); // product.template's slot
    assert_eq!(p.dolce, DolceMarker::Endurant);
}

#[test]
fn resolve_odoo_product_category_inherits_pivot_with_quality_dolce() {
    // The richer classifier (PR #15) classifies `*.category` as Quality.
    // `product.category` therefore resolves via `product.*` prefix fallback
    // for the pivot+family+slot, but its DOLCE is Quality not Endurant —
    // the marker correctly reflects that category is a classification, not
    // a persistent product. (Phase-3 OGAR-side will decide whether the
    // pivot itself should differ for Quality classes.)
    let p = resolve_odoo("product.category").expect("product.* prefix fallback");
    assert_eq!(p.pivot_uri, "schema:Product"); // inherited via product.template
    assert_eq!(p.family, FAMILY_BILLING_CORE);
    assert_eq!(p.dolce, DolceMarker::Quality);
}

// ── §7: richer dolce_odoo classifier (PR #15 — pulled from hydrators) ──────

#[test]
fn dolce_classifier_handles_prefixed_iris() {
    // Bare model + odoo: prefix + full namespace IRI all resolve to the same
    // category. Pulled from `lance_graph_ontology::hydrators::dolce_odoo`.
    assert_eq!(dolce_odoo("res.partner"), DolceMarker::Endurant);
    assert_eq!(dolce_odoo("odoo:res.partner"), DolceMarker::Endurant);
    assert_eq!(
        dolce_odoo("https://ada.world/onto/odoo#res.partner"),
        DolceMarker::Endurant
    );
}

#[test]
fn strip_odoo_prefix_handles_all_three_forms() {
    assert_eq!(strip_odoo_prefix("res.partner"), "res.partner");
    assert_eq!(strip_odoo_prefix("odoo:res.partner"), "res.partner");
    assert_eq!(
        strip_odoo_prefix("https://ada.world/onto/odoo#account.move"),
        "account.move"
    );
}

#[test]
fn dolce_classifier_new_perdurant_suffix_rules() {
    // Pulled from `hydrators::dolce_odoo::PERDURANT_SUFFIXES`.
    assert_eq!(dolce_odoo("mail.message"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("mail.activity"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("hr.attendance"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("calendar.event"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("audit.log"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("payment.transaction"), DolceMarker::Perdurant);
    assert_eq!(dolce_odoo("stock.scrap"), DolceMarker::Perdurant);
}

#[test]
fn dolce_classifier_new_quality_suffix_rules() {
    // Pulled from `hydrators::dolce_odoo::QUALITY_SUFFIXES` (minus `.tax`,
    // which we keep as Abstract — see § "Two-classifier disagreement" in
    // `alignment.rs`).
    assert_eq!(dolce_odoo("crm.tag"), DolceMarker::Quality);
    assert_eq!(dolce_odoo("account.account.type"), DolceMarker::Quality);
    assert_eq!(dolce_odoo("res.groups"), DolceMarker::Quality);
    assert_eq!(dolce_odoo("product.category"), DolceMarker::Quality);
}

#[test]
fn dolce_classifier_new_abstract_suffix_rules() {
    // Pulled from `hydrators::dolce_odoo::ABSTRACT_SUFFIXES`.
    assert_eq!(dolce_odoo("mail.template"), DolceMarker::Abstract);
    assert_eq!(dolce_odoo("res.config.settings"), DolceMarker::Abstract);
    // .policy / .rule / .formula are pulled but not test-exercised by any
    // shipped model name in the slice corpora; the suffix rule is present so
    // any future Odoo class ending in `.policy` lands correctly.
}

#[test]
fn dolce_classifier_keeps_endurant_special_cases() {
    // `product.template` and `account.account.template` are master records,
    // NOT abstract config templates. Must NOT be swept into Abstract by
    // the `.template` suffix rule.
    assert_eq!(dolce_odoo("product.template"), DolceMarker::Endurant);
    assert_eq!(
        dolce_odoo("account.account.template"),
        DolceMarker::Endurant
    );
}

#[test]
fn dolce_classifier_documented_disagreement_tax_stays_abstract() {
    // Disagreement with `lance_graph_ontology::hydrators::dolce_odoo` (which
    // classifies `.tax` as Quality). This crate keeps `.tax` → Abstract
    // (matches `lance_graph_callcenter::odoo_alignment` PR #14 pull).
    // Phase-3 OGAR-side will canonically resolve.
    assert_eq!(dolce_odoo("account.tax"), DolceMarker::Abstract);
}

// ── §8: namespace identity + cascade edge whitelist ────────────────────────

#[test]
fn namespace_iri_matches_lance_graph_hydrator() {
    assert_eq!(ODOO_NAMESPACE_IRI, "https://ada.world/onto/odoo#");
}

#[test]
fn odoo_bundle_inherits_from_fibo_foundations() {
    // The literal "Odoo inheriting classes" namespace declaration: Odoo
    // (bundle 0x0002) inherits from FIBO Foundations (bundle 0x0007). The
    // per-class equivalentClass rows in ODOO_SEED are the instances of this
    // inheritance; these constants are the declaration.
    assert_eq!(ODOO_BUNDLE_ID.graph, 0x0002);
    assert_eq!(ODOO_BUNDLE_ID.version, 1);
    assert_eq!(ODOO_INHERITS_FROM_FIBOFND_V1.graph, 0x0007);
    assert_eq!(ODOO_INHERITS_FROM_FIBOFND_V1.version, 1);
}

#[test]
fn cascade_edge_whitelist_has_the_two_load_bearing_iris() {
    // Per the compiler-AST frame: these are the AST-edge types the symbol
    // table follows. `rdfs:subClassOf` (facet subsumption) and
    // `owl:equivalentClass` (Layer-2 alignment pivots into FIBO/schema/QUDT)
    // are LOAD-BEARING — removing either breaks the cascade.
    assert!(
        ODOO_EDGE_WHITELIST.contains(&"http://www.w3.org/2000/01/rdf-schema#subClassOf"),
        "rdfs:subClassOf is load-bearing"
    );
    assert!(
        ODOO_EDGE_WHITELIST.contains(&"http://www.w3.org/2002/07/owl#equivalentClass"),
        "owl:equivalentClass is load-bearing"
    );
}

#[test]
fn cascade_edge_whitelist_covers_property_alignments_too() {
    // Field-level alignments (e.g. `odoo:res.partner.name owl:equivalentProperty
    // foaf:name`) require the property variants of the two load-bearing edges.
    assert!(
        ODOO_EDGE_WHITELIST.contains(&"http://www.w3.org/2000/01/rdf-schema#subPropertyOf"),
        "rdfs:subPropertyOf carries field-level subsumption"
    );
    assert!(
        ODOO_EDGE_WHITELIST.contains(&"http://www.w3.org/2002/07/owl#equivalentProperty"),
        "owl:equivalentProperty carries field-level alignments"
    );
}

#[test]
fn ttl_sources_list_the_three_documented_files() {
    // Mirrors `lance_graph_ontology::hydrators::odoo::ODOO_CORE_RELATIVE_PATH`
    // + `ODOO_ALIGNMENT_RELATIVE_PATHS`. odoo-rs ships the SPO ndjson form,
    // not these TTL files — they're documented here so the consumer knows
    // what the harvest pipeline reads upstream.
    assert!(ODOO_TTL_SOURCES.contains(&"data/ontologies/odoo/odoo-core.ttl"));
    assert!(
        ODOO_TTL_SOURCES.contains(&"data/ontologies/odoo/alignment/odoo-to-fibo.ttl"),
        "odoo-to-fibo TTL is the Layer-2 alignment overlay"
    );
    assert!(
        ODOO_TTL_SOURCES.contains(&"data/ontologies/odoo/alignment/odoo-to-skr.ttl"),
        "odoo-to-skr TTL carries SKR03/04 chart-of-accounts alignment"
    );
}

#[test]
fn resolve_odoo_unmapped_returns_none() {
    // Per Option B: unmapped class → None is the "needs a Layer-2 alignment
    // axiom" signal, NOT a minted family.
    assert!(resolve_odoo("stock.move").is_none());
    assert!(resolve_odoo("sale.order").is_none());
    assert!(resolve_odoo("account.reconcile.model").is_none());
}

// ── §6: cross-validate with OGAR's OdooPort identity codebook ──────────────

#[cfg(feature = "ogar-emit")]
#[test]
fn seeded_classes_have_compatible_ogar_identity() {
    use ogar_vocab::ports::{OdooPort, PortSpec};
    // Cross-axis identity check.
    //
    // The two surfaces are **complementary axes** of the same Odoo identity:
    //   - This crate's alignment table = "which FIBO pivot + family/slot"
    //     (covers six basins: BillingCore + SMBAccounting + ProductCatalog +
    //     SmbFoundryCustomer + SmbFoundryInvoice + HRFoundation).
    //   - OGAR's `OdooPort` aliases = "which canonical OGAR class_id"
    //     (currently covers the **commerce arm** only — `0x02XX` ids:
    //     COMMERCIAL_DOCUMENT, COMMERCIAL_LINE_ITEM, TAX_POLICY,
    //     BILLING_PARTY, PAYMENT_RECORD, CURRENCY_POLICY, plus the
    //     cross-arm BILLABLE_WORK_ENTRY).
    //
    // The intersection MUST agree (every commerce-arm seed row has an OdooPort
    // classid); the difference (product / accounting / HR seed rows without an
    // OdooPort alias) IS the surface for a future OdooPort PR and is reported
    // as informational rather than asserted.
    // After OGAR #111 + #126 + #127 (lance-graph #597 + #606 + #608), the
    // cross-axis identity gap surfaced by this crate's PR #14 is fully
    // closed: every seeded class has a canonical OGAR classid. The hard
    // assertion now covers ALL 15 seeded classes (was commerce-arm 3 only
    // pre-#111). Re-introducing a un-mapped class fires this test
    // immediately.
    let mut without_classid: Vec<&str> = Vec::new();
    for row in ODOO_SEED {
        if OdooPort::class_id(row.odoo_class).is_none() {
            without_classid.push(row.odoo_class);
        }
    }
    without_classid.sort_unstable();
    assert!(
        without_classid.is_empty(),
        "the cross-axis identity gap was closed by OGAR #111 + #126 + #127 — \
         re-introducing un-mapped seeded class(es) regresses the closure: {:?}",
        without_classid
    );

}
