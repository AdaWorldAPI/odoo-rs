//! One-shot generator for `data/account_nav.spo.ndjson` (committed corpus).
//! The `klickweg_parity` test re-derives and byte-compares against it.
use ruff_python_spo::{NavVocab, extract_odoo_nav_edges_with_report};
fn main() {
    let screens = ["account_move","account_move_line","account_account","account_analytic_line",
        "account_tax","res_partner","res_company","account_journal","account_payment",
        "account_bank_statement","account_fiscal_position","account_journal_group","account_incoterms"];
    let vocab = NavVocab { screens: screens.iter().map(|s| (*s).to_string()).collect() };
    let (edges, _r) = extract_odoo_nav_edges_with_report(std::path::Path::new("data"), &vocab);
    let triples: Vec<_> = edges.iter().map(|e| e.to_triple("odoo")).collect();
    print!("{}", ruff_spo_triplet::to_ndjson(&triples));
}
