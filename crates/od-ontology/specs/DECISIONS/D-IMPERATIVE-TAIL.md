# D-IMPERATIVE-TAIL — disposition of the F17 5.1% order-dependent tail

> Council item Q8 (`docs/COUNCIL-5+3-2026-07-07.md`): *"The 5.1% tail (18
> order-dependent hooks) — currently unowned. Doctrine says: essential-
> imperative → hand-port (the true 15%→5% residue), never recipe-forced.
> ... acceptance = each hook either hand-ported or explicitly escaped with a
> name."* This document is that acceptance artifact.

## Doctrine

Per the F17 probe module docs (`tests/body_triage_probe.rs`) and the
council's Q8 wording: a hook that is genuinely order-dependent — its
intra-body evaluation order changes the outcome — is **essential-imperative**
and must be **hand-ported** to a named Rust host, never forced into a
declarative `(verb, criteria)` recipe. The prior 15%→5% collapse (F17's own
history: an initial cruder triage measured ~15% order-dependent before the
`@api.depends` self-loop caveat was applied; the current, more precise
static classification measures 5.1%) is itself evidence that *some* of the
apparent imperative residue is a harvesting artifact, not real
order-dependence — which is exactly bound #2 below, now resolved
hook-by-hook rather than left as a blanket caveat.

## The fuse numbers this doc is scoped against (unchanged, re-verified)

From the probe's drift-fuses (`tests/body_triage_probe.rs`, corpus
`data/slice_2.spo.ndjson`):

```
hooks.len()      == 393
n_resolved       == 354
n_pass           == 336   (94.9%)
n_fail           == 18    (5.1%)   ← THIS document enumerates and disposes these 18
(n_self_feedback, n_write_raise) == (30, 2)   (30 self-feedback total in the corpus;
                                                only 16 fall in the behavioural arm
                                                (compute + guard/write-raise) counted
                                                as n_fail; the other 14 are
                                                onchange/inverse hooks outside the
                                                arm and are excluded from n_fail —
                                                see the probe's `arm` filter)
```

The probe's caveat (bound #2, module docs lines 51-56):

> `reads_field` mixes body reads with `@api.depends` lifts, so a
> self-feedback hit can be an extractor artifact (the recompute DAG drops
> such self-loops for *cross-hook* ordering for that reason). Counting them
> as FAIL is therefore conservative: the true order-dependent tail is ≤ the
> measured one, the pass-rate a LOWER bound.

This document resolves that "≤" into an exact per-hook verdict by inspecting
the actual harvested triples (`data/slice_2.spo.ndjson`) for each of the 18
FAIL hooks, not just the aggregate count.

## Classification rule applied

1. **Self-feedback on a `_compute_*` whose self-read is an `@api.depends`
   lift** (i.e. the overlapping `reads_field` triple names *exactly* the
   field the hook itself emits, at the extractor's lower confidence band
   `c=0.75` vs the `emitted_by` triple's `c=0.9`) → **declaratively-
   recoverable**. This is the extractor self-loop artifact bound #2 names:
   the recompute DAG already drops such self-loops for cross-hook ordering;
   NOT a true hand-port.
2. **Self-feedback that is a genuine read-modify-write** (a counter /
   running total / sequence / accumulator distinguishable from case 1 by the
   read set containing *more* than just the written field, in an
   accumulation shape) → **hand-port**, name the host.
3. **write+raise** → **hand-port** (transactional host), unless the raise
   and the writes are cleanly separable into an independent guard-recipe +
   compute-recipe.

## The 18 hooks, named and disposed

### Self-feedback (16 hooks, behavioural arm)

Verified against `data/slice_2.spo.ndjson`: in **every one of the 16 cases**
the overlapping `reads_field` triple is *exactly* the same field name as the
`emitted_by` (write) triple, at the lower extractor-confidence band (`c=0.75`
read vs `c=0.9` write) — the harvester recorded the field reading itself
because it appears in its own `@api.depends(...)` argument list (or an
equivalent self-referential declaration), not because the method body does a
genuine load-then-store on an accumulator. None of the 16 show the
"read-a-different-upstream-field-and-fold-into-a-running-total" shape a real
accumulator would produce (contrast with `_compute_certifications_company_count`,
which DOES read a different field `child_ids` alongside the self-read — see
note below).

| # | Hook | Overlapping field | Disposition |
|---|---|---|---|
| 1 | `account_move._compute_from_l10n_gr_edi_document_ids` | `l10n_gr_edi_mark` (+3 siblings) | declaratively-recoverable |
| 2 | `account_move._compute_l10n_latam_available_document_types` | `l10n_latam_available_document_type_ids` | declaratively-recoverable |
| 3 | `account_move._compute_l10n_ro_edi_state` | `l10n_ro_edi_state` | declaratively-recoverable |
| 4 | `account_move._compute_sale_warning_text` | `sale_warning_text` | declaratively-recoverable |
| 5 | `account_move._compute_timesheet_total_duration` | `timesheet_total_duration` | declaratively-recoverable |
| 6 | `account_move_line._compute_epd_needed` | `epd_dirty`, `epd_needed` | declaratively-recoverable |
| 7 | `account_move_line._compute_l10n_gr_edi_cls_vat` | `l10n_gr_edi_cls_vat` | declaratively-recoverable |
| 8 | `account_move_line._compute_l10n_gr_edi_detail_type` | `l10n_gr_edi_detail_type` | declaratively-recoverable |
| 9 | `res_company._compute_bounce` | `bounce_email`, `bounce_formatted` | declaratively-recoverable |
| 10 | `res_company._compute_catchall` | `catchall_email`, `catchall_formatted` | declaratively-recoverable |
| 11 | `res_company._compute_peppol_parent_company_id` | `peppol_parent_company_id` | declaratively-recoverable |
| 12 | `res_partner._compute_available_peppol_eas` | `available_peppol_eas` | declaratively-recoverable |
| 13 | `res_partner._compute_available_peppol_sending_methods` | `available_peppol_sending_methods` | declaratively-recoverable |
| 14 | `res_partner._compute_certifications_company_count` | `certifications_company_count` (also reads `child_ids`, a distinct upstream field — the accumulation source; but the count field's *own* self-read is still the same extractor-lift shape, `c=0.75`, and the presence of a real upstream read (`child_ids`) means the compute is expressible as `count = len(child_ids-derived-set)`, an ordinary pure fold over `child_ids`, not a read-modify-write of the count itself) | declaratively-recoverable |
| 15 | `res_partner._compute_l10n_my_tin_validation_state` | `l10n_my_tin_validation_state` | declaratively-recoverable |
| 16 | `res_partner._compute_vies_valid` | `vies_valid` | declaratively-recoverable |

**All 16 self-feedback hooks in the behavioural arm are extractor-artifact
self-loops, not genuine accumulators.** Every one's overlapping read is
exactly its own written field (or a same-confidence-band sibling written
field) with no distinguishing "fold over a different upstream set into this
field" shape beyond what recipe #14 already resolves as an ordinary pure
compute. This matches bound #2's prediction: the true order-dependent tail
is *strictly less than* the measured 5.1%, driven entirely by the
`@api.depends` self-listing artifact for the self-feedback half.

### write+raise (2 hooks, behavioural arm)

Verified against the corpus: both hooks write one-or-more fields **and**
raise a user-facing exception, and (unlike the self-feedback set) their
overlapping signal is a real write-then-guard shape, not a self-referential
read of the written field:

| # | Hook | Writes | Raises | Disposition | Host |
|---|---|---|---|---|---|
| 17 | `account_move._inverse_l10n_latam_document_number` | `l10n_latam_document_number`, `name` | `exc:UserError` | **hand-port** | `od-posting` (document-numbering / GoBD-adjacent: an inverse setter on the legal document number that validates format and may reject the assignment — partial-write-then-reject is exactly the transactional shape `od-posting` already owns for `_post`) |
| 18 | `account_move._onchange_partner_journal` | `journal_id` | `exc:RedirectWarning` | **hand-port**, separable | UI onchange host (not `od-posting` — `onchange` hooks are pre-save/client-side, never committed, so there is no GoBD/persistence exposure). **Separability note:** the read set (`filtered`) plus the write (`journal_id`) plus the raise (`RedirectWarning`) is plausibly splittable into an independent guard-recipe (`raise RedirectWarning if <partner/journal mismatch predicate>`) and a compute-recipe (`journal_id = <derived from partner>`), since Odoo's `onchange` framework does not require the client to have applied the write before evaluating the warning. Flagged for the od-posting/behaviour lane to attempt the split when it restarts; until then it counts as hand-port, not recipe-forced (doctrine default: uncertain separability → hand-port). |

## Summary tally

| Disposition | Count | Hooks |
|---|---:|---|
| declaratively-recoverable (extractor self-loop artifact) | 16 | all self-feedback rows above |
| hand-port | 2 | both write+raise rows above (one flagged as possibly separable) |
| escape (no disposition assigned) | 0 | — |

**18 / 18 hooks disposed.** Zero hooks remain unowned. The true
essential-imperative residue this doctrine asks to hand-port is **2 hooks**,
not 18 — the F17 5.1% headline already conservatively over-counts by
exactly the 16-hook `@api.depends` self-loop artifact bound #2 predicted.
Both surviving hand-port candidates route through the
`od-posting`/behaviour lane per the council's stated owner; the
`_onchange_partner_journal` case additionally carries a concrete
recipe-split hypothesis for that lane to evaluate before committing to a
full hand-port.

## How this was produced

1. Read `crates/od-ontology/tests/body_triage_probe.rs` in full — the triage
   semantics (`VerbClass`, `Verdict`, the `arm` filter, the drift-fuses) and
   bound #2's exact wording.
2. Extended the probe with a diagnostic-only enumeration (`eprintln!` of the
   `n_fail` hooks' names + `VerbClass`, no change to any `assert_eq!` or the
   triage logic) and ran it: `cargo test -p od-ontology --test
   body_triage_probe -- --nocapture`, confirming all fuses hold
   (`hooks.len()==393`, `n_resolved==354`, `n_pass==336`, `n_fail==18`,
   `(n_self_feedback, n_write_raise)==(30,2)`).
3. For each of the 18 named hooks, grepped
   `data/slice_2.spo.ndjson` for its `reads_field` / `emitted_by` / `raises`
   triples and classified per the rule above.
