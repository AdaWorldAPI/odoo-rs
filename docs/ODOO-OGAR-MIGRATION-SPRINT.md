# Odoo → OGAR migration — classid-pull sprint plan

> **Goal (operator, 2026-06-21):** *"land where that open PR is
> [lance-graph #589], without bridge, only pulling OGAR via class"* — and
> *"planning (OpenProject) and ERP (Odoo) become reusable ontologies so the
> planner times align with billable hours."*
>
> **Method:** autoattended 5+3 — a parallel sprint of 5 worker + 3 meta
> (Sonnet) agents, then a 5+3 review council gates the PR. Never-stop loop;
> disk checked before every `cargo`; `target/` auto-cleared when tight.

---

## The migration arc (OGAR #95 W3 — Odoo)

The OGAR `APP-CODEBOOK-MIGRATION-PLAN` W3 spec for Odoo, with this repo's
status against each step:

| Step | What | Status |
|---|---|---|
| **W3.1** | Lower `od-ontology` onto `ogar_vocab::Class`; map model objects onto canonical commerce classids **via `OdooPort`** | ✅ **DONE** — `schema_to_classes` (structure, commit c2618eb) + `concept_classid`/`render_classid`/`schema_classids` (identity, this PR) |
| **W3.2** | Emit via `ogar-adapter-surrealql` | ✅ DONE — `emit_via_ogar` (parallel emit, c2618eb) |
| **W3.3** | DELETE the fork (`surreal_ast` + `triple` + native emit) | ⛔ **GATED** — blocked on the shared emitter covering the documented Stage-2 gaps (One2many/Many2many `array<record>`, computed `VALUE`, `DEFINE FUNCTION`/`EVENT`/`INDEX`). Until then the native path stays; deleting it now loses reactive wiring. |
| **W3.4** | `ir.model.access` ∧ `ir.rule` → `lance_graph_rbac::authorize` | ⛔ **GATED** — keystone (`CLASSID-RBAC-KEYSTONE-SPEC`) is CONJECTURE until `PROBE-OGAR-RBAC-AUTHORIZE` runs bit-for-bit vs Odoo. Not this repo's lane to mint the keystone. |
| **W3.5** | Private mint only if no canonical analogue | ✅ N/A — every Odoo model in scope has a canonical analogue (commerce arm `0x02XX` + cross-arm `0x0103`). No private mint needed. |

**This PR closes W3.1's identity half** — the DoD #3 *"the classid pull is a
pure static function call"* that Stage 1 left open (`OdooPort` was named only
in a doc-comment). Steps 3 + 4 are honestly deferred behind their gates.

---

## What shipped this PR (the classid pull)

```rust
// crates/od-ontology/src/ogar_bridge.rs  (feature = "ogar-emit")
pub const ODOO_APP_PREFIX: u16 = 0x0002;                       // Odoo render lens
pub fn concept_classid(model: &str) -> Option<u16>;            // shared concept (low u16)
pub fn render_classid(model: &str)  -> Option<u32>;            // 0x0002 || concept
pub fn schema_classids(schema: &Schema) -> Vec<(String, Option<u16>)>;
```

`APP ‖ class` codebook (OGAR `APP-CLASS-CODEBOOK-LAYOUT`): the **low u16** is
WHAT it is (shared RBAC + ontology, cross-app); the **high u16** is WHOSE
render (per-app `ClassView`/template). Odoo's lens is `0x0002`.

The convergence pin: `account_analytic_line → 0x0103` (`BILLABLE_WORK_ENTRY`)
— identical to WoA/SMB `Stundenzettel` and OpenProject/Redmine `TimeEntry`.
One codebook lookup at every planner→ERP→billing hop.

---

## 5+3 sprint — the next additive increment ("consume the pull")

The pull is exposed but not yet *consumed*. Five parallel Sonnet workers,
each scoped to **distinct files** (no shared-tree write race), edit-only;
the Opus orchestrator compiles/tests **once** centrally.

| # | Worker (Sonnet) | Scope (distinct files) | Deliverable |
|---|---|---|---|
| **S1** | classid-in-DDL | `ogar_bridge.rs` (emit path only) | Stamp `concept_classid` into emitted DDL as a `DEFINE TABLE … COMMENT 'classid:0xAABB'` so the canonical id rides the schema. Gap-safe: tables with `None` emit no classid comment. |
| **S2** | convergence-pin test | `tests/odoo_ogar_convergence.rs` (new) | Cross-repo pin mirroring OGAR's 5-way test on the odoo-rs side: `concept_classid("account_analytic_line") == 0x0103`; documents the planner↔ERP alignment as a regression gate. |
| **S3** | CLI surface | `src/bin/od_codegen.rs` (`--classids` subcmd) | `od-codegen --classids` prints the `table → 0xAABB` map for a corpus, so the pull is operator-inspectable. `required-features = ["cli", "ogar-emit"]`. |
| **S4** | docs | `docs/ODOO-OGAR-MIGRATION-SPRINT.md` (this file) + root `README.md` | Keep the migration arc + status table current; add a "classid pull" section to the README. |
| **S5** | rev-freshness | `crates/od-ontology/Cargo.toml` (comment only) | Confirm the pinned OGAR rev `08a9c979` carries the full `OdooPort` alias set (it does — verified pre-sprint); annotate the pin comment with the verification + the post-#96 main sha for a future bump. |

**+3 meta (Sonnet):**
- **M1 preflight-drift** — verify each S-scope against the actual tree before spawn (no stale assumptions).
- **M2 baton/boundary** — check the `OdooPort` classid contract is consumed identically to how OGAR exports it (no drift on the low-u16 concept ids).
- **M3 consolidation** — atomic-consolidation pass: one coherent commit set, no overlapping edits, board/docs updated in the same batch.

---

## 5+3 review council — the PR gate (never-stop)

Runs in parallel against the pushed branch; main thread (Opus) synthesizes
a LAND / REVISE / REJECT verdict and **auto-resolves** any P0 before merge.

| # | Reviewer | Lens |
|---|---|---|
| R1 | brutally-honest-tester (PP-13) | Rust pre-merge gate: clippy/fmt/test, codex P1 anti-patterns |
| R2 | baton-handoff-auditor (PP-15) | OGAR↔odoo-rs classid boundary: does the pull roundtrip the codebook contract? |
| R3 | core-first-architect | Core-first doctrine: adapter ASSUMES the Core (OGAR), carries no state |
| R4 | convergence-architect (PP-14) | Is the `_`→`.` normalize the right seam? any 0-friction alignment missed? |
| R5 | integration-lead | Cross-session: does this land cleanly with OGAR #94/#95, lance-graph #587/#589? |
| +3 | meta synthesis (Opus main thread) | Consolidate findings → verdict → auto-resolve loop |

**Gate rule:** any P0 → fix + re-push + re-gate (never-stop). On green →
subscribe to PR activity, continue autoattended.

---

## Guardrails (operator-locked)

- **Additive only.** Native `ToSql` emit untouched; W3.3 delete is gated.
- **No bridge.** The pull is a static `OdooPort::class_id` call — no registry,
  no hydration, no `UnifiedBridge` on the consumer side.
- **Disk before cargo.** `df -h` before every build; auto-clear stale
  `target/` (keep lance-graph's 2.9 G unless space-critical).
- **`ogar-emit`-gated.** Default build stays serde-only; the pull + tests
  compile only under `--features ogar-emit`.

---

## Sprint outcome (2026-06-22) — SHIPPED

The 5+3 sprint + 5+3 review ran to completion. Status against the plan:

| Worker | Planned | Outcome |
|---|---|---|
| S1 | classid-in-DDL | ✅ `emit_via_ogar_annotated` — classid in the `DEFINE TABLE … COMMENT 'classid:0x00020202'` clause (review-corrected from a `--` header, which SurrealDB drops at parse time). + `class_ids` re-export. |
| S2 | convergence-pin test | ✅ `tests/odoo_ogar_convergence.rs` — symbol-bound, full-surface, + negative pin (7 tests). |
| S3 | CLI surface | ✅ `od-codegen --classids` — verified end-to-end on `data/slice_2.spo.ndjson`. |
| S4 | docs | ✅ README "Pulling the canonical classid (OGAR)" section. |
| S5 | rev-freshness | ✅ Cargo.toml pin-freshness annotation (08a9c979 = #94 merge; main 5ee87b5 additive/test-only). |
| +Wave B | — | ✅ `examples/classid_pull.rs` — the full pull end to end. |

**5+3 review (two rounds, 10 specialist reviews):** unanimous LAND.
`brutally-honest-tester` (toolchain) · `baton-handoff-auditor` CLEAN ·
`core-first-architect` TARGETS-CORE · `convergence-architect` OPPORTUNITY ·
`integration-lead` LANDS CLEANLY.

**Auto-resolved from review:** R4's COMMENT-clause seam (the classid now
survives into SurrealDB's catalog, not just the `.surql` file).

**Filed, not forced:** `PROBE-OGAR-ID-TO-CONCEPT-NAME`
(`specs/UPSTREAM_WISHLIST.md`) — the concept-*name* enrichment + the
`canonical_concept`-on-`Class` fusion are gated on an OGAR-side `u16 → &str`
reverse lookup that doesn't exist yet.

**Tests:** 32 lib + 6 bin + 7 convergence + 23 integration/doc green under
`--features cli,ogar-emit`; clippy exit 0 (zero new warnings); fmt-clean.

Shipped as odoo-rs PR #3 on `claude/odoo-classid-consume` (builds on the
merged #2 classid pull).

---

## Phase 2 (2026-06-29) — full thinning: od-ontology → OGAR-substrate caller

> The operator's framing: *"~85 % would be in the OGAR transpile substrate,
> and the transcode then is just a generic compiler-store caller with some
> adapters; everything 'impossible' becomes a custom adapter + ClassView +
> ontological adaptability, at the cost of an import."*

**What changed upstream — #132 unblocks W3.3's biggest gap.** The OGAR
per-class transpile substrate landed (OGAR #132, on main):
`ogar-from-ruff::{lift_model_graph_python, mint, emit}` +
`docs/OGAR-TRANSPILE-SUBSTRATE.md`. Two things matter here:

1. **The relation gap is closed at the source.** Before #132 the Odoo lift
   dropped relational fields (the Codex P1 on #131); now ruff's `relation_kind`
   predicate (ruff #35) + `project_odoo_fields` give the shared
   `ogar_vocab::Class` its **associations** — `Many2one → BelongsTo`,
   `One2many → HasMany` (inverse), `Many2many → HasAndBelongsToMany`, each with
   the comodel as `class_name`. So the **W3.3 "One2many/Many2many
   `array<record>`" gap now has the data it was missing** — the shared emitter
   can render it; it no longer *needs* od-ontology's native fork to carry
   relations.
2. **The substrate owns both transpile legs.** Pull-in
   (`compile_graph_python::<OdooPort>` → `Vec<CompiledClass{class, facet}>`,
   `account.move → 0x0002_0202`) and pull-back (`emit_rust`, the codegen
   reference; `ogar-adapter-surrealql` is the DDL reference). od-ontology's
   bespoke `triple` (input) + `surreal_ast` + native emit (output) are now
   *redundant* with the substrate, not load-bearing.

### The staged thinning (W3.3 finished, additive then subtractive)

| Stage | Where | What | Verification |
|---|---|---|---|
| **A — relational arrays** | OGAR `ogar-adapter-surrealql` | Emit `HasMany`/`HasAndBelongsToMany` associations as `array<record<comodel>>` (the now-available #132 association data). Closes the largest W3.3 emitter gap. | CI-gated (surrealdb) |
| **B — substrate input** | odoo-rs `od-ontology` | Source → `ruff_python_spo` → `ModelGraph` → `compile_graph_python::<OdooPort>` becomes the canonical lower path, **superseding** `parse_ndjson → corpus_to_schema → schema_to_classes` (the SPO-corpus intermediate). | CI-gated |
| **C — delete the fork** | odoo-rs `od-ontology` | Remove `surreal_ast` + `triple` + native `ToSql` emit (W3.3). od-ontology collapses to a thin `compile_graph` caller + the shared `ogar-adapter-surrealql` emit. | CI-gated |

### What stays — the "impossible" 15 % (adapters, not deletions)

- **`od-posting`** — GoBD double-entry (gapless Belegnummer + inalterability
  hash chain). Intrusive/stateful → stays a hand-written Rust adapter.
- **Computed `VALUE` bodies + `DEFINE FUNCTION`/`EVENT`** (W3.3 Stage-2 gaps 2-3)
  — behaviour, not schema. These ride the OGAR **`ActionDef`/`KausalSpec`**
  behaviour arm (or a consumer adapter) — never inline emitter codegen. Gated
  with W3.4 (RBAC keystone). `DEFINE INDEX` (`_sql_constraints`) is mechanical
  and folds into Stage A.
- **Grounding (FIBO/DOLCE)** — `alignment::ODOO_SEED` + `odoo-to-fibo.ttl` stay
  as the *source* of grounding, but are **resolved late** via
  `classid → ClassView → OGIT` (the substrate's resolve-don't-store rule), not
  re-lowered per class.

### End state — od-ontology, thinned

```
od-ontology  (after Phase 2)
  = ruff_python_spo (parse Odoo source)
  + compile_graph::<OdooPort>            (the 85% — pulled from OGAR)
  + ogar-adapter-surrealql               (shared emit)
  + a thin wrapper contract (lance-graph-contract types)
  + od-posting                           (the 15% GoBD adapter)
```

> **Constraint (honest):** `ogar-adapter-surrealql` and `od-ontology` both pull
> the `surrealdb` git dep (403 in-sandbox), so every Phase-2 stage is
> **CI-verified, not probe-verifiable**. Each stage lands additively behind its
> green CI before the subtractive Stage C deletes the fork; the native path is
> never removed before the shared path provably covers it (the W3.3 guardrail).

Cross-ref: OGAR `docs/OGAR-TRANSPILE-SUBSTRATE.md` (the substrate), OGAR #132
(lift + mint + emit), ruff #34/#35 (`ruff_python_spo` + `relation_kind`).

---

## Phase 2 Stage B — SHIPPED (2026-06-30): od-ontology consumes the substrate

The substrate-input lower path landed in `od-ontology`, **additive** over the
native corpus path (Stage C delete stays gated):

```
Odoo source .py
  → ruff_python_spo::extract_from_source                     (parse)
  → ogar_from_ruff::mint::compile_graph_python::<OdooPort>   (lift + mint = the 85%)
  → Vec<CompiledClass { class, facet }>
  → ogar_adapter_surrealql::emit_surrealql_ddl(&classes)     (shared emit, Stage A)
```

**What shipped:**
- `src/ogar_bridge.rs`: `compile_source(src) -> Vec<CompiledClass>` +
  `emit_source_via_ogar(src) -> String` (classid-annotated `DEFINE TABLE …
  COMMENT` via the minted facet + `canonical_concept_name`); re-exported from
  `lib.rs` under `ogar-emit`.
- `Cargo.toml`: bumped the OGAR + ruff deps to `branch = "main"` (the prior
  pre-#132 rev pin lacked `ogar-from-ruff`); added `ogar-from-ruff` +
  `ruff_python_spo`.
- Tests: `emit_source_via_ogar_lowers_odoo_source_to_annotated_ddl` +
  `compile_source_skips_unparseable_and_resolves_classids`.

**Stage A flowed through — the `array<record>` gap is closed.** Bumping to OGAR
main pulled the Stage-A array emitter, so the *existing* `emit_via_ogar` path now
renders One2many/Many2many as real `array<record<…>>` columns. The
`ogar_parallel_emit.rs` gap-pin flipped exactly as it was authored to
(`dropped_fields_are_exactly_the_array_collections` →
`array_collections_now_converge_not_dropped`).

**Correction to the Phase-2 "Constraint (honest)" note above — the EMIT path IS
probe-verifiable offline.** `ogar-adapter-surrealql`'s `surrealdb` dep is gated
behind its `surrealdb-parser` *feature* (the parse-back leg only); the emit path
(`emit_surrealql_ddl`) is `ogar-vocab` + a hand-written formatter, zero
surrealdb. Stage B was verified **offline end-to-end**: od-ontology's full suite
green via a `[path]`-override probe, and a real `account.move` source lowered to
`DEFINE TABLE account_move … COMMENT 'commercial_document (classid:0x00020202)'`
+ `record<res_partner>` (Many2one) + `array<record<account_move_line>>`
(One2many). Only the `surrealdb-parser` round-trip is genuinely CI-only.

**Two review fixes (Codex on #20):**
- **P1 — source alignment.** `ruff_python_spo` (od-ontology) and OGAR's
  `ogar-from-ruff` both depend on `ruff_spo_triplet` (the `ModelGraph` type), so
  they must resolve to ONE cargo source or the types won't unify. The deps are
  pinned to a mutually-consistent snapshot — OGAR `rev = 7d0dca2` + the exact
  ruff `rev = 4860e79` that OGAR's `ogar-from-ruff` pins. (The `[path]`-override
  probe masked this by collapsing both ruff sources to one local copy.)
- **P2 — comodel normalization.** The substrate carries the raw dotted Odoo
  comodel (`res.partner`); `emit_source_via_ogar` normalizes association targets
  to table form (`res_partner`) so `record<…>` matches the `DEFINE TABLE` name.

**Still gated (unchanged):** Stage C (delete `surreal_ast` + `triple` + native
`ToSql`) — the native path still owns `DEFINE FUNCTION`/`EVENT`/`INDEX` +
computed `VALUE`/`READONLY` (the behaviour arm, W3.4); deleting now loses the
reactive wiring. W3.4's RBAC keystone is upstream CONJECTURE.

---

## Recipe-bitmask probe — AR-lifecycle-override redundancy (OGAR `D-RECIPE-BITMASK`)

> **Framing (operator, 2026-06-30):** OGAR is **Open Graph *Active Record***, so
> the canonical "recipe" IS the AR lifecycle protocol. A best-shaped (AR-canonical)
> consumer stores that recipe once and carries only a per-class override *bitmask*
> + the genuine deltas — the conjecture is that this thins the "impossible 15%"
> behavioural leftover toward ~7% for the best-shaped consumers. The ClassView +
> bitmask + ERB→askama view port is the rendering tier on top (the icing); the AR
> core is the substrate.

The consumer-side falsifier lives at
`crates/od-ontology/tests/recipe_redundancy_probe.rs` — a **default-build**
(offline, no `ogar-emit`, no git deps) measurement that mirrors
`ogar_actions::corpus_to_actions`'s classification (raises ⇒ guard; else
`MethodKind::Compute` ⇒ compute) and measures how much of the lifted behavioural
arm collapses to the two shared recipe shapes. An `ogar-emit`-gated block pins the
mirror to the real `corpus_action_rows` lift so the two can never drift.

**Measured (slice_2 corpus — `account_move` + `account_move_line` + `res_partner`
+ `res_company`):**

```
behavioral arm     : 358  (guards 47 + computes 311)
  computes resolved: 141  (reads captured) · unresolved 170 (reads NOT captured)
recipe shapes      : 2 carry all 358 behavioral methods
guard arm          : 47 guards → 1 shared recipe (46 hidden)   — FULL collapse
compute path-sets  : 101 distinct of 141 (40 dedup) · avg 1.6 paths
headline (188 resolved): recipe-collapsible 86 (45.7%) · genuine leftover 102 (54.3%)
```

**Verdict (honest, Odoo / Python = UPPER bound):**

- The **guard arm collapses fully** — every `@api.constrains` guard is the same
  AR recipe (`LifecycleTrigger{before_save}` + `Reject`); 47 → 1. This is the
  recipe-bitmask mechanism working perfectly on a real arm.
- The **compute arm is mostly genuine** — 101 distinct dependency path-sets of
  141 resolved computes; the bitmask hides only 40. Odoo's `_compute_*` methods
  each react to a different field set.
- **Leftover 54.3% ≫ 7% → the strong reading is REFUTED** ("Odoo collapses to
  7%") and the conjecture's **scoping is CONFIRMED**: 7% is the best-shaped
  *Rails-AR* case, not compute-heavy *Odoo-Python*.
- **Why this is an upper bound (two gaps surfaced):** (a) **inherited-vs-override
  is unmeasurable on this slice** — the corpus *does* carry `inherits_from` (8
  edges in slice_2), but every base mixin it names (`mail_thread`,
  `sequence_mixin`, `analytic_mixin`, …) is **out-of-slice**, so the bases' method
  sets aren't present to dedup an override against (and separately the live-source
  `ruff_python_spo` path — `compile_source` — drops `_inherit` in `build_graph`
  outright); inheritance is the biggest collapse lever and it's invisible here.
  And (b) **method bodies / decorator *types* are not captured** (only
  `@api.depends` args + `reads`/`raises` facts), so body-dedup (lossless-DO §1's
  stricter test) is invisible. Both can only LOWER the leftover, never raise it.
  Filed for upstream in `specs/UPSTREAM_WISHLIST.md` (emit `inherits_from` for
  Odoo `_inherit`; optional method-body hash).

The clean AR-recipe measurement belongs on the **Rails/OpenProject** side, where
`ruff_ruby_spo` captures `callbacks` / `validations` / `sti` as first-class
`Model` data (the recipe *is* the captured AR protocol). Handover with the
concrete Ruby probe spec:
`openproject-nexgen-rs/.claude/handovers/`. Canon home for the conjecture +
falsifier registration: OGAR `D-RECIPE-BITMASK` / `E-RECIPE-BITMASK` /
`PROBE-OGAR-AR-RECIPE-COLLAPSE`.
