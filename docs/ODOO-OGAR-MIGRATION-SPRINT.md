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
