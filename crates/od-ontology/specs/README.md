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

- **`ADAPTER`** — mechanical / data-shaped leaf method. Fits the thin
  DEFINE FUNCTION adapter mold. May surface one or more CORE GAPs that
  need EXTEND-CORE before the body is fully expressible (gaps are NOT
  adapter-hack territory).
- **`HAND-PORT`** — intrusive / stateful method (raw SQL,
  hash-chain side effects, `env.cr.execute`, transactional fencing).
  Doctrine § "Frankenstein-flattening guard" routes these AWAY from the
  adapter mold to direct hand-port behind a feature gate; the
  surrounding mechanical surface stays in the adapter.

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
| `account.move._compute_amount` | ADAPTER | DRAFT-CONJECTURE | 2 (currency convert + rounding) | [`_compute_amount.md`](./_compute_amount.md) |

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
