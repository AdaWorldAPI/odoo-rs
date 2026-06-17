# D-POST-SEQ — GoBD gapless Belegnummer routing for `account.move._post`

> **Status:** `COUNCIL-CONVENED` (2026-06-17). 5-consolidate + 3-brutal-critique
> council running. Verdict will be baked into `_post.md` and this file
> updated to `RESOLVED` once synthesized.
> **Trigger:** the `adapter-shaper` HAND-PORT probe on `account.move._post`
> surfaced a source-verified CRITICAL CORE-FIT finding. Operator standing
> rule: "if critical make plan and ask 5 agents consolidate + 3 brutal
> agent critique."

## The established finding (the council validates, doesn't re-derive)

Source-verified against the AdaWorldAPI surrealdb fork
(`/home/user/surrealdb/surrealdb/core/src/`), with the same 4-test gate
that reversed the bogus `fn::core::currency::*` proposal in
`_compute_amount.md`:

- **`DEFINE SEQUENCE` is batch-allocated** (`kvs/sequences.rs` docstring:
  "batch allocation strategy ... minimizing coordination overhead"; the
  unused batch tail is abandoned via `tx.del(key)` at L607). Guarantees
  **uniqueness, NOT contiguity.** Options are only `batch`/`start`/`timeout`
  — no gapless/no-cache flag exists anywhere in core or language-tests.
- **`sequence::nextval` opens its OWN Write transaction** and commits
  independently (`kvs/sequences.rs:491-492`) — it does NOT enroll in the
  caller's transaction. A consumed-but-unused number after a post abort
  = a gap.
- **Hash-chain hosts faithfully:** `crypto::sha256`/`blake3` exist
  (`fnc/crypto.rs`); a synchronous `DEFINE EVENT` on CREATE computes
  `inalterable_hash = crypto::sha256(prev.hash + canonical(row))`,
  reading the predecessor by subquery. Append-only is a real Core
  primitive: per-op `Permissions { select, create, update, delete }`
  (`catalog/schema/mod.rs:91`) → a posted-row table at `update: NONE,
  delete: NONE` enforces the GoBD `_can_be_unlinked → false` boundary.
- **Atomicity of state+hash+freeze hosts faithfully:** sync `DEFINE
  EVENT THEN` runs in the triggering transaction (`doc/event.rs:34-35`).

**Net:** SurrealDB CAN host GoBD inalterability — except the gapless
Belegnummer, which `DEFINE SEQUENCE` is structurally the wrong shape for.
Using `nextval` to assign `account_move.name` = silent compliance
failure under concurrency / node restart.

## The decision

**How should odoo-rs route GoBD gapless Belegnummer numbering for
`account.move._post` against the SurrealDB target?**

| Option | Shape | Trade |
|---|---|---|
| **A — HAND-PORT** | A Rust posting service owns the gapless loop (UNIQUE index on `(journal_id, sequence_prefix, sequence_number)` + optimistic-conflict-retry) inside an explicit SurrealDB `BEGIN/COMMIT`; never calls `DEFINE SEQUENCE`. The `_post` adapter-shaper's recommendation. | Faithful immediately; but every Odoo localization re-implements a retry loop in the consumer forever. |
| **B — EXTEND-CORE** | Propose a `gapless` / `no-cache` option on surrealdb's `DEFINE SEQUENCE` (substrate-bump PR to AdaWorldAPI/surrealdb) so the Belegnummer stays in-DB and tx-enrolled. | One Core grow serves every gapless consumer; but a substrate PR that might be rejected for the same reasons the currency bolt-on was — must clear the 4-test gate genuinely. |
| **C — HYBRID** | Option A now (immediate, faithful) + Option B filed as a parallel substrate improvement so future consumers get an in-DB path. | Pragmatic; risks two code paths to maintain. |

## Council roster

### Wave 1 — 5 consolidate (diverse lenses on A/B/C)

1. **`core-first-architect`** — which option is TARGETS-CORE? Is hand-port
   (A) a doctrine-honoring "intrusive → hand-port", or a residue-Core
   cop-out? Is EXTEND-CORE (B) legitimate, or the currency-bolt-on
   mistake again?
2. **`core-gap-auditor`** — re-run the 4-test gate on Option B (gapless
   `DEFINE SEQUENCE` extension). The currency proposal "passed
   multi-reuse" but failed algebraic-generality; does a change to an
   *existing* sequence subsystem clear algebraic-generality where a new
   `fnc/*` didn't?
3. **`truth-architect`** — verify the source claims independently. Is
   "batch-allocated, not gapless" accurate? Is "nextval own-transaction"
   accurate? Don't take the probe's word.
4. **`integration-lead`** — cross-repo. B = a PR to AdaWorldAPI/surrealdb
   (contribution process? op-bridge precedent?). A = what does odoo-rs's
   crate structure need (a new `od-posting` crate? a feature gate?).
5. **`scenario-world`** — concurrency counterfactual. Under K concurrent
   `_post` + a node restart mid-batch, does A's conflict-retry actually
   produce gapless contiguous numbers? Does B's hypothetical no-cache
   sequence? Stress the failure modes.

### Wave 2 — 3 brutal critique (review the consolidated draft verdict)

- **`brutally-honest-tester`** (PP-13) — is the consolidated verdict
  sound, or rigorous-looking theater?
- **`baton-handoff-auditor`** (PP-15) — boundary integrity: A's
  posting-service ↔ SurrealDB transaction seam, OR B's odoo-rs ↔
  surrealdb-fork feature/DTO seam.
- **`preflight-drift-auditor`** (PP-16) — does the verdict drift from
  what's actually in the shipped surrealdb source / the corpus?

## What this decision does NOT block

- Writing `_post.md` (the HAND-PORT spec) — the route verdict (HAND-PORT)
  is settled; only the gapless-numbering *sub-mechanism* (A/B/C) is open.
- Any source code in od-ontology — nothing built today depends on this.
- The odoo-rs thesis — hash + append-only + atomic-state all host
  faithfully; this is one narrowly-scoped sub-mechanism.

## Resolution log

- 2026-06-17 — `COUNCIL-CONVENED`. Wave 1 spawned.
- _(verdict pending)_
