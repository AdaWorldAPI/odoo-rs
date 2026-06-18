# `account.move._check_invoice_currency_rate` — guard adapter spec

> **Status:** `DRAFT-CONJECTURE` (2026-06-17). Shape derived from the SPO
> corpus + Core-First Transcode Doctrine. Python body **not yet captured**
> on this host; parity oracle is the GATE.
> **Authored by:** `adapter-shaper` agent (subagent run 2026-06-17), with
> the framing corrected post-`core-gap-auditor` verdict on
> `_compute_amount`'s CORE GAPs (see `_compute_amount.md` § "Audit
> verdict" — `res_currency_rate` is a regular `DEFINE TABLE` the
> transcode emits, NOT a substrate-bump primitive).

## Route — `guard_adapter` (variant of ADAPTER for `@api.constrains`)

Pure existence-check against the `res_currency_rate` table. No
`with_context`, no `env.cr.execute`, no transactional fencing.
Mechanical / data-shaped — fits the doctrine's adapter scope cleanly.

### Frankenstein-flattening guard

SPO corpus for `_check_invoice_currency_rate`:

- **0** `side_effect` triples
- **0** `raw_sql` triples
- **0** `writes_field` triples (guards raise, they don't emit)
- **1** `raises` triple (→ `exc:ValidationError`)
- **0** `reads_field` triples present in the harvest (harvest gap noted;
  spec drawn from upstream Odoo knowledge of `@api.constrains` shape)

Pure existence-check. **Routes adapter cleanly.**

## Guard vs compute — spec-template generalization

The `specs/README.md` YAML template was authored around compute methods
(DO-in, DO-out, `emit_shape: scalar | object`, body returns a value).
For guards, the same template works with **slot renaming, no
sub-directory needed**:

| Compute slot | Guard mapping |
|---|---|
| `route: adapter` | `route: guard_adapter` (variant) |
| `do_in.reads_field` | identical — fields the WHEN/THEN read |
| `do_out.writes_field` | **`null`** (sentinel: guards raise, they don't write) |
| `emit_shape: scalar \| object` | `emit_shape: event` (DEFINE EVENT, not DEFINE FUNCTION) |
| `body_sketch` | contains the WHEN+THEN pair, not a function body |
| `parity_oracle.field_tuple` | renamed `raises_with` — exception type + message format |

The template extension is a **one-line variant note** in
`specs/README.md`. No `specs/guards/` tree, no template duplication.

## DO-in / DO-out

### DO-in

- `account_move.move_type` — filter to invoice-typed moves
  (`out_invoice` / `out_refund` / `in_invoice` / `in_refund` /
  `out_receipt` / `in_receipt`)
- `account_move.currency_id` — line currency (Many2one)
- `account_move.date` — rate-lookup key
- `account_move.company_id` — Many2one
- `account_move.company_id.currency_id` — target currency (Many2one chain)
- `res_currency_rate` — table-level lookup with `(currency_id,
  company_id, name)` keyed scan (per audit verdict: a regular Odoo
  table emitted by the transcode, NOT a Core extension)

### DO-out

**`null`** — guards raise, they don't write fields.

## SurrealQL body sketch — DEFINE EVENT

```surql
DEFINE EVENT check_invoice_currency_rate ON account_move
    WHEN $event IN ["CREATE", "UPDATE"]
      AND $after.move_type IN [
          "out_invoice", "out_refund",
          "in_invoice",  "in_refund",
          "out_receipt", "in_receipt"
      ]
      AND $after.currency_id != NONE
      AND $after.currency_id != $after.company_id.currency_id
    THEN {
        LET $has_rate = (
            SELECT VALUE id FROM res_currency_rate
            WHERE currency_id = $after.currency_id
              AND company_id  = $after.company_id
              AND name       <= $after.date
            ORDER BY name DESC LIMIT 1
        )[0];
        IF $has_rate = NONE {
            THROW "ValidationError: no currency rate on file for "
                + string::concat(
                    $after.currency_id.name, " → ",
                    $after.company_id.currency_id.name,
                    " as of ", string::concat($after.date)
                );
        };
    };
```

**No `DEFINE FUNCTION` needed** — the guard's predicate logic IS the
WHEN/THEN. The Core's reactive trigger fires; `THROW` raises the
equivalent of Python's `ValidationError`.

**The rate lookup is an inline SurrealQL subquery against
`res_currency_rate`** — a regular `DEFINE TABLE` the transcode emits.
No `fn::core::currency::*` namespace. Per the audit verdict on
`_compute_amount`'s GAPs: the Core already has the right primitive
(table reads with date-ordered scans); inventing a `fn::core::*`
helper would be ADAPTER-HACK.

## YAML spec

```yaml
method:
  qualname: account.move._check_invoice_currency_rate
  classid: account_move
  route: guard_adapter
  status: DRAFT-CONJECTURE
  do_in:
    reads_field:
      - account_move.move_type
      - account_move.currency_id
      - account_move.date
      - account_move.company_id
      - account_move.company_id.currency_id
    reads_relation:
      - "res_currency_rate via (currency_id, company_id, name)"
  do_out:
    writes_field: null              # guards don't emit
  emit_shape: event                 # DEFINE EVENT, not DEFINE FUNCTION
  body_sketch: |
    # See § "SurrealQL body sketch" above for the full DEFINE EVENT.
    # WHEN (filter): invoice-typed move with foreign currency
    # THEN (predicate): SELECT FROM res_currency_rate LIMIT 1 → IF NONE THROW
  core_gaps: []                     # NONE — audit reversed prior framing
  composed_by:
    has_function:        [account_move]
    virtually_overrides: []
  parity_oracle:
    odoo_method:  account.move._check_invoice_currency_rate
    fixture_set: |
      Invoice (out_invoice or similar) with currency_id != company.currency_id,
      one fixture with a res_currency_rate row matching (currency, company, date)
      → no raise; another fixture without a matching row → ValidationError raised.
    raises_with:  ValidationError
    message_fmt:  |
      Odoo i18n-translates the message; diff-gate checks exception
      type + raised/not-raised, NOT the message string verbatim.
  frankenstein_check:
    side_effect_triples: []
    raw_sql_triples:     []
    flattening_notes: |
      Pure existence-check, no with_context, no env.cr. Adapter route is clean.
```

## CORE-FIT verdict

**TARGETS-CORE.** Consumes `res_currency_rate` as a regular table via
inline subquery — same shape as any One2many lookup the adapter already
does. **No substrate-bump required.**

> **2026-06-17 — corpus enrichment opportunity (ruff#21 ratification).**
> `AdaWorldAPI/ruff#21` ("emit validation_kind triple per recognised
> Rails validation key", merged 2026-06-17) adds
> `Predicate::ValidationKind` to the ruff vocab and demonstrates the
> typed-constraint shape cross-language for Rails: per-attribute
> `(attribute_iri, validation_kind, "presence"|"uniqueness"|"length"|…)`.
> This guard is structurally a **`currency_rate_lookup` validation_kind**
> on `account.move._check_invoice_currency_rate` — a related-row-must-
> exist guard scoped by `(currency, company, date)`. If the Odoo
> extractor adopts the predicate, the `WHEN` filter + `THROW` message
> become specializable per kind rather than spec-side notes. See
> `UPSTREAM_WISHLIST.md` § "ruff#21 — `validation_kind` is a NEW
> cross-language predicate" for the full ask. **This spec stays
> DRAFT-CONJECTURE; no projection change today.**

> **2026-06-18 — validation_kind LANDED, but as `range` not `lookup`.**
> lance-graph PR [#526](https://github.com/AdaWorldAPI/lance-graph/pull/526)
> + corpus regen [#527](https://github.com/AdaWorldAPI/lance-graph/pull/527)
> shipped `validation_kind`. The fresh corpus carries
> `(odoo:account_move._check_invoice_currency_rate, validation_kind,
> "range")` — **not** the `lookup` kind this spec's intuition
> suggested. The AST classifier reading Odoo's actual Python body
> detects a `< | > | <= | >=` comparison (likely against a numeric
> rate value), classifying it as `range`. **The spec's framing was
> wrong about the underlying detector pattern.** The semantic
> shape — "a related-row-must-exist guard scoped by (currency,
> company, date)" — still describes the SurrealQL lowering correctly
> (the projection's `WHEN` filter + `IF NONE` subquery against
> `res_currency_rate` is unchanged), but the corpus classifies the
> *check shape* as range. Useful FINDING: the consumer cannot rely on
> the validation_kind to predict the SurrealQL emit shape; the kind
> describes the *AST pattern detected*, not the *semantic role*.

## Minor sugar gaps surfaced (non-blockers, NOT proposed as Core extensions)

- **`THROW` with interpolated values.** Today's SurrealQL accepts a
  string; the spec uses string-concatenation (verbose but functional).
  An Odoo-shaped `THROW { exc, msg, params }` envelope variant would be
  ergonomic. **Filed as ergonomic sugar; not a substrate-bump.** Reused
  by every `_check_*` guard (~150 across Odoo).

- **`EXISTS` quantifier shorthand.** `IF (SELECT ... LIMIT 1)[0] = NONE`
  reads verbose; `IF !EXISTS (...)` would read cleaner. **Sugar, not
  primitive.** ~40 guards would benefit.

Both are SurrealQL-language ergonomic suggestions for the substrate
team, NOT EXTEND-CORE proposals. The verbose forms above are correct
and parity-stable.

## Open questions

1. Does Odoo's `_check_invoice_currency_rate` actually check
   `move_type` (the spec assumes only invoice-typed moves), or does it
   apply more broadly? Python source capture needed.
2. The audit on `_compute_amount` identified `display_type` as the
   correct line-partition discriminator (not `tax_line_id != NONE`). Does
   the guard need any `display_type`-aware logic, or does it operate
   purely on `account_move` head-level fields? Likely the latter, but
   worth confirming.
3. What's the exact i18n key for the `ValidationError` message? The
   parity oracle's diff-gate as-defined (exception type + raised/not)
   sidesteps this, but eventual full parity would need the key set.

## What ships when

This spec **does not** modify `emit.rs`. The current emit path at
`emit.rs:334-356` produces the bare `IF !fn::<model>::<method>($after)
{ THROW ... }` shape; once this spec is `SPEC-FROZEN`, the projection
inherits the inlined existence-check body verbatim (replacing the
non-existent `fn::` call with the `SELECT ... res_currency_rate ...`
subquery).
