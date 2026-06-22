//! OWL pivot alignment — the **symbol table** of the Odoo language frontend.
//!
//! # The compiler-AST role
//!
//! Per `triple.rs` § "The compiler frame": lance-graph on OGAR is acting like
//! a compiler. This module IS the **symbol table / type-resolver** for the
//! Odoo language frontend — Odoo class name → OGAR `(family, slot)` identity
//! + `owl:equivalentClass` pivot URI (`fibo:LegalEntity`, `fibo:Transaction`,
//!   `schema:Product`, `qudt:Unit`, …) + DOLCE upper marker.
//!
//! ```text
//!   odoo class ──owl:equivalentClass──► OWL pivot URI ──► OGIT family + slot
//!              resolve_odoo()           (fibo / schema /   (Option B: inherit
//!                                        qudt / vcard /     existing family;
//!                                        org)               never mint new)
//! ```
//!
//! # Empowerment shapes (universal, eventual OGAR push)
//!
//! Three universal patterns this module deposits — Phase 3 of the
//! `specs/REPATRIATION-FRAME.md` will push them to OGAR as universal
//! `Class` capabilities so other source languages (Rails, Elixir, Django, …)
//! reuse them:
//!
//! - **[`OwlPivot`]** — the resolved landing point shape `(pivot_uri, family,
//!   slot, dolce)`. Any source-language→OGAR alignment table has rows of this
//!   shape; the names are universal.
//! - **[`dolce_odoo`]** — suffix-pattern DOLCE classifier. The *mechanism* is
//!   universal (suffix-match → upper-category marker); the *patterns* are
//!   Odoo-specific muscle memory. The mechanism splits cleanly: a parameterised
//!   classifier with the patterns supplied per-language.
//! - **[`resolve_odoo`]** — symbol-table resolution with prefix fallback. Same
//!   shape for every per-language symbol table.
//!
//! # Muscle memory (Odoo specifics, stays here)
//!
//! - The 15 [`ODOO_SEED`] rows — `res.partner → fibo:LegalEntity`,
//!   `account.move → fibo:Transaction`, … These are declarations of Odoo's
//!   convention for which FIBO / schema.org / QUDT concept each ORM model
//!   instantiates. Pure muscle memory; the per-row knowledge IS Odoo.
//! - The DOLCE suffix patterns — `.move` ⇒ Perdurant, `res.` ⇒ Endurant,
//!   `.tax` ⇒ Abstract, `uom.` ⇒ Quality. Odoo naming conventions.
//! - The 6 family-byte constants — `BillingCore = 0x61`, `SMBAccounting =
//!   0x62`, etc. These restate the bytes assigned in lance-graph's
//!   `data/family_registry.ttl`; the assignments are stable and authoritative.
//!
//! # Identity (in OGAR codebook — `OdooPort`)
//!
//! The fact that "Odoo's `res.partner` is a customer" lives canonically in
//! OGAR's `OdooPort::class_id("res.partner") → CUSTOMER` (PR #94/#95). This
//! module's seed table carries the *complementary* axis — which FIBO/schema
//! pivot + DOLCE marker each Odoo class lands on — and the
//! [`tests/alignment_pin.rs`](`super`) tests cross-validate the two.
//!
//! # Phase-2 pull provenance
//!
//! Pulled from `lance_graph_callcenter::odoo_alignment` per
//! `specs/REPATRIATION-FRAME.md`. The lance-graph file additionally wires
//! into `OgitFamilyTable` (runtime hydration) and `StyleCluster` (the
//! family-default-style inheritance, deferred to Phase 3); the static
//! alignment data + the classifier + the lookup surface land here.

// ═══════════════════════════════════════════════════════════════════════════
// Namespace identity — Odoo inherits from FIBO Foundations (Layer-1 declaration)
// ═══════════════════════════════════════════════════════════════════════════

/// The Odoo TTL namespace IRI. Every Odoo concept declared in the harvested
/// ontology lives under this base.
pub const ODOO_NAMESPACE_IRI: &str = "https://ada.world/onto/odoo#";

/// The Odoo OGAR ontology identity bundle. Pulled from
/// `lance_graph_ontology::hydrators::odoo` (Phase-2 repatriation).
/// Mirrors the lance-graph-ontology constants used to register the Odoo TTL
/// bundle into the `OntologyRegistry`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OntologyBundleId {
    /// The bundle's graph id. `0x0002 = "Odoo"` per OGAR's app-prefix
    /// allocation (`OdooPort::APP_PREFIX`).
    pub graph: u32,
    /// Schema version major (incremented on breaking schema changes).
    pub version: u32,
}

/// `OGIT::ODOO_V1` — the Odoo ontology identity bundle.
pub const ODOO_BUNDLE_ID: OntologyBundleId = OntologyBundleId {
    graph: 0x0002,
    version: 1,
};

/// `OGIT::FIBOFND_V1` — the FIBO Foundations bundle Odoo inherits from.
/// The cascade resolves Odoo concepts THROUGH this bundle (a Layer-1
/// inheritance: `inherits_from: Some(OGIT::FIBOFND_V1.0)` in lance-graph's
/// hydrator).
///
/// **This is the literal "Odoo inheriting classes" declaration** — the
/// namespace-level statement that Odoo's classes are equivalent to / subclass
/// of FIBO Foundations concepts. The per-class `owl:equivalentClass` rows in
/// [`ODOO_SEED`] are the instances of this inheritance; this constant is the
/// declaration.
pub const ODOO_INHERITS_FROM_FIBOFND_V1: OntologyBundleId = OntologyBundleId {
    graph: 0x0007, // FIBO Foundations
    version: 1,
};

/// Cascade edge-IRI whitelist for the Odoo surface — the RDF predicates the
/// symbol-table follows during transitive resolution. Pulled from
/// `lance_graph_ontology::hydrators::odoo::ODOO_EDGE_WHITELIST`.
///
/// Per the compiler-AST frame (PR #13): these ARE the **AST-edge types** the
/// symbol-table walks. `rdfs:subClassOf` carries Odoo's facet subsumption
/// (`odoo:res.partner.Company ⊑ odoo:res.partner`); `owl:equivalentClass`
/// carries the Layer-2 alignment pivots that route into FIBO / schema.org /
/// QUDT slots (the [`ODOO_SEED`] rows). The property variants
/// (`rdfs:subPropertyOf` / `owl:equivalentProperty`) cover field-level
/// alignments (`odoo:res.partner.name owl:equivalentProperty foaf:name`, etc.).
///
/// The first two are LOAD-BEARING — removing either breaks the symbol-table
/// (the `pivot_namespace_and_family_are_consistent` pin assumes
/// `owl:equivalentClass` resolves).
pub const ODOO_EDGE_WHITELIST: &[&str] = &[
    // Odoo facet subsumption (load-bearing — REQUIRED for the cascade)
    "http://www.w3.org/2000/01/rdf-schema#subClassOf",
    // Layer-2 alignment pivots into FIBO/schema/QUDT (load-bearing — REQUIRED)
    "http://www.w3.org/2002/07/owl#equivalentClass",
    // Field-level alignments
    "http://www.w3.org/2000/01/rdf-schema#subPropertyOf",
    "http://www.w3.org/2002/07/owl#equivalentProperty",
];

/// The shipped TTL files this crate's alignment derives from. Informational —
/// odoo-rs ships the SPO ndjson form (under `data/*.spo.ndjson`), not these
/// TTL files; the harvest pipeline that produces both runs upstream. These
/// path tails are the lance-graph-ontology runtime's source.
pub const ODOO_TTL_SOURCES: &[&str] = &[
    "data/ontologies/odoo/odoo-core.ttl",
    "data/ontologies/odoo/alignment/odoo-to-fibo.ttl",
    "data/ontologies/odoo/alignment/odoo-to-skr.ttl",
];

// ═══════════════════════════════════════════════════════════════════════════
// Family bytes — restated from data/family_registry.ttl (Option B)
// ═══════════════════════════════════════════════════════════════════════════

/// A foundry family byte — the basin an OGIT identity lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OdooFamily(pub u8);

impl OdooFamily {
    /// The raw byte.
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// `ogit:BillingCore` — billable items / billing surface. familyId 97 (`0x61`).
pub const FAMILY_BILLING_CORE: OdooFamily = OdooFamily(0x61);
/// `ogit:SMBAccounting` — double-entry substrate (accounts, posting lines).
/// familyId 98 (`0x62`).
pub const FAMILY_SMB_ACCOUNTING: OdooFamily = OdooFamily(0x62);
/// `ogit:ProductCatalog` — product catalogue + pricelist + `UoM`. familyId 100
/// (`0x64`). NOTE: the lance-graph family registry assigns `0x64`, NOT the
/// `0x63` (=99 `ogit:MRORepair`) named in earlier proposals.
pub const FAMILY_PRODUCT_CATALOG: OdooFamily = OdooFamily(0x64);
/// `ogit:SmbFoundryCustomer` — partner / legal-entity master data. familyId
/// 128 (`0x80`).
pub const FAMILY_SMB_FOUNDRY_CUSTOMER: OdooFamily = OdooFamily(0x80);
/// `ogit:SmbFoundryInvoice` — invoice / transaction document. familyId 129
/// (`0x81`).
pub const FAMILY_SMB_FOUNDRY_INVOICE: OdooFamily = OdooFamily(0x81);
/// `ogit:HRFoundation` — employee / org / job / base-contract. familyId 144
/// (`0x90`). Base HR data only; payroll engine is Odoo Enterprise (absent).
pub const FAMILY_HR_FOUNDATION: OdooFamily = OdooFamily(0x90);

// ═══════════════════════════════════════════════════════════════════════════
// DolceMarker — DOLCE upper-category marker
// ═══════════════════════════════════════════════════════════════════════════

/// DOLCE upper-ontology marker — the topmost category an Odoo class lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DolceMarker {
    /// Persistent objects — `res.partner`, `account.account`, `product.template`.
    Endurant,
    /// Transactional events / processes — `account.move`, `sale.order`, `stock.move`.
    Perdurant,
    /// Dimensions of comparison — `uom.uom`, `uom.category`.
    Quality,
    /// Rules, classifications, models — `account.tax`, `account.fiscal.position`.
    Abstract,
    /// No suffix rule matched.
    Unknown,
}

// ═══════════════════════════════════════════════════════════════════════════
// OwlPivot — the empowerment shape (universal across source languages)
// ═══════════════════════════════════════════════════════════════════════════

/// The resolved `owl:equivalentClass` landing for an Odoo class: the pivot
/// URI, the inherited foundry family + slot, and the DOLCE marker. This
/// **shape** is universal — any per-language alignment table has rows of this
/// shape. The contents are per-language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwlPivot {
    /// `owl:equivalentClass` target, e.g. `"fibo:LegalEntity"`,
    /// `"fibo:Transaction"`, `"schema:Product"`.
    pub pivot_uri: &'static str,
    /// Inherited foundry basin.
    pub family: OdooFamily,
    /// Inherited within-family slot.
    pub slot: u16,
    /// DOLCE upper marker (Endurant / Perdurant / Quality / Abstract).
    pub dolce: DolceMarker,
}

// ═══════════════════════════════════════════════════════════════════════════
// dolce_odoo — DOLCE marker from Odoo class suffix rules (muscle memory)
// ═══════════════════════════════════════════════════════════════════════════

/// Classify an Odoo class onto its DOLCE upper marker from structural suffix
/// rules. Independent of the [`ODOO_SEED`] table so unmapped-but-recognisable
/// classes (e.g. `sale.order`) still get a marker the consumer can record.
/// Accepts a bare model name (`"res.partner"`) or a prefixed IRI
/// (`"odoo:res.partner"`).
///
/// Resolution order (first match wins):
///
/// 1. **Endurant special-cases** — `product.template` and `account.account.template`
///    are *master data*, NOT abstract config templates (odoo's `.template` here means
///    "the master record"). Must be checked before the `.template` Abstract rule.
/// 2. **Perdurant** — transactional events / processes: `*.move.line`, `*.move`,
///    `*.payment`, `*.order.line`, `*.order`, `*.message`, `*.activity`,
///    `*.attendance`, `*.transition`, `*.event`, `*.log`, `*.history`,
///    `*.transaction`, `*.picking`, `*.scrap`, `bank.statement`, `stock.move`.
/// 3. **Abstract** — rules / classifications / templates: `*.tax`,
///    `*.fiscal.position`, `*.reconcile.model`, `*.payment.term`,
///    `*.pricelist.item`, `*.pricelist`, `*.template`, `*.config`, `*.policy`,
///    `*.rule`, `*.formula`.
/// 4. **Quality** — dimensions / classifications / rates: `uom.*`, `*.tag`,
///    `*.type`, `*.group`, `*.category`.
/// 5. **Endurant** — persistent master-data fallback: `res.*`, `*.account`,
///    `product.*`.
/// 6. **Unknown** — no rule matched (deliberately NOT a default-to-Endurant
///    rule, contra `lance_graph_ontology::hydrators::dolce_odoo::classify_odoo`
///    which defaults to Endurant; see § "Two-classifier disagreement" below).
///
/// # Classifier provenance — two lance-graph classifiers merged here
///
/// lance-graph hosts **two** disagreeing DOLCE classifiers for Odoo:
///
/// - `lance_graph_callcenter::odoo_alignment::dolce_odoo` (pulled in PR #14)
///   — fewer suffix rules, returns `Unknown` for unmatched.
/// - `lance_graph_ontology::hydrators::dolce_odoo::classify_odoo` (pulled here)
///   — richer suffix lists (3-way split into Perdurant / Quality / Abstract),
///   defaults to `Endurant` for unmatched.
///
/// This implementation merges the **richer** suffix lists from the hydrator
/// while keeping the alignment-side `Unknown` default and the alignment seed's
/// disambiguation of `.tax` and the `.template` special-cases. See §
/// "Two-classifier disagreement" for the explicit reconciliation table.
///
/// # Two-classifier disagreement (findings, deliberate reconciliations)
///
/// | Case | callcenter (PR #14) | ontology hydrator | This crate (reconciled) | Why |
/// |---|---|---|---|---|
/// | `.tax` | Abstract | Quality | **Abstract** | A tax IS a rule, not just a rate. Matches the [`ODOO_SEED`] (no row, but consistent with `payment.term` / `fiscal.position`). Phase-3 OGAR-side may revisit. |
/// | `.tag` / `.type` / `.group` / `.category` | unmatched → Endurant via fallback | Quality | **Quality** | Classifications. The hydrator is correct: a category IS a way to classify, a Quality dimension. |
/// | `.template` | unmatched → Endurant via product.* | Abstract (with `product.template` special-case) | **Abstract** with `product.template` AND `account.account.template` special-cases | Templates ARE configurations (Abstract) — except where Odoo uses "template" for master records (the two SKR-aligned exceptions). |
/// | `hr.*` | Unknown | Endurant by default | **Unknown** | Deliberate: the seam table disagrees with the seed (`hr.job` / `hr.contract` are Abstract per the seed) and the hydrator (Endurant by default). Keeping Unknown surfaces the gap rather than picking one. |
/// | Default | Unknown | Endurant | **Unknown** | Unknown signals "no rule matched" — the consumer can choose to default. Hiding this behind a fallback obscures the gap surface. |
///
/// The reconciliations are CONJECTURE-grade — Phase 3's universal Class-side
/// push to OGAR is where the canonical resolution lands.
#[must_use]
#[allow(clippy::case_sensitive_file_extension_comparisons)] // Odoo class-name suffixes are not file extensions
pub fn dolce_odoo(class: &str) -> DolceMarker {
    let model = strip_odoo_prefix(class);

    // (1) Endurant special-cases — Odoo uses `.template` here for the master
    //     record, NOT an abstract config template. Must check BEFORE the
    //     `.template` Abstract rule below.
    if model == "product.template" || model == "account.account.template" {
        return DolceMarker::Endurant;
    }

    // (2) Perdurant — transactional events / processes.
    if model.ends_with(".move.line")
        || model.ends_with(".move")
        || model.ends_with(".payment")
        || model.ends_with(".order.line")
        || model.ends_with(".order")
        || model.ends_with(".message")
        || model.ends_with(".activity")
        || model.ends_with(".attendance")
        || model.ends_with(".transition")
        || model.ends_with(".event")
        || model.ends_with(".log")
        || model.ends_with(".history")
        || model.ends_with(".transaction")
        || model.ends_with(".picking")
        || model.ends_with(".scrap")
        || model.ends_with("bank.statement")
        || model == "stock.move"
    {
        return DolceMarker::Perdurant;
    }

    // (3) Abstract — rules / policies / templates / classifications.
    //     NOTE: `.settings` is the real catch for `*.config.settings` models
    //     (the canonical Odoo settings shape, e.g. `sale.config.settings`).
    //     The hydrator-side `.config` rule had a misleading comment claiming
    //     to match `*.config.settings` but did not — `res.config.settings`
    //     ends with `.settings`, not `.config`. Both are listed here so a
    //     class ending in either lands as Abstract.
    if model.ends_with(".tax")
        || model.ends_with("fiscal.position")
        || model.ends_with("reconcile.model")
        || model.ends_with("payment.term")
        || model.ends_with(".pricelist.item")
        || model.ends_with(".pricelist")
        || model.ends_with(".template")
        || model.ends_with(".settings")
        || model.ends_with(".config")
        || model.ends_with(".policy")
        || model.ends_with(".rule")
        || model.ends_with(".formula")
    {
        return DolceMarker::Abstract;
    }

    // (4) Quality — dimensions / classifications / rates.
    //     NOTE: `.groups` (plural) is the real Odoo class shape — the
    //     canonical `res.groups`. The hydrator-side `.group` rule had a
    //     misleading comment claiming to match `res.groups` but did not.
    //     Both singular and plural are listed.
    if model.starts_with("uom.")
        || model.ends_with(".tag")
        || model.ends_with(".type")
        || model.ends_with(".groups")
        || model.ends_with(".group")
        || model.ends_with(".category")
    {
        return DolceMarker::Quality;
    }

    // (5) Endurant — persistent master-data fallback.
    if model.starts_with("res.") || model.ends_with(".account") || model.starts_with("product.") {
        return DolceMarker::Endurant;
    }

    DolceMarker::Unknown
}

/// Strip a leading `odoo:` namespace prefix or the full odoo namespace IRI,
/// returning the bare Odoo model name. Idempotent — bare names pass through.
#[must_use]
pub fn strip_odoo_prefix(iri: &str) -> &str {
    if let Some(rest) = iri.strip_prefix("https://ada.world/onto/odoo#") {
        return rest;
    }
    iri.strip_prefix("odoo:").unwrap_or(iri)
}

// ═══════════════════════════════════════════════════════════════════════════
// ODOO_SEED — the muscle memory (15 declared alignment rows)
// ═══════════════════════════════════════════════════════════════════════════

/// One static alignment row — Odoo class → OWL pivot + inherited
/// (family, slot) + DOLCE + canonical label + provenance.
#[derive(Clone, Copy, Debug)]
pub struct OdooSeedRow {
    /// Odoo model name (the resolvable key).
    pub odoo_class: &'static str,
    /// `owl:equivalentClass` pivot URI.
    pub pivot_uri: &'static str,
    /// Inherited family byte.
    pub family: OdooFamily,
    /// Inherited slot within the family.
    pub slot: u16,
    /// DOLCE upper marker.
    pub dolce: DolceMarker,
    /// Canonical OGIT label this slot carries inside the foundry family table.
    pub label_uri: &'static str,
    /// `dcterms:source` lineage stamped into the FamilyEntry on hydration.
    pub provenance: &'static str,
}

/// The declared alignment rows — fixed table; linear scan is effectively O(1).
///
/// Each row asserts: "Odoo's `<odoo_class>` IS-A `<pivot_uri>`, and therefore
/// inherits OGIT family `<family>` slot `<slot>`." This is the muscle memory
/// Odoo deposits — the BillingCore / SMBAccounting / SmbFoundryCustomer /
/// SmbFoundryInvoice / ProductCatalog / HRFoundation basins are *not* invented;
/// they are existing foundry families the per-row OWL pivot routes into.
pub static ODOO_SEED: &[OdooSeedRow] = &[
    OdooSeedRow {
        odoo_class: "res.partner",
        pivot_uri: "fibo:LegalEntity",
        family: FAMILY_SMB_FOUNDRY_CUSTOMER,
        slot: 1,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.SMB:Customer",
        provenance: "odoo res.partner (company facet) =owl:equivalentClass=> fibo:LegalEntity",
    },
    OdooSeedRow {
        odoo_class: "account.move",
        pivot_uri: "fibo:Transaction",
        family: FAMILY_SMB_FOUNDRY_INVOICE,
        slot: 1,
        dolce: DolceMarker::Perdurant,
        label_uri: "ogit.SMB:Invoice",
        provenance: "odoo account.move =owl:equivalentClass=> fibo:Transaction",
    },
    OdooSeedRow {
        odoo_class: "account.move.line",
        pivot_uri: "fibo:JournalEntryLine",
        family: FAMILY_SMB_ACCOUNTING,
        slot: 1,
        dolce: DolceMarker::Perdurant,
        label_uri: "ogit.SMBAccounting:JournalEntryLine",
        provenance: "odoo account.move.line =owl:equivalentClass=> fibo:JournalEntryLine",
    },
    OdooSeedRow {
        odoo_class: "account.account",
        pivot_uri: "fibo:Account",
        family: FAMILY_SMB_ACCOUNTING,
        slot: 2,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.SMBAccounting:Account",
        provenance: "odoo account.account =owl:equivalentClass=> fibo:Account",
    },
    OdooSeedRow {
        odoo_class: "account.account.template",
        pivot_uri: "fibo:Account",
        family: FAMILY_SMB_ACCOUNTING,
        slot: 3,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.SMBAccounting:SkrAccount",
        provenance: "SKR03/04 chart concept (odoo account.account.template) => fibo:Account",
    },
    OdooSeedRow {
        odoo_class: "product.template",
        pivot_uri: "schema:Product",
        family: FAMILY_BILLING_CORE,
        slot: 1,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.Billing:Product",
        provenance: "odoo product.template =owl:equivalentClass=> schema:Product",
    },
    OdooSeedRow {
        odoo_class: "product.product",
        pivot_uri: "schema:Product",
        family: FAMILY_BILLING_CORE,
        slot: 2,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.Billing:ProductVariant",
        provenance: "odoo product.product (variant) =owl:equivalentClass=> schema:Product",
    },
    // ── ProductCatalog 0x64 — catalogue STRUCTURE (pricing + measurement) ──
    // product.template / product.product stay on BillingCore (0x61): they are
    // billable ITEMS. The pricelist / UoM concepts are catalogue STRUCTURE.
    OdooSeedRow {
        odoo_class: "product.pricelist",
        pivot_uri: "schema:PriceSpecification",
        family: FAMILY_PRODUCT_CATALOG,
        slot: 1,
        dolce: DolceMarker::Abstract,
        label_uri: "ogit.ProductCatalog:Pricelist",
        provenance: "odoo product.pricelist =owl:equivalentClass=> schema:PriceSpecification",
    },
    OdooSeedRow {
        odoo_class: "product.pricelist.item",
        pivot_uri: "schema:UnitPriceSpecification",
        family: FAMILY_PRODUCT_CATALOG,
        slot: 2,
        dolce: DolceMarker::Abstract,
        label_uri: "ogit.ProductCatalog:PricelistRule",
        provenance: "odoo product.pricelist.item =owl:equivalentClass=> schema:UnitPriceSpecification",
    },
    OdooSeedRow {
        // uom.uom ties into the QUDT Foundation namespace (qudt:Unit) — the
        // measurement spine.
        odoo_class: "uom.uom",
        pivot_uri: "qudt:Unit",
        family: FAMILY_PRODUCT_CATALOG,
        slot: 3,
        dolce: DolceMarker::Quality,
        label_uri: "ogit.ProductCatalog:UnitOfMeasure",
        provenance: "odoo uom.uom =owl:equivalentClass=> qudt:Unit",
    },
    // ── HRFoundation 0x90 — employee / org / job / base-contract ──
    // Payroll ENGINE is Odoo Enterprise (absent): only base HR data aligns here.
    OdooSeedRow {
        odoo_class: "hr.employee",
        pivot_uri: "vcard:Individual",
        family: FAMILY_HR_FOUNDATION,
        slot: 1,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.HR:Employee",
        provenance: "odoo hr.employee =owl:equivalentClass=> vcard:Individual",
    },
    OdooSeedRow {
        odoo_class: "hr.department",
        pivot_uri: "org:OrganizationalUnit",
        family: FAMILY_HR_FOUNDATION,
        slot: 2,
        dolce: DolceMarker::Endurant,
        label_uri: "ogit.HR:Department",
        provenance: "odoo hr.department =owl:equivalentClass=> org:OrganizationalUnit",
    },
    OdooSeedRow {
        odoo_class: "hr.job",
        pivot_uri: "org:Role",
        family: FAMILY_HR_FOUNDATION,
        slot: 3,
        dolce: DolceMarker::Abstract,
        label_uri: "ogit.HR:Job",
        provenance: "odoo hr.job =owl:equivalentClass=> org:Role",
    },
    OdooSeedRow {
        // Base employment contract only — payroll computation is Enterprise.
        odoo_class: "hr.contract",
        pivot_uri: "fibo:Contract",
        family: FAMILY_HR_FOUNDATION,
        slot: 4,
        dolce: DolceMarker::Abstract,
        label_uri: "ogit.HR:EmploymentContract",
        provenance: "odoo hr.contract (base, payroll is Enterprise/absent) =owl:equivalentClass=> fibo:Contract",
    },
];

// ═══════════════════════════════════════════════════════════════════════════
// Resolution surface — the symbol-table lookup
// ═══════════════════════════════════════════════════════════════════════════

/// Resolve an Odoo class to its OWL pivot. Exact seed match first; then a
/// `product.*` prefix fallback onto the generic `product.template` row
/// (schema:Product / BillingCore). Returns `None` for any class with no
/// existing family — that is the signal to author a Layer-2 alignment axiom,
/// not to invent a family.
#[must_use]
pub fn resolve_odoo(class: &str) -> Option<OwlPivot> {
    if let Some(row) = ODOO_SEED.iter().find(|r| r.odoo_class == class) {
        return Some(OwlPivot {
            pivot_uri: row.pivot_uri,
            family: row.family,
            slot: row.slot,
            dolce: row.dolce,
        });
    }
    // Unseen product subtype → inherit the generic product slot.
    if class.starts_with("product.") {
        let row = ODOO_SEED
            .iter()
            .find(|r| r.odoo_class == "product.template")?;
        return Some(OwlPivot {
            pivot_uri: row.pivot_uri,
            family: row.family,
            slot: row.slot,
            dolce: dolce_odoo(class),
        });
    }
    None
}
