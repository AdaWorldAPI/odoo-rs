//! **Real-corpus probe** — compile a REAL Odoo addon tree through the V3
//! substrate (`compile_source`), no toy fixtures (5+3 council R5 finding:
//! every prior `compile_source` call site was an inline snippet).
//!
//! Run: `cargo run -p od-ontology --example real_corpus_probe`
//! (env `ODOO_MODELS` overrides the addon dir; defaults to the account
//! addon of a sibling odoo checkout).
//!
//! Measured 2026-07-07 on /home/user/odoo addons/account/models (55 files,
//! incl. the 7380-line account_move.py):
//!   files=55 models=71 attrs=642 assocs=293 actions=1496 kausal=347
//!   classid_resolved=14
fn main() {
    let dir = std::env::var("ODOO_MODELS")
        .unwrap_or_else(|_| "/home/user/odoo/addons/account/models".into());
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("real_corpus_probe: {dir} not present — run next to an odoo checkout");
        return;
    };
    let (mut files, mut models, mut actions, mut kausal, mut classid_hits, mut attrs, mut assocs) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for e in entries {
        let p = e.expect("dir entry").path();
        if p.extension().is_none_or(|x| x != "py") {
            continue;
        }
        let src = std::fs::read_to_string(&p).expect("read source");
        files += 1;
        let ccs = od_ontology::compile_source(&src);
        models += ccs.len();
        for cc in &ccs {
            attrs += cc.class.attributes.len();
            assocs += cc.class.associations.len();
            actions += cc.actions.len();
            kausal += cc.actions.iter().filter(|a| a.kausal.is_some()).count();
            if cc.facet.facet_classid() != 0 {
                classid_hits += 1;
            }
        }
    }
    println!(
        "REAL-CORPUS: files={files} models={models} attrs={attrs} assocs={assocs} \
         actions={actions} kausal={kausal} classid_resolved={classid_hits}"
    );
}
