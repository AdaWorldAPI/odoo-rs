# D-POST-SEQ — GoBD gapless Belegnummer routing for `account.move._post`

> **Status:** `RESOLVED` (2026-06-17) — **Option C (HYBRID)**, mechanism
> corrected three times through the 8-agent council. Verdict baked into
> [`../_post.md`](../_post.md). Resolution log at the bottom of this file.
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

| Option | Shape (as originally framed) | Trade |
|---|---|---|
| **A — HAND-PORT** | ~~A Rust posting service owns the gapless loop (UNIQUE index + optimistic-conflict-retry)~~ **← CORRECTED by the council: UNIQUE+retry delivers UNIQUE, not GAPLESS (scenario-world case-2 proof). Resolved mechanism = a single-transaction in-DB PESSIMISTIC counter; see Resolution.** | Faithful immediately. |
| **B — EXTEND-CORE** | Propose a `gapless` / `no-cache` option on surrealdb's `DEFINE SEQUENCE` (substrate-bump PR to AdaWorldAPI/surrealdb) so the Belegnummer stays in-DB and tx-enrolled. | One Core grow serves every gapless consumer; cleared the 4-test gate genuinely (see Resolution). |
| **C — HYBRID** | Option A now (immediate, faithful) + Option B filed as a parallel substrate improvement so future consumers get an in-DB path. | Two code paths until B lands, then A's loop collapses to a shim. **← CHOSEN.** |

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

## Resolution

**VERDICT: Option C (HYBRID).** Unanimous across 8 agents. The
*mechanism* inside Option A was corrected three times — the council
earned its keep by catching errors the obvious framing smuggled.

### The mechanism, corrected through the council

1. **scenario-world (Wave 1):** "UNIQUE index + optimistic retry"
   delivers **UNIQUE, not GAPLESS**. Case-2 proof: P1 claims N+1
   (uncommitted); P2 retries to N+2, COMMITS; P1 ABORTS → `{…, N, N+2}` =
   permanent gap. SurrealDB MVCC conflict-detects *same-key* writes only;
   different-number writers never conflict → no serialization. **Gapless
   ⇒ serialized allocation; uniqueness is necessary, not sufficient.**
2. **PP-13 (Wave 2):** "serialize per chain" names the *property* but not
   the *mechanism* — risked collapsing A into an out-of-DB sequence owner
   (the 4th option). HOLD until the serializer is named.
3. **PP-16 (Wave 2):** the serializer EXISTS in-DB — `LockType::Pessimistic`
   (`core/src/kvs/tr.rs:20-25`, threaded at `ds.rs:205-210`). A pessimistic
   write tx doing RMW on a per-`(journal,prefix)` counter key serializes
   correctly. **Collapse averted; A is implementable in-DB.** Also: B does
   NOT touch the hot `nextval` path (separate counter key); the probe's
   concurrency half is runnable today via `kvs/tests/multiwriter_same_keys_conflict.rs`.
4. **PP-15 (Wave 2):** the load-bearing invariant — counter-RMW + CREATE +
   hash-event + state-flip must share **ONE** `BEGIN/COMMIT`. Source-verified
   the composition (`create.rs:27` store-row+hash before `:31` events;
   `doc/event.rs:34-35` event-in-tx). Any own-tx number path (`nextval`)
   drops the lock at the seam → gap + dangling number. **B's hard
   acceptance gate: tx-enrolled / no own-tx, else it re-imports the bug.**

### Consolidated verdict (baked into `../_post.md`)

- **Route: C.** A = single-tx in-DB pessimistic counter in a new
  `od-posting` crate; B = tx-enrolled `gapless DEFINE SEQUENCE` substrate
  PR (legitimate EXTEND-CORE per core-gap-auditor's 4-test PASS — distinct
  from the rejected `fn::core::currency::*` because it's a policy flag on
  an existing subsystem).
- **Hash-chain + append-only + composite-atomicity: TARGETS-CORE**
  (host faithfully).
- **Parity: CONJECTURE**, gated on `PROBE-POST-GAPLESS-PARITY` (K-concurrent
  + node-restart). Concurrency half runnable now; full parity needs Odoo
  Python + the disk-gated fork build.
- **Sequencing (integration-lead):** carve `od-posting` member (does not
  exist yet) → ship A → file B after the 4-test gate. A/B genuinely
  parallel.

### Severity disposition

The CRITICAL finding ("never use `DEFINE SEQUENCE`/`nextval` for the
Belegnummer") is now documented loudly in `_post.md`'s `anti_mechanism`
slot. It does NOT sink the odoo-rs thesis — hash + append-only +
atomicity host faithfully; only gapless numbering needed the council, and
it resolved to a real in-DB primitive (pessimistic counter) + an optional
Core convenience (B).

## Resolution log

- 2026-06-17 — `COUNCIL-CONVENED`. Wave 1 spawned (5 consolidate).
- 2026-06-17 — Wave 1 returned: core-first-architect (C, all TARGETS-CORE),
  core-gap-auditor (B = EXTEND-CORE, 4-test PASS), truth-architect
  (CONJECTURE label + `PROBE-POST-GAPLESS-PARITY` gate), integration-lead
  (A/B genuinely parallel, carve `od-posting`), scenario-world
  (**UNIQUE≠GAPLESS — mechanism correction #1**).
- 2026-06-17 — Wave 2 returned: PP-13 (HOLD — name the serializer),
  PP-16 (**`LockType::Pessimistic` exists — collapse averted**, nextval
  mis-framing, probe partially runnable), PP-15 (**single-tx invariant —
  the load-bearing condition**; B's tx-enrolled gate).
- 2026-06-17 — `RESOLVED`. Verdict baked into `../_post.md`.
- 2026-06-17 — **Post-rebase verification.** Re-checked PP-16's source
  citations against fresh `surrealdb` at `68eaf63` (after rebase past PR
  #41): `LockType::Pessimistic` confirmed at `core/src/kvs/tr.rs:22-23`;
  plumbed through `ds.rs:206` (Pessimistic → bool true → builder lock);
  `multiwriter_same_keys_conflict.rs` exists as the structural template
  (45 lines, existing test uses `Optimistic` and demonstrates
  conflict-then-abort — exactly the failure mode the pessimistic
  mechanism avoids). The probe's concurrency half remains
  runnable-today; the source-level structural claim is **promoted from
  the council's word to independently re-verified post-rebase fact**.
- 2026-06-17 — **`od-posting` skeleton carved** (workspace commit
  `c08c1a7`). Trait surface (`PostingHost`, `MoveDraft`, `PostedMove`)
  + the four load-bearing invariants are now published in source; the
  actual `BEGIN → pessimistic RMW → CREATE → COMMIT` runner is
  unimplemented and gated on the disk-gated fork build. NO surrealdb
  client dep yet — keeps workspace `cargo check` cheap on the
  constrained host.
