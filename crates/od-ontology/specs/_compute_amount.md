# `account.move._compute_amount` — adapter spec

> **Status:** `SUPERSEDED-BY-AUDIT` (2026-06-17, post-`core-gap-auditor`
> verdict — see § "Audit verdict" at the end of this file). Original
> framing (`DRAFT-CONJECTURE` per the adapter-shaper run) below is
> retained verbatim for traceability; the **pivot body sketch** and
> bug-list in the supersession section are the canonical going-forward
> shape. The previous "CORE GAPs as EXTEND-CORE proposals" framing was
> doctrinally wrong: `res_currency_rate` is just another `DEFINE
> TABLE` the transcode emits (not a substrate-bump primitive), and
> per-currency rounding already has the right algebraic Core primitive
> (`math::fixed`). Plus the audit caught **two real bugs** in the
> original sketch: P0 dependency cycle on `reconciled`, P1 wrong
> tax/base discriminator (`tax_line_id != NONE` instead of
> `display_type`).
>
> **Authored by:** `adapter-shaper` agent (original run 2026-06-17),
> reversed by `core-gap-auditor` agent (audit run 2026-06-17).
> **Read the audit verdict first** (jump to bottom) — the rest of this
> file is historical context.

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
2. **~~The CORE GAP proposals, which would land in the AdaWorldAPI/surrealdb
   fork as `fn::core::currency::*` standard functions~~ — REVERSED by
   audit, see § "Audit verdict" below. No substrate-bump PR; both
   "GAPs" use the regular `DEFINE TABLE` + algebraic-`math::fixed` path.**
3. Future per-method specs in this directory follow the same template
   (see `README.md`).

---

# Audit verdict — `core-gap-auditor` 2026-06-17

> **Both proposed CORE GAPs REJECTED as ADAPTER-HACK.** No substrate-bump
> PR. The Core already has the right primitives.

## Per-GAP verdict (4-test summary)

### GAP-1 (currency conversion) — **REJECT: ADAPTER-HACK**

- **Multi-reuse:** PASS (93 unique `_compute_(amount|price|tax|total|balance)*` methods + 274 currency/Monetary triples in the SPO corpus — original "~80" claim is sound).
- **Algebraic-generality:** **FAIL.** Every existing `fnc/*` module in the AdaWorldAPI surrealdb fork is **pure stateless** `fn(args) -> Value`. `currency::convert($amount, $from, $to, $date)` would be the **first standard-fn requiring a DB read** — a category bend. Domain-table dispatch dressed as algebra is not the same kind of operator as `math::sum`.
- **Frankenstein-guard:** **FAIL.** Production currency conversion needs rounding mode per rate, fallback rates (weekend → previous business day), `company_id`-scoped multi-rate-provider tables (ECB vs internal), rate-direction (CCY→Company vs CCY→CCY pivot), `res.currency.rate.rate_string` inverse-rate display, `_convert(round=True)` per-call rounding flag. The 4-field UNIQUE index smuggles "one (currency, company, date) → one rate" — Odoo's `_get_rates` does range-scan `name <= date ORDER BY name DESC LIMIT 1`.
- **One-sentence:** A DB-table-backed lookup function dressed as a standard-fn is a domain bolt-on masquerading as algebra; ~80 reuses doesn't make a domain table a Core primitive.

### GAP-2 (per-currency rounding) — **REJECT: ADAPTER-HACK (lighter)**

- **Multi-reuse:** PASS (same surface).
- **Algebraic-generality:** **FAIL.** `round_to($amount, $currency)` reads `res_currency.rounding` — a record-field deref, not a standard-fn. **`math::fixed($amount, $precision)` already exists** and IS the right algebraic primitive.
- **Frankenstein-guard:** **FAIL.** Real `currency.round()` needs the rounding-method enum (`HALF-UP` / `UP` / `DOWN`) that Odoo's `res.currency.rounding_method` carries — silently missing.
- **One-sentence:** `math::fixed($x, $currency.decimal_places)` is the existing Core primitive; the proposed `currency::round_to` is sugar for a record-field deref + `math::fixed`.

## Missed gaps the original spec smuggled past

### MISSED-1 (P0 — the spec is logically circular as written)

`reconciled` is itself **emitted by `_compute_amount_residual`** AND **depends on `matched_debit_ids` / `matched_credit_ids`** (partial-reconciliation match-edge tables). The original `$lines[WHERE reconciled = true]` reads inside `_compute_amount` is a **dependency cycle** unless the projection orders the recompute graph. **Fix-required before parity oracle capture.**

### MISSED-2 (P1 — produces wrong totals on real invoices)

`tax_line_id != NONE` as the tax/base discriminator is **not how Odoo distinguishes them**. Odoo partitions via `line.display_type` (`product` / `tax` / `payment_term` / `rounding`) OR `line.tax_line_id` populated. The original binary partition silently drops `payment_term` and `rounding` lines that **must not** be summed into `untaxed`. Wrong totals for invoices with cash-discount or rounding lines.

## Corrected SurrealQL body sketch (pivot — no substrate-bump)

```surql
DEFINE FUNCTION fn::account_move::_compute_amount($this: record<account_move>) {
    LET $lines       = $this.line_ids;
    LET $company_ccy = $this.company_id.currency_id;
    LET $date        = $this.date;
    LET $sign        = IF $this.move_type IN ["in_invoice", "in_refund"] THEN -1 ELSE 1 END;

    -- MISSED-2 fix: partition by display_type, NOT tax_line_id != NONE.
    LET $tax_lines  = $lines[WHERE display_type = "tax"];
    LET $base_lines = $lines[WHERE display_type = "product"];
    --                  ↑ deliberately excludes "payment_term" and "rounding"
    --                    lines that must not be summed into untaxed.

    -- Rate lookup is an INLINE SUBQUERY against res_currency_rate
    -- (a regular DEFINE TABLE the transcode emits — per audit verdict,
    -- not a Core extension). Same shape as any One2many lookup.
    LET $convert = |$amount: decimal, $from: record<res_currency>| {
        IF $from = $company_ccy THEN $amount
        ELSE {
            LET $rate = (
                SELECT VALUE rate FROM res_currency_rate
                WHERE currency_id = $from
                  AND company_id  = $this.company_id
                  AND name       <= $date
                ORDER BY name DESC LIMIT 1
            )[0];
            -- If no rate exists, _check_invoice_currency_rate guard
            -- (see _check_invoice_currency_rate.md) raises before
            -- this function is reached; we can safely deref.
            $amount * $rate
        }
    };

    -- GAP-2 fix: math::fixed (existing Core primitive) with the
    -- currency's decimal_places (a stored scalar on res_currency).
    LET $round = |$amount: decimal, $ccy: record<res_currency>|
        math::fixed($amount, $ccy.decimal_places);

    LET $untaxed = $round(math::sum($base_lines.map(|$l| $convert($l.balance, $l.currency_id))), $company_ccy);
    LET $tax     = $round(math::sum($tax_lines.map(|$l|  $convert($l.balance, $l.currency_id))), $company_ccy);
    LET $total   = $untaxed + $tax;

    -- MISSED-1 fix: read amount_residual from each line directly
    -- (it's itself emitted_by line._compute_amount_residual; the
    -- projection's recompute-ordering must run line first).
    -- DO NOT filter by `reconciled` here — that field is itself
    -- computed and creates a cycle. Use the line-level residual
    -- which already incorporates reconciliation.
    LET $residual = $round(math::sum($lines.map(|$l| $convert($l.amount_residual, $l.currency_id))), $company_ccy);

    RETURN {
        amount_untaxed:                   $untaxed,
        amount_tax:                       $tax,
        amount_total:                     $total,
        amount_residual:                  $residual,
        amount_untaxed_signed:            $untaxed  * $sign,
        amount_tax_signed:                $tax      * $sign,
        amount_total_signed:              $total    * $sign,
        amount_residual_signed:           $residual * $sign,
        amount_total_in_currency_signed:  $total    * $sign,
        amount_untaxed_in_currency_signed: $untaxed * $sign,
    };
};
```

## Open work to close before SPEC-FROZEN

1. **MISSED-1 fix verification:** confirm `account_move_line.amount_residual` (line-level) genuinely incorporates reconciliation so the head-level `_compute_amount` doesn't have to. Likely yes per Odoo's `_compute_amount_residual` on the line model.
2. **MISSED-2 fix verification:** confirm `display_type` values are exactly `["product", "tax", "payment_term", "rounding"]` (or whatever the Odoo enum is) by reading the Python source.
3. **Inline-subquery rate-lookup verification:** the inline `SELECT VALUE rate FROM res_currency_rate WHERE ... LIMIT 1` shape works in SurrealQL VALUE clauses. Needs the deferred `--validate` hook (or any other parser check) to confirm syntax.
4. **Recompute-ordering wiring:** when `_compute_amount` reads
   `line.amount_residual` which itself reads `line.matched_debit_ids` /
   `line.matched_credit_ids`, the projection must order the dependency
   graph correctly. **The projection currently doesn't model this.** Open
   architectural question — defer to a future commit, name in the
   open-questions table below.

## Doctrinal lesson (the bigger reason this matters)

The original adapter-shaper run proposed "EXTEND-CORE" framings that *look* like the doctrine but inverted its actual rule. The 4-test discipline of the gap-auditor caught it cleanly:

> The Core's existing primitives (`DEFINE TABLE`, `SELECT`, `math::fixed`)
> ARE the primitives this adapter needs. The proposed `fn::core::currency::*`
> namespace was the adapter wishing the Core looked like its own
> domain — exactly the residue-Core anti-pattern the doctrine warns
> against. **One adapter's convenience function is not a Core primitive.**

Future specs in this directory MUST run the 4-test gate (multi-reuse + algebraic-generality + Frankenstein-guard + the specific-pivot-proposal-if-rejected) before declaring any "GAP." `specs/README.md` updated with the protocol.

## Status now

`SUPERSEDED-BY-AUDIT`. The pivot body sketch above is the going-forward shape; once MISSED-1 / MISSED-2 are verified against the Python source and the recompute-ordering question is answered, this spec promotes to `SPEC-FROZEN`.
