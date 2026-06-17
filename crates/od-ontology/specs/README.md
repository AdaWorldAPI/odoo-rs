# Per-method adapter specs

Each Odoo compute method, guard, or action that crosses into the
SurrealDB Core gets a spec here BEFORE its DEFINE FUNCTION body is wired
into `emit.rs`. The spec is the *shape* — DO-in / DO-out / body sketch /
CORE GAPs — that the projection eventually emits.

Per [`core-first-transcode-doctrine.md`][doctrine]: the harvest (SPO
corpus) and the codegen (these specs) are two halves of **one** system,
not orthogonal. Each spec ties the corpus's `has_function` /
`emitted_by` / `reads_field` / `raises` triples on a single method to a
concrete adapter SHAPE in `surrealql`.

[doctrine]: https://github.com/AdaWorldAPI/lance-graph/blob/main/.claude/knowledge/core-first-transcode-doctrine.md

## Status conventions

Every spec carries one of:

- **`DRAFT-CONJECTURE`** — shape derived from the SPO corpus + doctrine
  application; Python body not yet captured (likely the source isn't
  on this machine). Parity oracle is the GATE: the spec doesn't ship
  into the projection until the Python is read + a fixture capture
  exists.
- **`SPEC-FROZEN`** — Python read, body verified expressible, parity
  oracle captured. Ready for projection wiring.
- **`SHIPPED`** — emitted by the projection; covered by a regression
  test.

## Route conventions

- **`adapter`** — mechanical / data-shaped leaf method (Odoo `_compute_*`).
  Fits the thin DEFINE FUNCTION adapter mold. May surface CORE GAPs —
  but those gaps MUST pass the 4-test gate (see below) before being
  proposed as EXTEND-CORE rather than ADAPTER-HACK.
- **`guard_adapter`** — variant of `adapter` for `@api.constrains` guard
  methods (Odoo `_check_*`). DOES emit (a `DEFINE EVENT` with `WHEN` +
  `THEN { THROW … }`), does NOT write fields (sentinel `do_out.writes_field:
  null`), and uses `emit_shape: event`. The compute YAML template
  generalizes cleanly; see `_check_invoice_currency_rate.md` for the
  worked example.
- **`hand_port`** — intrusive / stateful method (raw SQL, hash-chain side
  effects, `env.cr.execute`, transactional fencing). Doctrine §
  "Frankenstein-flattening guard" routes these AWAY from the adapter
  mold to direct hand-port behind a feature gate; the surrounding
  mechanical surface stays in the adapter.

## CORE-GAP gate — the 4-test discipline (MANDATORY before any gap is filed)

A "GAP" only counts as EXTEND-CORE if it passes ALL four tests below.
Failing any single test demotes the proposal to ADAPTER-HACK and the
spec must be re-shaped to use existing Core primitives. Established by
the `_compute_amount` audit (see `_compute_amount.md` § "Audit verdict"
— `core-gap-auditor` 2026-06-17 rejected BOTH originally-proposed GAPs).

1. **Multi-reuse test.** Does the proposed primitive serve ≥ ~20 future
   adapters? Quantify against the SPO corpus (grep the relevant
   triple shapes).
2. **Algebraic-generality test.** Does the primitive fit the same
   *kind* of operator as the Core's existing standard-fn surface (pure
   stateless `fn(args) -> Value` in surrealdb-core's `fnc/*`)? Or is it
   a DB-table-backed lookup masquerading as algebra? **DB reads inside
   standard-fn = category bend = ADAPTER-HACK.**
3. **Frankenstein-guard.** Is the proposed shape genuinely
   mechanical/data-shaped, or does it smuggle intrusive state that
   production usage would need (business-day calendars, holiday tables,
   fiscal-period boundaries, multi-provider tables, enum
   discriminators)? Anything silently smuggled = ADAPTER-HACK.
4. **Pivot-proposal test.** If a "GAP" really is just "an existing Core
   primitive used inline" (`DEFINE TABLE` + `SELECT` subquery,
   `math::fixed` on a stored scalar), the doctrine's "the Core empowers
   the adapter to be thin" claim already holds — there is no gap. The
   adapter author was wishing the Core looked like their domain.

**Reject reflex:** when the SECOND `_compute_*` or `_check_*` method
would naturally consume the proposed primitive as an inline subquery
against a regular `DEFINE TABLE`, that's not multi-reuse evidence
*for* the gap — that's evidence the existing primitive **already**
covers both call sites and no gap exists.

## Per-method spec template (YAML)

```yaml
method:
  qualname: <model>.<method>            # account.move._compute_amount
  classid: <model>                      # table identity (dots→underscores at codegen)
  route: adapter | hand_port
  status: DRAFT-CONJECTURE | SPEC-FROZEN | SHIPPED
  do_in:
    reads_field:    [<table.field>]     # from SPO `reads_field` triples
    reads_relation: [<rel>: <target>]   # Many2one / One2many chains
  do_out:
    writes_field:   [<table.field>]     # from SPO `emitted_by` (REVERSED — emit → method)
  emit_shape: scalar | object           # one DEFINE FUNCTION, return type
  body_sketch: |
    <surrealql>                         # with `fn::core::*` markers for unresolved primitives
  core_gaps:                            # EXTEND-CORE proposals, not adapter hacks
    - name: <primitive>
      proposed_table:    <DEFINE TABLE …>
      proposed_function: <DEFINE FUNCTION fn::core::…>
      reused_by:         [<other methods that would share this primitive>]
  composed_by:
    has_function:        [<class>]      # SPO has_function inversions
    virtually_overrides: [<base>]       # for MRO flattening (when v3 lands)
  parity_oracle:
    odoo_method:    <model>._<method>
    fixture_set:    <path or in-corpus tag>
    field_tuple:    [<output fields>]
    rounding:       banker | per_currency_rounding
  frankenstein_check:
    side_effect_triples: []             # MUST be empty for route=adapter
    raw_sql_triples:     []             # MUST be empty for route=adapter
    flattening_notes:    <text>
```

## Current specs

| Method | Route | Status | CORE GAPs | File |
|---|---|---|---|---|
| `account.move._compute_amount` | `adapter` | **SUPERSEDED-BY-AUDIT** (pivot in spec) | **0** (originally proposed 2; both REJECTED as ADAPTER-HACK per 4-test gate) | [`_compute_amount.md`](./_compute_amount.md) |
| `account.move._check_invoice_currency_rate` | `guard_adapter` | DRAFT-CONJECTURE | 0 | [`_check_invoice_currency_rate.md`](./_check_invoice_currency_rate.md) |

## Cross-session communication

- [`UPSTREAM_WISHLIST.md`](./UPSTREAM_WISHLIST.md) — consumer-side
  requirements from odoo-rs for whichever lance-graph session is
  driving the `classid → ClassView` / composition / inheritance design.
  Honest framing: lance-graph's implementation is POC, SurrealDB-side
  has zero inherited specs, the doctrine doc itself is `CONJECTURE`.
  We will NOT invent a parallel ClassView here; we'll consume what
  lands. The wishlist names what we'd consume so the upstream session
  has one real downstream use-case to design against.

## Why specs and not just generated code

The projection (`emit::corpus_to_schema`) is **the codegen half** of the
doctrine's two-halves-one-system framing. The specs in this directory
are the **harvest-and-design half** — they read the SPO corpus + the
Python source + the doctrine, and write down the SHAPE the codegen
should produce per method. Once a spec is `SPEC-FROZEN`, the projection
inherits its body sketch verbatim.

Specs that surface CORE GAPs feed back into the substrate (the
SurrealDB fork): the proposed `DEFINE TABLE` / `DEFINE FUNCTION` items
become extend-the-Core proposals that ~80 future Money-compute methods
would share. **One Core grow-bump > 80 adapter hacks**, per doctrine.
