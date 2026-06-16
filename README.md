# odoo-rs

**Odoo's business-logic ontology, lowered to a native SurrealDB schema.**

Odoo is not a SQL application with logic bolted on. **Odoo's ORM _is_ an
ontology** — every construct is a runtime assertion the framework executes:

| Odoo construct | What it asserts |
|---|---|
| `class AccountMove(models.Model): _name = 'account.move'` | an object type |
| `name = fields.Char(required=True)` | a typed, constrained property |
| `partner_id = fields.Many2one('res.partner', ondelete='cascade')` | a typed relation with referential semantics |
| `amount = fields.Monetary(compute='_compute_amount', store=True)` | a **reactive computed** property |
| `@api.depends('line_ids.balance')` | a **causal edge** in the compute graph |
| `@api.constrains('date')` | an invariant guard |
| `_sql_constraints = [('uniq', 'unique(code,company_id)', …)]` | a structural unique constraint |
| `def action_post(self): …` | a typed method with side effects |
| `groups = 'account.group_account_user'` | a per-property ACL |

This repo treats that ontology as the deliverable. The
[22 245-triple SPO corpus](https://github.com/AdaWorldAPI/lance-graph)
already extracted from the Odoo source (388 object types, 3 328 functions,
3 107 properties, **6 309 `@api.depends` causal edges**) *is* the ontology in
machine-readable form. `od-ontology` lowers it into **native SurrealDB
constructs** so the semantics live *in the database*:

```
  SPO corpus  ──corpus_to_schema──►  Schema { tables, functions, events }
                                         │ ToSql
                                         ▼
                  DEFINE TABLE / FIELD (VALUE, ASSERT, READONLY) / FUNCTION / EVENT
```

## Why SurrealDB, not sea-orm

sea-orm is **sink-in**: rows marshal into Rust structs, logic runs in the
binary, results persist back — the database is a dumb store. That is the
*opposite* of ActiveRecord / Odoo, where the class **is** the table, computed
fields recompute reactively, validations fire at write, and methods are looked
up on records. SurrealDB is the one target that expresses Odoo's semantics
natively:

| Odoo | SurrealQL |
|---|---|
| `compute='_x', store=True` | `DEFINE FIELD x … VALUE fn::model::_x($this) READONLY` |
| `@api.depends('a','b')` (same row) | implicit in the `VALUE` recompute-on-write |
| `@api.depends('rel.sub')` (cross row) | `DEFINE EVENT … ON <child> WHEN … THEN UPDATE <parent>` |
| `@api.constrains` / `_check_*` | `DEFINE EVENT … THEN { IF … { THROW … } }` |
| `def action_post(self)` | `DEFINE FUNCTION fn::model::action_post($this)` |
| `_sql_constraints unique(…)` | `DEFINE INDEX … UNIQUE` |
| `groups = …` | `PERMISSIONS FOR … WHERE …` |

The `@api.depends` graph (6 309 edges) becomes **first-class dataflow in the
database**, not boilerplate event handlers in a binary. That is the difference
between "we generated some Rust" and "we ported the ontology faithfully."

## Faithful now vs deferred

The **reactive wiring** — which field recomputes over what, which guard fires,
which method materialises which field, which relations link where, which deps
cross records — is **100 % derivable from the corpus and lands immediately**.

The compute/guard **bodies** (Python expressions), exact field types beyond the
name heuristic, and cross-record child-table resolution are **stubbed and port
incrementally**. Even fully stubbed, the schema is a faithful
*skeleton-with-nerves*: the dataflow topology that Odoo actually is lives in the
database on day one.

## Slice 1 — `account.move`

The canonical rich model: 1 647 triples, 610 `@api.depends` edges, dozens of
`_compute_` materialisers, `_check_*` guards raising `ValidationError`,
`line_ids` cross-record deps. If `account.move` lights up faithfully, the
pattern generalises to all 388 object types — it exercises every `DEFINE`
variant at once.

```bash
cargo test  -p od-ontology                              # 9 slice tests, real fixture
cargo run   -p od-ontology --example emit_account_move  # print the DDL
```

## The cut tail (deferred)

A **codegen / migration convenience layer** — export the typed AST, snapshot it
somewhere safe (`schema.surql.{bin,txt}`), shelve it — is explicitly *deferred*.
It is the OpenProject-port shape (`AdaWorldAPI/openproject-nexgen-rs`:
`op-surreal-ast` + `op-codegen-*` + a sea-orm/sqlx target). Useful as a
one-shot migration export; **not** the goal here. The goal is the
ontology-shape: Odoo *running as* a SurrealDB schema.

## Layout

```
odoo-rs/
├── crates/od-ontology/        # corpus → SurrealQL DDL (this crate)
│   ├── src/surreal_ast.rs     #   typed DDL AST + ToSql
│   ├── src/triple.rs          #   SPO corpus loader ({s,p,o,f,c} ndjson)
│   ├── src/emit.rs            #   the corpus → ontology-shape projection
│   └── tests/                 #   account.move slice (real fixture)
└── data/
    └── account_move.spo.ndjson  # 1 647-triple slice of the 22 245 corpus
```

## Provenance

The corpus is produced upstream by the Python frontend
(`AdaWorldAPI/ruff`'s `ruff_python_dto_check` + `tools/odoo-blueprint-extractor`
in `lance-graph`), `expand()`-ed to the `{s,p,o,f,c}` ndjson the SPO store
loads. This repo only *reads* it. License: Apache-2.0.
