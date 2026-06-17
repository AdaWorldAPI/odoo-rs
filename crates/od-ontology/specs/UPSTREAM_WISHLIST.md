# Wishlist — for the lance-graph composition / ClassView / inheritance session

> **From:** odoo-rs (Python → SurrealDB transcode), `AdaWorldAPI/odoo-rs`
> `main @ 7d0c48d`, 2026-06-17.
>
> **For:** whichever lance-graph session is currently driving the
> `classid → ClassView` / composition / inheritance / recompute-ordering
> design (per `core-first-transcode-doctrine.md` § "OGAR's movable parts",
> v2 two-tier compile ladder, v3 elixir-tissue ladder).
>
> **Status of this file:** **CONSUMER-SIDE REQUIREMENTS**, not a design
> proposal. Your session owns the design; this names what odoo-rs would
> consume once your design stabilizes, so you have one real downstream
> use-case to design against instead of designing in a vacuum.
>
> **Status of the upstream pieces this depends on:** lance-graph's
> ClassView implementation is currently POC ("it works somehow" per
> upstream's own honest framing); the doctrine doc itself carries
> `Status: CONJECTURE` in its header; SurrealDB-side has zero inherited
> specs yet. We are NOT going to invent a parallel ClassView in
> od-ontology to fill the gap — that would be the residue-Core
> anti-pattern the doctrine warns against. We'll wait, and consume what
> lands.

## Why this file exists

Two `core-gap-auditor` findings (commits `ed83a63` → `7d0c48d`) on the
spec for `account.move._compute_amount` showed a structural class of
bug that no projection-level primitive of ours could catch — only a
knowing human reader could:

> `account.move._compute_amount` reads `line.amount_residual`,
> which is itself emitted by `account.move.line._compute_amount_residual`.
> Different model. Must-run-before ordering. **P0 dependency cycle** in
> the original SurrealQL body if emitted as-written.

The doctrine names the missing primitive: `composition/inheritance =
classid → ClassView`, with the SPO `has_function` / `inherits_from` /
`virtually_overrides` triples as the **method-resolution manifest**.
This file names what odoo-rs would consume from a stabilized ClassView
to retire its per-method audit ritual.

## What odoo-rs needs — prioritized

### P0 · Cross-method recompute-ordering DAG

**The minimum primitive.** Given the SPO corpus's `emitted_by` and
`reads_field` triples, produce a topological order over `(model,
method)` pairs such that no method runs before a method whose
`emitted_by` field appears in its `reads_field`.

- **Consumer use:** `corpus_to_schema` would emit
  `DEFINE FIELD` and `DEFINE FUNCTION` statements in that order so
  SurrealDB's reactive `VALUE` cascade matches Odoo's compute order;
  alternatively, the projection annotates each `DEFINE FUNCTION` body
  with a `-- depends_on_recompute: <list>` comment + (eventually) a
  `DEFINE EVENT` chain that respects the order.
- **Acceptance:** the P0 finding from the `_compute_amount` audit
  (cross-model dep on `line.amount_residual`) would have surfaced as a
  structural annotation on the projection, not as a 4-test-gate
  audit-ritual catch.
- **Failure mode if absent:** every `_compute_*` spec runs the audit
  ritual by hand to catch cross-model cycles. Tractable at this scale
  (2 specs), gets unbearable at scale (~80 Money-computes, ~150 guards).

### P1 · `_inherit` (mixin) flattening

Odoo's `_inherit = 'mail.thread'` is mixin composition. The parent's
fields / methods / decorators flatten into the child as if declared
there.

- **Consumer use:** for each Odoo model with `_inherit = [...]`, emit
  the union of every parent class's `DEFINE FIELD` / `DEFINE FUNCTION` /
  `DEFINE EVENT` on the inheriting model's table.
- **Acceptance:** `account.move` (which `_inherit`s `portal.mixin`,
  `mail.thread`, `mail.activity.mixin`, `sequence.mixin`) emits the
  union of all four parents' fields, methods, guards.
- **Where the data lives:** likely in the typed `OdooEntity` blueprint
  (`lance-graph-ontology::odoo_blueprint`) as an explicit `inherits:
  Vec<&'static str>` slot. The SPO corpus may or may not carry
  `inherits_from` triples for Python today (Ruby PR #6 has it; not
  verified for Python yet).

### P1 · `_inherits` (delegation) flattening

Odoo's `_inherits = {'res.partner': 'partner_id'}` is **delegation**
(distinct from mixin): `self.name` proxies through `self.partner_id.name`.
SurrealDB has no native delegation.

- **Consumer use:** for each `_inherits` declaration, emit:
  ```surql
  DEFINE FIELD <parent_field> ON <child>
    VALUE $this.<delegated_relation>.<parent_field>
    READONLY;
  ```
  — same shape as the One2many virtual projection just shipped
  (`695c1c8`). The parent's fields appear as read-only projections on
  the child table.
- **Acceptance:** `res.users _inherits {'res.partner': 'partner_id'}`
  → `name`, `email`, `phone` etc. from `res.partner` appear as accessor
  projections on `res_users`.

### P2 · `virtually_overrides` precedence

When a child model `_inherit`s a parent and overrides a `_compute_*` or
`_check_*` method, the corpus needs to name the precedence so the
projection emits the WINNING body, not both.

- **Consumer use:** the override winner gets the `DEFINE FUNCTION`
  body; the loser is suppressed or annotated as `-- shadowed_by: <id>`.
- **Today's gap:** the Odoo SPO extractor likely doesn't emit
  `virtually_overrides` triples (Ruby PR #9 added them for Rails; not
  verified for Python). Harvest extension would be needed.

### P3 · Selection-field value enumeration

**Not inheritance, but related** — the second audit finding (MISSED-2,
wrong `display_type` partition into `tax`/`base`) is a consequence of
the Selection field's value-set not being captured anywhere.
`OdooFieldKind::Selection` exists in the typed blueprint, but neither
the blueprint nor the SPO corpus enumerates which values the field
accepts.

- **Consumer use:** enumerated Selection values let the projection
  emit `ASSERT $value IN ["product", "tax", "payment_term", "rounding"]`
  on the field, and let downstream specs partition on the values
  verbatim.
- **Where the data lives:** Python source — `display_type =
  fields.Selection([('product', ...), ('tax', ...), ...])`. Either the
  typed blueprint grows a `selection_values: &'static [&'static str]`
  slot, or the SPO corpus emits one triple per allowed value.

## What odoo-rs explicitly does NOT need (scope-fencing)

- A **runtime ClassView dispatcher**. We codegen-flatten at DDL emit
  time; SurrealDB has no runtime MRO. ClassView for us is a
  build-time data structure read by the projection, not a runtime
  surface.
- **Hot-swap of method bodies** (v3 elixir-tissue ladder). Not
  blocking; relevant for the tesseract arc, not for odoo-rs's near-term
  path.
- **A separate adapter-bodies AST** (`tesseract-rs-ast-dll-codegen-v1`
  analog). For Odoo, the typed Python compute body becomes a SurrealQL
  `DEFINE FUNCTION` body in one projection step; no intermediate
  AST-DLL needed at this scope.

## What's already enough (no upstream work needed)

- **`has_function` triples** — we consume these for the function set
  per model. Working today via `emit.rs` (`obj::FUNCTION` arm).
- **`record<>` relations for Many2one targets.** ClassView for FK
  direction isn't needed; the doctrine of "Many2one is the FK side"
  is unambiguous.
- **One2many graph traversal arrow** (`VALUE <-child.back_ref READONLY`).
  Already shipped on our side at commit `695c1c8`; no upstream
  dependency.

## Open questions where odoo-rs would want upstream's call

These aren't blocking; they're decisions we don't want to predict, so
we can adopt your choice when it lands.

1. **Where does the inheritance graph live?** In the SPO corpus (new
   `inherits_from` triples — Ruby precedent), in the typed
   `OdooEntity` blueprint (new field), or in a separate spec the
   projection reads alongside? Different tooling implications.
2. **Is `classid` an integer or a string at the consumer surface?**
   Odoo models have string names (`'account.move'`); SurrealDB tables
   are strings. We've been using underscored model names. If the
   future ClassView assumes `classid: u32`, odoo-rs needs a string ↔
   integer translation layer.
3. **Does ClassView model state-machine inheritance?** Odoo's
   `account.move` inherits `mail.thread`'s activity-tracking machinery,
   which is state-machine-like. Out of scope for the audit findings;
   relevant for full Odoo parity at v2/v3.
4. **Does the v1 ladder generalize from C++/Tesseract to Python/Odoo?**
   The doctrine's v1 says "thin adapters target OGAR." For odoo-rs the
   v1 reads as "thin adapters target SurrealDB DDL." Same shape — but
   "adapters target OGAR" assumes a single ABI consumer; we have a
   schema-emit consumer. Does ClassView's design cleanly generalize, or
   is it tesseract-shaped (single-language-target by construction)?

## Current state of odoo-rs (so the design session knows what to design against)

- `corpus_to_schema` projects SPO triples → typed `Schema { tables,
  functions, events }` → SurrealQL text. **42 unit tests + 1 doctest
  green.**
- Two specs in `crates/od-ontology/specs/`:
  - `_compute_amount.md` (`SUPERSEDED-BY-AUDIT` — the P0 cycle)
  - `_check_invoice_currency_rate.md` (`DRAFT-CONJECTURE`)
- 4-test gate codified in `specs/README.md` (multi-reuse /
  algebraic-generality / Frankenstein-guard / pivot-proposal-if-rejected)
  — every future "GAP" must pass before being filed.
- Three follow-ups deferred pending this wishlist's intersection with
  our trajectory:
  - `od-ontology-bridge` crate (walk `OdooEntity` blueprint → full
    `relations.ndjson`); gated on `lance-graph-ontology` typing of
    `_inherit` if we want to consume it in the same pass.
  - Parity probe via `--validate` hook (`surrealdb_core::syn`); gated
    on disk headroom.
  - Slice 3 corpus expansion; explicitly low-priority per operator
    framing.

## Cross-session etiquette

- **Not blocked.** We're naming this so when your ClassView spec
  lands, we adopt it cleanly instead of having invented a parallel
  one in the meantime.
- **No pressure on shape.** Your session owns the shape. This file
  names what we'd *consume*; it doesn't prescribe what you *build*.
- **Honest about uncertainty.** Several of the "P0/P1/P2/P3" tags
  are our consumer guesses at priority for what we'd need first; your
  session knows the design constraints we don't see and may reorder.
- **One-way pull, not push.** Reach via the lance-graph zipball /
  GitHub browse; we don't need a response. Update this file (PR
  against `AdaWorldAPI/odoo-rs`) only if our requirements actually
  drift from what you ship — silence is alignment.
