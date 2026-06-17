# `account.move._compute_amount` — adapter spec

> **Status:** `DRAFT-CONJECTURE` (2026-06-17). Shape derived from the SPO
> corpus + Core-First Transcode Doctrine. Python body **not yet captured**
> (`/home/user/odoo` not extracted on this host); `.claude/odoo/L1-K3-POST.md`
> referenced but not on disk. **Parity oracle is the GATE** — the spec does
> not ship into `emit::corpus_to_schema` until the Python source is read
> and a fixture capture exists. Per doctrine: CONJECTURE until the
> falsifier runs.
> **Authored by:** `adapter-shaper` agent (subagent run 2026-06-17),
> consolidated from SPO corpus
> `lance-graph::crates/lance-graph/src/graph/spo/odoo_ontology.spo.ndjson`.

## Route — ADAPTER (with one blocker CORE GAP)

Mechanical sum over a One2many graph traversal; clean adapter route per
doctrine § "Holds for mechanical/data-shaped leaf methods only". The
currency-conversion sub-function is a CORE GAP, **NOT an adapter hack** —
it lands as an EXTEND-CORE proposal that ~80 future Money-compute
methods would share.

### Frankenstein-flattening guard

The SPO corpus on disk shows for `_compute_amount`:

- **0** `side_effect` triples
- **0** `raw_sql` triples
- **0** intrusive-hash-chain triples (`inalterable_hash`,
  `secure_sequence` — those attach to `_compute_hash` /
  `_check_hash_integrity`, NOT here)
- **9** `emitted_by` outputs (mechanical writes — clean adapter signal)
- **1** `reads_field` triple (`line_ids` — the One2many entry point)

If a future harvest pass adds a `side_effect` triple, that specific
Python branch hand-ports behind a feature gate; the surrounding sum
stays in the adapter.

## DO-in / DO-out

### DO-in (tenants / edges read)

From SPO `reads_field` + `emitted_by` inversion:

- **`account_move.line_ids`** — One2many. **Already materialised** as
  `VALUE <-account_move_line.move_id READONLY` per commit `695c1c8`. The
  adapter reads via the Core's graph-traversal arrow; it does NOT
  iterate Python-side.
- Per-line tenants on `account_move_line`:
  - `balance` (signed amount in line currency)
  - `amount_currency` (amount in line's own currency)
  - `tax_line_id` (Many2one — discriminator: tax line vs base line)
  - `currency_id` (line currency, Many2one)
- `account_move.company_id.currency_id` — target currency (Many2one chain).
- `account_move.date` — rate-lookup key (used in currency convert).
- `account_move.move_type` — sign multiplier discriminator
  (`in_invoice` / `in_refund` → negative; `out_*` → positive).

### DO-out (9 tenants written)

From SPO `emitted_by` triples inversed:

- `amount_untaxed`, `amount_tax`, `amount_total`, `amount_residual`
- Signed companions: `amount_untaxed_signed`, `amount_tax_signed`,
  `amount_total_signed`, `amount_residual_signed`
- In-currency-signed: `amount_total_in_currency_signed`,
  `amount_untaxed_in_currency_signed`

## Multi-field emit — **object-literal return** (decision)

SurrealQL `DEFINE FUNCTION` returns a single value; Odoo's
`_compute_amount` writes 9 fields. Three options were considered:

- (a) One function per output field with shared `LET` subqueries —
  duplicates the tax/base partition 9 times.
- (b) **One function returning an object literal** — `LET` the shared
  partition once; each `FieldDefinition.value` becomes
  `fn::account_move::_compute_amount($this).<field>`. The Core's
  reactive `VALUE` recomputes the whole object, indexes the field.
- (c) Materialised view — out of scope.

**Choice: (b).** Matches the SPO corpus (one method → 9 `emitted_by`)
without inventing nine ghost methods.

## SurrealQL body sketch

```surql
DEFINE FUNCTION fn::account_move::_compute_amount($this: record<account_move>) {
    LET $lines       = $this.line_ids;
    LET $company_ccy = $this.company_id.currency_id;
    LET $date        = $this.date;
    LET $sign        = IF $this.move_type IN ["in_invoice", "in_refund"] THEN -1 ELSE 1 END;

    LET $tax_lines  = $lines[WHERE tax_line_id != NONE];
    LET $base_lines = $lines[WHERE tax_line_id  = NONE];

    -- CORE GAP markers (see § "Core gaps"): fn::core::currency::convert
    -- is the EXTEND-CORE primitive proposed below.
    LET $untaxed = math::sum($base_lines.map(|$l|
        fn::core::currency::convert($l.balance, $l.currency_id, $company_ccy, $date)
    ));
    LET $tax = math::sum($tax_lines.map(|$l|
        fn::core::currency::convert($l.balance, $l.currency_id, $company_ccy, $date)
    ));
    LET $total = $untaxed + $tax;
    LET $residual = $total - math::sum(
        $lines[WHERE reconciled = true].map(|$l|
            fn::core::currency::convert($l.amount_residual, $l.currency_id, $company_ccy, $date)
        )
    );

    RETURN {
        amount_untaxed:     $untaxed,
        amount_tax:         $tax,
        amount_total:       $total,
        amount_residual:    $residual,
        amount_untaxed_signed:               $untaxed  * $sign,
        amount_tax_signed:                   $tax      * $sign,
        amount_total_signed:                 $total    * $sign,
        amount_residual_signed:              $residual * $sign,
        amount_total_in_currency_signed:     $total    * $sign,
        amount_untaxed_in_currency_signed:   $untaxed  * $sign,
    };
};
```

## Core gaps — EXTEND-CORE proposals

> Per doctrine: adapter that needs state the Core can't carry → **Core
> gap, EXTEND-CORE deliberately, never ADAPTER-HACK**. The proposed
> primitives below are universally reusable; one Core grow-bump replaces
> ~80 adapter duplications across Odoo's Money-compute surface.

### GAP-1 (BLOCKER) — currency conversion is not a Core primitive

Odoo carries it via `ResCurrency._convert(amount, to_currency, company,
date)` — a stateful lookup against `res.currency.rate` rows. SurrealDB
has no first-class "rate-as-of-date" indexed lookup.

**Adapter-state-leak risk if not extended:** every Money-compute adapter
(~80 in Odoo's `account` + adjacent modules) duplicates the rate table
inline.

**Minimum EXTEND-CORE primitive:**

1. **Table:**
   ```surql
   DEFINE TABLE res_currency_rate SCHEMAFULL TYPE NORMAL;
   DEFINE FIELD currency_id ON res_currency_rate TYPE record<res_currency>;
   DEFINE FIELD company_id  ON res_currency_rate TYPE record<res_company>;
   DEFINE FIELD name        ON res_currency_rate TYPE datetime;  -- effective date
   DEFINE FIELD rate        ON res_currency_rate TYPE decimal;
   DEFINE INDEX rate_lookup ON res_currency_rate FIELDS currency_id, company_id, name UNIQUE;
   ```

2. **Function:**
   ```surql
   DEFINE FUNCTION fn::core::currency::convert(
       $amount: decimal,
       $from:   record<res_currency>,
       $to:     record<res_currency>,
       $date:   datetime,
   ) -> decimal {
       /* rate-as-of-date join; returns $amount * (to_rate / from_rate) */
   };
   ```

Lives in a `fn::core::currency::*` namespace alongside future
`fn::core::currency::format`, `fn::core::currency::round_to_precision`.

**Reused by:** ~80 Money-compute methods across `account`, `sale`,
`purchase`, `stock`, `mrp`, `hr_payroll`. One Core grow-bump.

### GAP-2 (non-blocker) — per-currency rounding precision

`res_currency.rounding` (decimal) must be a stored scalar; precise
financial rounding requires `fn::core::currency::round_to($amount,
$currency)` rather than generic banker's rounding.

**EXTEND-CORE primitive:** one more standard fn in the
`fn::core::currency::*` namespace; reads `res_currency.rounding` as a
scalar field. No new table.

## ClassView composition

- `account_move → has_function → _compute_amount` (corpus,
  Structural triple)
- **No** `virtually_overrides` triples present on the corpus —
  no MRO flattening needed for this method.

## Parity oracle (the gate)

- **Odoo method:** `account.move._compute_amount`
- **Fixture set:** multi-line, multi-currency invoice with both tax and
  base lines, reconciled and unreconciled. Captured from an Odoo
  instance; round-tripped through the projection's output schema.
- **Diff-gate:** 9-field tuple, decimal-exact, per-currency rounding
  precision honored.
- **Status:** **NOT CAPTURED** — Python source not on this host.
  Spec ships into `emit::corpus_to_schema` after capture.

## Open questions surfaced

1. Does the corpus's `reads_field: line_ids` triple fully name the
   method's read set? Or are there transitive reads via
   `line_ids.X.Y` paths the corpus didn't unwind? (Cross-record reads
   that the WHEN-clause-collapse fix would need to inform too.)
2. Are `_compute_amount`'s signed companion fields (`*_signed`) actually
   separate `emitted_by` triples, or are they derived from
   `move_type`-based sign-flips of the base fields? (Affects whether
   the projection needs to emit 4 fields with separate `VALUE` clauses
   or 4 fields that share the `$this._compute_amount.<field>` access.)
3. Does the Python source reference `self.company_id.currency_id` or
   `self.currency_id`? The slice-2 corpus has `currency_id` directly on
   `account_move` — clarifying which currency wins (line / move /
   company) gates GAP-1's function signature.

## What ships when

This spec **does not** modify `emit.rs`. It informs:

1. The deferred `FunctionDefinition::stub()` → `FunctionDefinition` with
   a real body, once Python is captured.
2. The CORE GAP proposals, which would land in the AdaWorldAPI/surrealdb
   fork as `fn::core::currency::*` standard functions (out of scope for
   odoo-rs; in scope for the substrate-bump arc).
3. Future per-method specs in this directory follow the same template
   (see `README.md`).
