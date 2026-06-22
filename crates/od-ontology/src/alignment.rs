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
///
/// - **Perdurant** — transactional events / processes: `*.move`,
///   `*.move.line`, `*.payment`, `*.order`, `*.order.line`, bank statements,
///   stock moves, pickings.
/// - **Abstract** — rules / classifications / models: `*.tax`,
///   `*.fiscal.position`, `*.reconcile.model`, `*.payment.term`,
///   `*.pricelist`, `*.pricelist.item` (pricing rules, not persistent goods).
/// - **Quality** — dimensions of comparison: `uom.*`.
/// - **Endurant** — persistent master-data: `res.*`, `*.account`, `product.*`
///   (the Abstract rules above match FIRST, so `product.pricelist` is
///   correctly Abstract rather than swept into Endurant by `product.*`).
/// - **Unknown** — no rule matched.
///
/// # Classifier completeness (finding from cross-validation)
///
/// The original `lance_graph_callcenter::odoo_alignment::dolce_odoo` lacked
/// the `pricelist` exception. The seed table declared `product.pricelist` as
/// `Abstract` (correctly: it's a pricing rule, not a persistent product), but
/// the classifier returned `Endurant` because `product.*` matched first.
/// Surfaced by the cross-validation test `every_seed_rows_dolce_matches_its
/// _classifier_assignment` (Phase-2 alignment-pin, this crate).
#[must_use]
#[allow(clippy::case_sensitive_file_extension_comparisons)] // Odoo class-name suffixes are not file extensions
pub fn dolce_odoo(class: &str) -> DolceMarker {
    if class.ends_with(".move")
        || class.ends_with(".move.line")
        || class.ends_with(".payment")
        || class.ends_with(".order")
        || class.ends_with(".order.line")
        || class.ends_with("bank.statement")
        || class.ends_with(".picking")
        || class == "stock.move"
    {
        return DolceMarker::Perdurant;
    }
    if class.ends_with(".tax")
        || class.ends_with("fiscal.position")
        || class.ends_with("reconcile.model")
        || class.ends_with("payment.term")
        || class.ends_with(".pricelist")
        || class.ends_with(".pricelist.item")
    {
        return DolceMarker::Abstract;
    }
    if class.starts_with("uom.") {
        return DolceMarker::Quality;
    }
    if class.starts_with("res.") || class.ends_with(".account") || class.starts_with("product.") {
        return DolceMarker::Endurant;
    }
    DolceMarker::Unknown
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
