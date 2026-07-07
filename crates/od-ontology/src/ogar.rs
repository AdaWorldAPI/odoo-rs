//! The OGAR substrate consumption surface.
//!
//! **`SurrealQL` is deprecated** (operator ruling 2026-07-06: *"`SurrealQL` is
//! absolutely deprecated. OGAR V3 for transpile substrate, lance-graph V3 for
//! database."*). This module used to also carry a "Stage 1 parallel emit"
//! that lowered the bespoke `Schema` onto `ogar_vocab::Class` and re-emitted
//! `SurrealQL` DDL through `ogar-adapter-surrealql`, alongside the (now also
//! deleted) native hand-rolled `ToSql` AST. Both emit paths, and the bespoke
//! `Schema`/`TableDefinition`/`FieldDefinition` AST they lowered from, are
//! gone — deleted whole with `surreal_ast.rs` + `emit.rs`. There is no
//! `SurrealQL` target left to emit; classes sink into the lance-graph V3
//! database instead (16-byte facet key, canon-high classid).
//!
//! What survives is the substrate-input lowering (`compile_source`) and the
//! canonical classid pull — the parts of this module that never depended on
//! the bespoke DDL AST in the first place.
//!
//! # Substrate-input lowering
//!
//! [`compile_source`] lowers Odoo model *source* (`.py` text) straight
//! through the shared OGAR transpile substrate (OGAR #132): parse with
//! `ruff_python_spo`, lift + mint with `compile_graph_python::<OdooPort>`.
//! Each [`CompiledClass`] carries the lifted `ogar_vocab::Class` (structure:
//! attributes + associations) plus the minted `facet` (identity: the render
//! classid for codebook models) and the `actions` DO-arm
//! (`ogar_vocab::ActionDef`, OGAR #164 AT-CARRY-1).
//!
//! # Canonical classid pull (the "pull OGAR via class" deliverable)
//!
//! The functions below lower *identity*: they pull the canonical OGAR
//! `classid` for an Odoo model straight from the [`OdooPort`] alias table —
//! NO bridge object, NO registry, NO TTL hydration, just a pure static lookup
//! over the shared codebook (OGAR #94). This is the consumer migration target
//! (lance-graph #589 / OGAR `CONSUMER-MIGRATION-HOWTO`): a consumer names a
//! surface concept and gets back the shared id that WoA `Stundenzettel`, SMB
//! `Stundenzettel`, OpenProject/Redmine `TimeEntry`, and Odoo
//! `account.analytic.line` all converge on (`BILLABLE_WORK_ENTRY`, the
//! planner↔ERP billable-hours pin).

use ogar_from_ruff::mint::{compile_graph_python, CompiledClass};
use ogar_vocab::app::render_classid_for;
use ogar_vocab::ports::{OdooPort, PortSpec};

/// Compile Odoo model source (`.py` text) to the canonical OGAR
/// `Vec<CompiledClass>` via `ruff_python_spo` + `compile_graph_python::<OdooPort>`
/// — the substrate-input leg of the odoo-rs⟷OGAR convergence. Source that fails
/// to parse contributes nothing (`ruff_python_spo`'s silent-skip invariant).
///
/// Each [`CompiledClass`] carries the lifted `ogar_vocab::Class` (structure:
/// attributes + associations, with the comodel on each association) and the
/// minted `facet` (identity: the render classid for codebook models).
#[must_use]
pub fn compile_source(src: &str) -> Vec<CompiledClass> {
    compile_graph_python::<OdooPort>(&ruff_python_spo::extract_from_source(src))
}

// ── Canonical classid pull (the "pull OGAR via class" deliverable) ──────

/// Odoo's APP-prefix — the high `u16` of the 32-bit *render* classid per OGAR's
/// `APP-CLASS-CODEBOOK-LAYOUT` (`classid = concept(hi) ‖ APP(lo) — canon-high per D-CLASSID-CANON-HIGH-FLIP`). The low
/// `u16` is the shared cross-app concept (WHAT it is — RBAC + ontology); the
/// high `u16` is the per-app render lens (WHOSE template). `0x0002` is Odoo's
/// lens, so every Odoo-rendered id is `0x0002_<concept>`.
///
/// **Bound to the Core** (OGAR #97): this is [`OdooPort::APP_PREFIX`], not a
/// local literal — the prefix allocation lives once, in OGAR's `PortSpec`. If
/// OGAR ever re-allocates Odoo's prefix this follows automatically; there is
/// no second copy to drift.
pub const ODOO_APP_PREFIX: u16 = OdooPort::APP_PREFIX;

/// Pull the canonical OGAR **concept** classid (the shared low `u16`) for an
/// Odoo model name, straight through [`OdooPort`] — the static alias table, no
/// bridge object, no registry, no hydration.
///
/// The SPO corpus names models in Odoo's *table* form (`.` replaced by `_`:
/// `account_move`, `account_analytic_line`); [`OdooPort`]'s aliases are the
/// *model* form (`account.move`, `account.analytic.line`). Deriving the table
/// name as `_name.replace('.', '_')` is the canonical Odoo convention, so the
/// inverse `_`→`.` recovers the model name losslessly; a name already in model
/// form (no `_`) passes through unchanged. The raw name is also tried as a
/// fallback so a caller that already holds a dotted model name still resolves.
///
/// Returns `None` for a model outside the codebook (e.g. `ir_cron`).
///
/// `account_move` → `0x0202` (`COMMERCIAL_DOCUMENT`), `account_analytic_line` →
/// `0x0103` (`BILLABLE_WORK_ENTRY`, the cross-arm bridge), `res_partner` →
/// `0x0204` (`BILLING_PARTY`).
///
/// **Scope caveat — `_`→`.` is not a universal bijection.** The normalize is
/// lossless for all nine codebook aliases: each alias consists of dot-separated
/// single-word segments (no underscore inside a segment), so
/// `account_analytic_line` → `account.analytic.line` is an exact round-trip.
/// Odoo localization and multi-word module classes that carry an underscore
/// *inside* a segment (e.g. `l10n_es_edi_document`, `im_livechat_channel`) are
/// intentionally out of codebook scope and resolve to `None` — a fail-safe miss,
/// never a wrong id. Callers that need to map such names should maintain their
/// own alias table rather than extending the `_`→`.` heuristic.
#[must_use]
pub fn concept_classid(model: &str) -> Option<u16> {
    OdooPort::class_id(&model.replace('_', ".")).or_else(|| OdooPort::class_id(model))
}

/// The full 32-bit **render** classid for an Odoo model: Odoo's APP prefix
/// (`0x0002`) in the high `u16`, the shared canonical [`concept_classid`] in
/// the low `u16`. `account_move` → `0x0202_0002`; `account_analytic_line` →
/// `0x0103_0002`. `None` for an unaliased model.
///
/// Composition is OGAR's canonical [`render_classid_for`] (OGAR #97), not a
/// hand-rolled shift — the `(prefix << 16) | concept` layout lives in one
/// place (the Core), never re-implemented per consumer.
#[must_use]
pub fn render_classid(model: &str) -> Option<u32> {
    concept_classid(model).map(render_classid_for::<OdooPort>)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Canonical classid pull ──────────────────────────────────────────

    #[test]
    fn concept_classid_pulls_commercial_document_for_account_move() {
        // account_move (table form) → account.move (model form) →
        // COMMERCIAL_DOCUMENT 0x0202, straight from the OdooPort alias table.
        assert_eq!(concept_classid("account_move"), Some(0x0202));
    }

    #[test]
    fn concept_classid_pulls_billable_work_entry_for_analytic_line() {
        // The planner↔ERP convergence pin: account.analytic.line is the
        // cross-arm bridge into the project domain. Same id WoA/SMB
        // Stundenzettel + OpenProject/Redmine TimeEntry resolve to.
        assert_eq!(concept_classid("account_analytic_line"), Some(0x0103));
    }

    #[test]
    fn concept_classid_pulls_billing_party_for_res_partner() {
        assert_eq!(concept_classid("res_partner"), Some(0x0204));
    }

    #[test]
    fn concept_classid_resolves_multi_dot_model_names() {
        // Two underscores → two dots: account_move_line → account.move.line
        // → COMMERCIAL_LINE_ITEM 0x0201.
        assert_eq!(concept_classid("account_move_line"), Some(0x0201));
    }

    #[test]
    fn concept_classid_accepts_already_dotted_model_form() {
        // A caller already holding the dotted model name resolves via the
        // raw-name fallback (the normalize is a no-op without underscores).
        assert_eq!(concept_classid("account.move"), Some(0x0202));
    }

    #[test]
    fn concept_classid_is_none_outside_the_codebook() {
        assert_eq!(concept_classid("ir_cron"), None);
        assert_eq!(concept_classid(""), None);
    }

    #[test]
    fn render_classid_stamps_odoo_app_prefix() {
        // 0x0002 (Odoo render lens) ‖ low concept.
        assert_eq!(render_classid("account_move"), Some(0x0202_0002));
        assert_eq!(render_classid("account_analytic_line"), Some(0x0103_0002));
        assert_eq!(render_classid("res_partner"), Some(0x0204_0002));
        assert_eq!(render_classid("ir_cron"), None);
        assert_eq!(ODOO_APP_PREFIX, 0x0002);
    }

    // ── compile_source (substrate-input lowering) ───────────────────────

    #[test]
    fn compile_source_skips_unparseable_and_resolves_classids() {
        // Parse failure contributes nothing; a parseable codebook model mints
        // its render classid through the facet.
        assert!(compile_source("class Broken(:\n").is_empty());
        let compiled = compile_source(
            "from odoo import models, fields\n\n\nclass AM(models.Model):\n    _name = 'account.move'\n    name = fields.Char()\n",
        );
        assert_eq!(compiled.len(), 1);
        assert_eq!(compiled[0].facet.facet_classid(), 0x0202_0002);
    }

    /// AT-CONSUME (W3.3 delete gate, `docs/W3.3-DELETE-GATE-MATRIX.md`): the
    /// DO-arm that OGAR #164 (AT-CARRY-1) put on `CompiledClass` is actually
    /// consumed on this side — a live-source compile carries one `ActionDef`
    /// per method with its body facts, and the lifecycle classification agrees
    /// with the corpus-side mirror (`corpus_to_actions`'s prefix convention,
    /// `MethodKind::classify`). Before #164 `compile_source` dropped the whole
    /// behaviour arm; deleting the native fork would have lost the reactive
    /// wiring with no consumer ever noticing.
    #[test]
    fn compile_source_carries_the_do_arm() {
        let compiled = compile_source(concat!(
            "from odoo import api, models, fields\n\n\n",
            "class AM(models.Model):\n",
            "    _name = 'account.move'\n",
            "    amount_total = fields.Monetary(compute='_compute_amount')\n\n",
            "    @api.depends('line_ids.balance')\n",
            "    def _compute_amount(self):\n",
            "        for move in self:\n",
            "            move.amount_total = sum(move.line_ids.mapped('balance'))\n",
        ));
        assert_eq!(compiled.len(), 1);
        let cc = &compiled[0];

        // The DO-arm rides the compiled class (AT-CARRY-1 consumed).
        assert_eq!(cc.actions.len(), 1, "one ActionDef per harvested method");
        let act = &cc.actions[0];
        assert_eq!(act.predicate, "_compute_amount");
        // the frontend normalizes `_name = 'account.move'` to the table form
        assert_eq!(act.object_class, "account_move");
        assert!(
            act.identity.ends_with("::action_def::_compute_amount"),
            "identity carries the action-def address, got {}",
            act.identity
        );

        // Classification parity with the corpus-side mirror: the carried
        // predicate classifies as Compute under the same prefix convention
        // `corpus_to_actions` uses — the two arms can never silently drift.
        assert_eq!(
            crate::MethodKind::classify(&act.predicate),
            crate::MethodKind::Compute,
            "carried DO-arm predicate must classify as the corpus arm would"
        );

        // The THINK arm still carries the reactive schema half alongside.
        assert!(
            !cc.class.computed_fields.is_empty(),
            "computed_fields (THINK arm) and actions (DO arm) travel together"
        );
    }

    /// The foreign-consumer SDK (OGAR #177, `E-AR-DIRECT-SDK`): a
    /// `CompiledClass` materializes to a native-language class in Python / C# /
    /// Rust — no bridge, no serialization, the classid + typed fields ride
    /// straight into the target language. This pins the **Odoo → SDK** path:
    /// an Odoo model source lowered through the substrate emits a usable SDK
    /// class in each language, so a Python or C# consumer of Odoo models needs
    /// only the emitted dataclass, never SurrealQL (deprecated) or a bridge.
    #[test]
    fn odoo_source_materializes_to_the_foreign_consumer_sdk() {
        use ogar_from_ruff::emit::{emit_csharp, emit_python, emit_rust};

        let compiled = compile_source(concat!(
            "from odoo import models, fields\n\n\n",
            "class AM(models.Model):\n",
            "    _name = 'account.move'\n",
            "    name = fields.Char()\n",
            "    partner_id = fields.Many2one('res.partner')\n",
        ));
        assert_eq!(compiled.len(), 1);
        let cc = &compiled[0];

        // Python SDK: an `@dataclass` carrying the canon-high classid and the
        // typed fields (the association renders as a `ToOne[...]` relation).
        let py = emit_python(cc);
        assert!(py.contains("@dataclass"), "python SDK is a dataclass:\n{py}");
        assert!(
            py.contains("CLASSID: ClassVar[int] = 0x02020002"),
            "python SDK carries the canon-high classid:\n{py}"
        );
        assert!(py.contains("name:"), "python SDK carries the scalar field:\n{py}");
        assert!(
            py.contains("partner_id:"),
            "python SDK carries the association field:\n{py}"
        );

        // C# SDK: the same class materialized for the .NET consumer.
        let cs = emit_csharp(cc);
        assert!(
            cs.contains("0x02020002"),
            "c# SDK carries the canon-high classid:\n{cs}"
        );

        // Rust SDK: the sibling emitter — the materialized struct the Rust
        // consumer would use (distinct from the lance-graph V3 row sink; this
        // is the typed API surface, that is the storage row).
        let rs = emit_rust(cc);
        assert!(
            rs.contains("struct") && rs.contains("0x02020002"),
            "rust SDK materializes a struct with the classid:\n{rs}"
        );
    }

    /// KAUSAL-PARITY PIN: the two DO-arm pipelines — the corpus witness
    /// [`corpus_to_actions`](crate::corpus_to_actions) (deprecated) and the
    /// OGAR live arm ([`compile_source`] -> `lift_actions`) — build
    /// [`KausalSpec`] from the SAME Odoo model DIFFERENTLY for a guard
    /// method. This test PINS both divergences with OGAR as canonical, so
    /// neither pipeline can silently re-converge (or diverge further)
    /// without failing here. Companion to `compile_source_carries_the_do_arm`
    /// (which pins `MethodKind` classification parity); this one pins the
    /// `KausalSpec` *shape* parity/divergence.
    #[test]
    #[allow(deprecated)] // corpus_to_actions is the intentional deprecated witness
    fn kausal_parity_pinned_ogar_vs_corpus_witness() {
        use ogar_vocab::{GuardFailurePolicy, KausalSpec};

        // One Python model, deliberately mirrored into both arms: a
        // `_compute_amount` compute (`@api.depends('line_ids.balance')`) and
        // a `_check_balanced` guard (`@api.constrains('line_ids')`, raises).
        const SRC: &str = concat!(
            "from odoo import api, models, fields\n",
            "from odoo.exceptions import ValidationError\n\n\n",
            "class AM(models.Model):\n",
            "    _name = 'account.move'\n",
            "    amount_total = fields.Monetary(compute='_compute_amount')\n\n",
            "    @api.depends('line_ids.balance')\n",
            "    def _compute_amount(self):\n",
            "        for move in self:\n",
            "            move.amount_total = sum(move.line_ids.mapped('balance'))\n\n",
            "    @api.constrains('line_ids')\n",
            "    def _check_balanced(self):\n",
            "        for move in self:\n",
            "            if move.amount_total != 0:\n",
            "                raise ValidationError('not balanced')\n",
        );

        // The corpus witness's parallel SPO facts for the SAME two methods
        // (same method names, same field). The `reads_field` object is
        // deliberately written as the bare dotted path `line_ids.balance`
        // (matching OGAR's raw `@api.depends` arg verbatim) rather than this
        // crate's usual `<model>.<field>` qualified convention (see this
        // file's own corpus fixtures elsewhere, e.g. `account_move.line_ids`)
        // — ONLY so this controlled fixture can assert textual path equality
        // below. GENERALLY the corpus's `reads_field` harvest and OGAR's
        // `Field::depends_on` are lifted by two independent extractors and
        // are NOT guaranteed to agree byte-for-byte — the corpus can carry a
        // superset (deep cross-model leaf reads, see `recompute_dag.rs`'s
        // slice-2 enrichment) or a differently-qualified string. OGAR remains
        // canonical whenever the two disagree.
        const NDJSON: &str = concat!(
            r#"{"s":"odoo:account_move","p":"has_function","o":"odoo:account_move._compute_amount","f":0.9,"c":0.9}"#, "\n",
            r#"{"s":"odoo:account_move._compute_amount","p":"reads_field","o":"odoo:line_ids.balance","f":0.9,"c":0.9}"#, "\n",
            r#"{"s":"odoo:account_move","p":"has_function","o":"odoo:account_move._check_balanced","f":0.9,"c":0.9}"#, "\n",
            r#"{"s":"odoo:account_move._check_balanced","p":"raises","o":"exc:ValidationError","f":0.9,"c":0.9}"#, "\n",
        );

        // ── OGAR live arm ────────────────────────────────────────────────
        let compiled = compile_source(SRC);
        assert_eq!(compiled.len(), 1, "one class compiled from SRC");
        let cc = &compiled[0];

        let ogar_compute = cc
            .actions
            .iter()
            .find(|a| a.predicate == "_compute_amount")
            .expect("OGAR arm carries _compute_amount");
        let ogar_guard = cc
            .actions
            .iter()
            .find(|a| a.predicate == "_check_balanced")
            .expect("OGAR arm carries _check_balanced");

        // ── corpus witness arm ──────────────────────────────────────────
        let triples = crate::parse_ndjson(NDJSON).expect("fixture ndjson parses");
        let corpus_actions = crate::corpus_to_actions(&triples);

        let corpus_compute = corpus_actions
            .iter()
            .find(|a| a.predicate == "_compute_amount")
            .expect("corpus witness carries _compute_amount");
        let corpus_guard = corpus_actions
            .iter()
            .find(|a| a.predicate == "_check_balanced")
            .expect("corpus witness carries _check_balanced");

        // ── 1. Compute method: both arms agree on the VARIANT (Depends);
        //    OGAR is authoritative on the PATHS. ──────────────────────────
        let ogar_paths = match &ogar_compute.kausal {
            Some(KausalSpec::Depends { paths }) => paths.clone(),
            other => panic!("OGAR compute kausal must be Depends, got {other:?}"),
        };
        let corpus_paths = match &corpus_compute.kausal {
            Some(KausalSpec::Depends { paths }) => paths.clone(),
            other => panic!("corpus compute kausal must be Depends, got {other:?}"),
        };
        assert_eq!(
            ogar_paths,
            vec!["line_ids.balance".to_string()],
            "OGAR's Depends paths come straight from SRC's @api.depends(...) \
             set (Field::depends_on) — that is the authoritative source \
             (NATIVE-BEHAVIOUR-SEMANTICS finding-6)"
        );
        // On THIS controlled fixture the two arms agree textually because the
        // ndjson was hand-aligned to OGAR's raw depends_on convention. This is
        // NOT a general guarantee: the corpus's `reads_field` harvest can be a
        // superset (deep cross-model leaf reads) or use a different
        // qualification convention (`<model>.<field>` vs OGAR's raw decorator-
        // arg string) than OGAR's `depends_on`. OGAR is canonical whenever
        // they diverge — this equality is a fixture artifact, not a promise.
        assert_eq!(
            corpus_paths, ogar_paths,
            "aligned only because this fixture's reads_field was hand-matched \
             to OGAR's depends_on; not a general corpus<->OGAR guarantee"
        );

        // ── 2. Guard method: the KNOWN variant divergence, pinned. ────────
        // DIVERGENCE (pinned, OGAR canonical): the corpus witness models a
        // constrains-guard as an event LifecycleTrigger+Reject; OGAR models
        // it as a Constrains validation trigger carrying the constrained
        // paths — the OGAR shape is canonical (SPEC-ATC2-OGAR Arm B keeps
        // validation distinct from a persisted recompute trigger).
        assert_eq!(
            corpus_guard.kausal,
            Some(KausalSpec::LifecycleTrigger {
                event: "before_save".to_string()
            }),
            "corpus witness models @api.constrains as a before_save lifecycle event"
        );
        assert_eq!(
            corpus_guard.guard_failure_policy,
            Some(GuardFailurePolicy::Reject),
            "corpus witness attaches a Reject guard-failure policy to the guard"
        );
        assert_eq!(
            ogar_guard.kausal,
            Some(KausalSpec::Constrains {
                paths: vec!["line_ids".to_string()]
            }),
            "OGAR (canonical) models @api.constrains as a Constrains validation \
             trigger carrying the constrained field paths, NOT a lifecycle event"
        );
        assert_eq!(
            ogar_guard.guard_failure_policy, None,
            "OGAR's ActionDef has no guard_failure_policy populated by \
             lift_actions Arm B — guard/RBAC enrichment is a downstream \
             registrar concern, not the producer's (see lift_actions docs)"
        );

        // ── 3. Drift tripwire: the behaviour-method COUNT agrees between
        //    arms for this fixture (guard + compute = 2 each). ────────────
        assert_eq!(
            (cc.actions.len(), corpus_actions.len()),
            (2, 2),
            "a future drift in either pipeline's harvested method-set must \
             show up here before it shows up as a silent behaviour gap"
        );
    }
}
