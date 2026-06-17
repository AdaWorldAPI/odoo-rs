# `account.move._post` — HAND-PORT spec (the GoBD posting method)

> **Status:** `ROUTE-RESOLVED` / parity `CONJECTURE` (2026-06-17).
> The **route** (HAND-PORT) and the **gapless-numbering mechanism**
> (Option C, single-transaction pessimistic counter) are RESOLVED by an
> 8-agent council — see [`DECISIONS/D-POST-SEQ.md`](./DECISIONS/D-POST-SEQ.md)
> for the full record. The **parity** (does the hand-port reproduce
> Odoo's GoBD output byte-for-byte) is CONJECTURE until
> `PROBE-POST-GAPLESS-PARITY` runs green.
> **Authored by:** `adapter-shaper` probe (2026-06-17) → corrected
> through the D-POST-SEQ council (scenario-world / core-gap-auditor /
> core-first-architect / truth-architect / integration-lead +
> PP-13 / PP-15 / PP-16).
> This is the **first `route: hand_port` spec** — it proves the spec
> directory handles the intrusive case, not just the mechanical
> `adapter` / `guard_adapter` ones.

## Route — HAND-PORT (confirmed, not refuted)

`account.move._post` is the GoBD-critical posting method behind the
`action_post` button. Intrusive by construction: a gapless atomic
Belegnummer + a tamper-evident inalterability hash chain + transactional
fencing of the whole posting unit. The doctrine's Frankenstein-flattening
guard fires exactly as predicted — this method is NOT forced into the
`DEFINE FUNCTION` adapter mold.

### Mechanical / intrusive boundary (the doctrine allows peeling)

The doctrine: "the surrounding mechanical surface stays in the adapter."
Three fragments separate:

| Fragment | Route | Why |
|---|---|---|
| Pre-post **validations** (balance, partner, archived-journal, negative-total) | `guard_adapter` — each is a `_check_*`-shaped `DEFINE EVENT` with `THROW` | mechanical, no state crossing |
| `posted_before = true` freeze | **inside the hand-port** — the enforcement hook for Belegnummer immutability; must be set in the same tx as the hash or the freeze is forgeable | intrusive-adjacent |
| `state: draft→posted` flip | **inside the hand-port** — a standalone flip would let a row be `posted` without a hash (tamper window) | intrusive |
| Gapless number assign (`name`) + `inalterable_hash` chain | **HAND-PORT core** | the intrusive operations below |

**Everything from number-assign through hash-chain through the atomic
state+freeze write is ONE hand-port unit.** You cannot peel the `state`
flip out — its correctness is that it is welded to the hash.

## The three intrusive operations + the line each crosses

1. **Gapless atomic Belegnummer** (GoBD K3 / §14 UStG). Crosses:
   gap-free numbering is a *legal* invariant; requires serialized
   allocation under concurrency; the number must commit *with* the row.
2. **Inalterable hash chain** (GoBD K11). Crosses: each row's
   `inalterable_hash = sha256(prev.hash + canonical(row))`, chained per
   `(journal_id, sequence_prefix)`. Append-only, order-dependent, reads
   the predecessor row.
3. **Transactional fencing.** Crosses: ops 1+2 + state-flip + freeze
   must be ONE transaction or a partial post is a forged ledger.

## CORE-FIT — per primitive (source-verified against the surrealdb fork)

| Primitive | Verdict | Evidence |
|---|---|---|
| **Hash chain** | **TARGETS-CORE** | `crypto::sha256`/`blake3` exist (`core/src/fnc/crypto.rs`); a sync `DEFINE EVENT` on CREATE computes `inalterable_hash`, reading the predecessor by subquery. |
| **Append-only / tamper-evidence** | **TARGETS-CORE** | per-op `Permissions { select, create, update, delete }` (`catalog/schema/mod.rs:91`) → posted-row table at `update: NONE, delete: NONE` enforces GoBD `_can_be_unlinked → false`. |
| **Composite atomicity** | **TARGETS-CORE** | sync `DEFINE EVENT THEN` runs in the triggering tx (`doc/event.rs:34-35`); `store_record_data` (row+hash) at `create.rs:27` runs **before** `process_table_events` at `:31`, both in one `Document::process` → the hash event reads the predecessor through the same tx snapshot. |
| **Gapless Belegnummer** | **HAND-PORT, in-DB pessimistic counter** (the council decision — § below) | `DEFINE SEQUENCE` is batch-allocated (`kvs/sequences.rs:607` abandons the tail) and `nextval` runs its own tx (`:491-497`) → **UNIQUE, not GAPLESS**. But `LockType::Pessimistic` exists (`kvs/tr.rs:20-25`, threaded at `ds.rs:205-210`). |

## The gapless-numbering mechanism — council-resolved (D-POST-SEQ → Option C)

> **Corrected three times through the council. Read the trail; the
> obvious answer is wrong twice over.**

- ❌ **NOT "UNIQUE index + optimistic retry."** scenario-world's case-2
  proof: P1 claims N+1 (uncommitted); P2 retries to N+2, COMMITS; P1
  ABORTS → `{…, N, N+2}` = permanent **gap**. SurrealDB MVCC
  conflict-detects *same-key* writes only; two posters writing
  *different* numbers never conflict → UNIQUE ≠ GAPLESS.
- ❌ **NOT "an out-of-DB sequence owner."** (PP-13's feared collapse.)
- ✅ **A single-transaction, in-DB PESSIMISTIC counter.** A **pessimistic
  write transaction** (`LockType::Pessimistic`) does **read-modify-write
  on a per-`(journal_id, sequence_prefix)` counter key**; poster-2 blocks
  on the counter-key lock until poster-1 COMMITS (which is also when
  poster-1's hash becomes visible) → serial number ⇒ serial predecessor
  visibility ⇒ sound chain.

### The single-transaction invariant (PP-15 — the load-bearing correctness condition)

> **The counter read-modify-write, the `CREATE` row, the hash-computing
> `DEFINE EVENT`, and the `state`/`posted_before` write MUST share ONE
> `BEGIN…COMMIT`.**

If the number is assigned via *any* own-tx path (`nextval`, or any
helper that opens its own transaction), the pessimistic lock's guarantee
is dropped at the seam: the number commits in tx-A, the hash/state in
tx-B → an abort-after-number is a gap, AND a reader between commits sees
a number with no row. **This invariant is what makes Option A correct
and what Option B must preserve.**

### Option C (HYBRID) — the route

- **A (immediate):** a new **`od-posting`** workspace crate (a RUNTIME
  consumer with a SurrealDB client dep — kept OUT of zero-dep
  `od-ontology`) runs `BEGIN → pessimistic RMW counter key → CREATE row
  (hash event fires) → set state/posted_before → COMMIT` as ONE tx.
- **B (parallel substrate PR to AdaWorldAPI/surrealdb):** package the
  pessimistic-counter pattern as a `gapless` / `no-cache` option on
  `DEFINE SEQUENCE`. **Legitimate EXTEND-CORE** (core-gap-auditor 4-test
  PASS: a policy flag on an existing subsystem, peer of
  `BATCH`/`START`/`TIMEOUT`, not a new `fnc/*` — distinct from the
  rejected `fn::core::currency::*`). **HARD acceptance gate (PP-15):
  tx-enrolled / no own-tx** — a gapless `DEFINE SEQUENCE` that keeps
  `nextval`'s own-tx shape re-imports the exact bug it claims to fix. B
  does NOT route through the batch-allocated `nextval`; it is a separate
  pessimistic-counter path (PP-16 drift correction — B does not touch
  the "hot `nextval` path").
- A and B are **genuinely parallel** (zero shared code: A is the
  `od-posting` consumer, B is the surrealdb fork). If B lands, A's loop
  becomes a thin shim over the in-DB path; if B is rejected, A stands
  alone.

## Parity oracle — `PROBE-POST-GAPLESS-PARITY` (the CONJECTURE→FINDING gate)

- **Fixture:** N moves, one `(journal, prefix)`, **K concurrent posters
  + one mid-batch node restart** — the only interleaving where A/B differ
  from naive `nextval`.
- **Oracle:** Odoo `account.move._post` reference run.
- **SUT:** the SurrealDB `od-posting` path.
- **Diff-gate:** **set-equality on `name`** (gapless, contiguous) +
  **byte-equality on each `inalterable_hash`** (chained, with byte-exact
  `canonical(row)`: `float_repr` monetary + sorted-compact-ASCII JSON +
  `$4$` version prefix stripped before chaining). Message strings exempt
  (i18n), per the `_check_invoice_currency_rate` precedent.
- **Runnable today (the concurrency half ONLY):** "does a pessimistic
  write tx serialize same-key writers" is testable in SurrealDB's own
  `core/src/kvs/tests/multiwriter_same_keys_conflict.rs` harness — **no
  Odoo, no full build** (PP-16). Full parity (gapless names + byte-exact
  hash chain vs the Odoo reference) still gated on Odoo Python (not on
  this host) + the disk-gated surrealdb build.

## YAML spec

```yaml
method:
  qualname: account.move._post
  classid: account_move
  route: hand_port
  status: ROUTE-RESOLVED / parity CONJECTURE
  do_in:
    reads_field: [account_move.state, account_move.date, account_move.journal_id,
                  account_move.move_type, account_move.line_ids, account_move.name,
                  account_move.sequence_prefix, account_move.sequence_number]
    reads_relation: [previous_move via (journal_id, sequence_prefix, sequence_number DESC)]
  do_out:
    writes_field: [account_move.state, account_move.name,
                   account_move.inalterable_hash, account_move.posted_before]
  emit_shape: hand_port            # no single DEFINE * emits this
  body_sketch: null                # NO SurrealQL body — see intrusive_operations
  intrusive_operations:
    - op: gapless_belegnummer
      crosses: "legal gap-free numbering (§14 UStG); serialized allocation; commit-with-row"
      mechanism: "pessimistic write tx (LockType::Pessimistic) + RMW on per-(journal,prefix)
                  counter key; consumed only on COMMIT; counter-RMW shares ONE tx with
                  CREATE+hash-event+state-write (single-tx invariant)"
      anti_mechanism: "NEVER nextval / DEFINE SEQUENCE (batch-allocated, own-tx → UNIQUE-not-GAPLESS)"
    - op: inalterable_hash_chain
      crosses: "append-only, order-dependent, reads predecessor row"
      mechanism: "sync DEFINE EVENT on CREATE; inalterable_hash = crypto::sha256(prev.hash + canonical(row))"
    - op: transactional_fence
      crosses: "counter ⊕ create ⊕ hash ⊕ state ⊕ freeze = ONE BEGIN/COMMIT or forged ledger"
  preserves_invariant:
    - single_tx_counter_create_hash_state         # PP-15 — the load-bearing condition
    - chain_order_per_journal_sequence_prefix
    - append_only_no_update_delete_once_hashed     # Permissions update:NONE delete:NONE
    - serialization_byte_exact                     # float_repr + sorted-compact-ascii JSON + $4$ strip
  hand_port_target: "od-posting (new workspace crate; surrealdb client dep; calls the DB via
                     explicit BEGIN/COMMIT, NOT a DEFINE FUNCTION)"
  core_fit:
    hash_chain:  TARGETS-CORE                      # crypto::sha256 + sync event
    append_only: TARGETS-CORE                      # per-op Permissions
    atomicity:   TARGETS-CORE                      # event runs in triggering tx (create.rs:27→31)
    gapless:     HAND-PORT-PESSIMISTIC + EXTEND-CORE-B   # Option C
  composed_by:
    has_function: [account_move]
    virtually_overrides: [l10n_de.account_move._post]   # German delivery_date pre-fill
  parity_oracle:
    probe: PROBE-POST-GAPLESS-PARITY
    fixture_set: "N moves same (journal, prefix); K concurrent + one mid-batch node restart"
    diff_gate: "set-equality on name (gapless) + byte-equality on each inalterable_hash"
    runnable_now: "concurrency half via surrealdb kvs/tests/multiwriter_same_keys_conflict.rs;
                   full parity gated on Odoo Python (not on host) + disk-gated fork build"
  frankenstein_check:
    side_effect_triples: []        # _post's EDI/mail side effects are SEPARATE methods, not _post core
    raw_sql_triples:     []
    flattening_notes: "intrusive by construction — correctly routed HAND-PORT, not flattened into adapter"
```

## What ships when

This spec **does not** modify `emit.rs` — the projection's bare guard/
function emit is unchanged. `_post`'s hand-port body lives in the future
`od-posting` crate, NOT in the projection. Sequencing (integration-lead):
carve `od-posting` member → ship A (single-tx pessimistic counter) → file
B (tx-enrolled gapless `DEFINE SEQUENCE`) after core-gap-auditor's gate.
Nothing is built today; the spec records the resolved route + mechanism so
the eventual build doesn't reach for `nextval` (the wrong primitive).

See [`DECISIONS/D-POST-SEQ.md`](./DECISIONS/D-POST-SEQ.md) for the full
8-agent council record.
