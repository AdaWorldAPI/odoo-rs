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
`line_ids` cross-record deps. Exercises every `DEFINE` variant at once.

```bash
cargo test  -p od-ontology                              # 9 slice-1 + 11 slice-2 tests
cargo run   -p od-ontology --example emit_account_move  # print the DDL
```

## Slice 2 — `account.move` + `account.move.line` + `res.partner` + `res.company`

The smallest meaningful expansion. **2 739 triples across 4 models** surface
three shapes single-model focus can't:

1. **Back-ref resolution within the focus set.** `account_move.line_ids` lowers
   to a `DEFINE EVENT` on `account_move_line` (the resolved child table) via
   the `<parent>_<stem>` Odoo convention, not the slice-1 placeholder
   `account_move__line_ids`. The event fires `LET $parent = $after.move_id;
   UPDATE $parent SET …` — the One2many inverse becomes real.
2. **`res_*` namespace resolution.** `account_move.partner_id` lowers to
   `option<record<res_partner>>` via the `res_<stem>` convention; same for
   `company_id → res_company`.
3. **Honest unresolved audits.** Relations whose targets aren't in the focus
   set (e.g. `journal_id`, `commercial_partner_id`, `invoice_line_ids`'s
   semantic-but-misnamed target) emit `(child UNRESOLVED — not in focus set)`
   notes and a `/* TODO: resolve child→parent back-ref */` on the `THEN`
   clause — never silent fallthroughs.

```bash
cargo run -p od-ontology --example emit_slice_2
# -- summary: 4 tables, 369 fields, 401 functions (deferred bodies),
#    64 events (reactive + guards), 24 unresolved-child audit notes —
#    from 2 739 triples
```

### Typed-lift bridge — `RelationMap`

The relation-target resolver tries the typed `OdooField.target` truth (a
`RelationMap`) **first**; the heuristic ladder is the fallback. This closes the
deferred gap from slice 2:

```bash
cargo test -p od-ontology --test slice_2_typed_lift
# 7 tests demonstrating the lift's resolutions vs the heuristic's misses
```

The map ships as an ndjson artifact (`data/slice_2.relations.ndjson`) mirroring
how the SPO corpus already works:

```text
{"model":"account_move","field":"invoice_line_ids","target":"account_move_line","inverse":"move_id"}
{"model":"account_move","field":"bank_partner_id","target":"res_partner","inverse":null}
```

`RelationMap::from_ndjson(&str)` loads it. The future `od-ontology-bridge`
binary extracts the full map from
`lance-graph-ontology::odoo_blueprint::ENTITIES` (each
`OdooField { kind: Many2one|One2many|Many2many, target: Some(t), … }` produces
one row), keeping `od-ontology` itself zero-dep on the heavy ontology stack.

Cross-record events carry a provenance audit in their note —
`(child=account_move_line via typed-lift, …)` vs `(via convention, …)` — so a
reader can see at a glance which relations are grounded by truth vs guess.

### Naming-convention ladder (fallback when the map misses)

The relation-target resolver tries, in order:

| # | Convention | Example (focus={account_move, account_move_line, res_partner, res_company}) |
|---|---|---|
| 1 | exact: stem itself is a focus model | `move_id` → ❌ (no model named `move`) |
| 2 | `res_<stem>` (Odoo `res.*` master data) | `partner_id` → ✅ `res_partner` |
| 3 | `<parent>_<stem>` (parent-suffixed children) | `account_move.line_ids` → ✅ `account_move_line` |
| — | miss → bare stem with inline audit | `journal_id` → `record<journal>` + UNRESOLVED note |

The exceptions Odoo hand-wires (`account.move.line.move_id` is the One2many
inverse for both `line_ids` AND `invoice_line_ids`; `invoice_line_ids`'s first
positional arg is `'account.move.line'` not `'account.move.invoice_line'`) are
exactly what the typed-lift bridge above carries. `slice_2.rs` pins the
honest *convention* fallback (`record<invoice_line>`); the parallel
`slice_2_typed_lift.rs` proves the *lift* resolves it to `account_move_line`.
Both stay in the test set — the diff between them is the value the bridge adds.

## Pulling the canonical classid (OGAR)

Under the `ogar-emit` feature, `od-ontology` pulls the canonical OGAR classid
for each Odoo model straight from `ogar_vocab::ports::OdooPort` — no bridge
object, no registry, no TTL hydration. A pure static lookup over the shared
codebook (OGAR #94). This is the "pull OGAR via class" consumer-migration target
(lance-graph #589).

### API

```rust
// feature = "ogar-emit"
concept_classid("account_move")          // Some(0x0202)  COMMERCIAL_DOCUMENT
concept_classid("account_analytic_line") // Some(0x0103)  BILLABLE_WORK_ENTRY
render_classid("account_move")           // Some(0x0202_0002)  APP 0x0002 (Odoo lens) ‖ concept
schema_classids(&schema)                 // Vec<(table, Option<u16>)>
```

The APP prefix (`OdooPort::APP_PREFIX`, OGAR #97) and the `(prefix << 16) |
concept` composition (`ogar_vocab::app::render_classid_for`) come from the Core
— never a local literal or a hand-rolled shift.

### Annotated DDL — classid in the catalog

`emit_via_ogar_annotated(&schema)` lowers each table onto `ogar_vocab::Class`
and emits via the canonical `ogar-adapter-surrealql`, stamping each codebook
table's **concept name + classid** into the `DEFINE TABLE … COMMENT` clause —
so it rides into SurrealDB's own catalog (queryable via `INFO FOR TABLE`),
human-readable, not just the `.surql` text:

```surql
DEFINE TABLE account_move SCHEMAFULL COMMENT 'commercial_document (classid:0x02020002)';
```

The concept name comes from `ogar_vocab::canonical_concept_name` (OGAR #98's
`id → name` reverse map), never re-derived locally. The COMMENT carries
**identity only** — never lifecycle/behavior (per OGAR's SurrealQL-AST-trap
governance, #99: SurrealQL is an adapter, not a spine).

### APP‖class codebook

The `classid` is a 32-bit value: `APP (high u16) ‖ concept (low u16)`. The **low
u16** is WHAT it is — the shared cross-app concept (RBAC + ontology). The **high
u16** is WHOSE render — the per-app lens; Odoo's is `0x0002`.

### Convergence pin

Planning times align with billable hours through one codebook lookup:

| Surface | Model / entity | Concept classid |
|---|---|---|
| Odoo (ERP) | `account.analytic.line` | `0x0103` `BILLABLE_WORK_ENTRY` |
| WoA / SMB | `Stundenzettel` | `0x0103` `BILLABLE_WORK_ENTRY` |
| OpenProject / Redmine | `TimeEntry` | `0x0103` `BILLABLE_WORK_ENTRY` |

### Name-form caveat

The SPO corpus names models in table form (`account_move`); `OdooPort` aliases
are model form (`account.move`); the bridge normalizes `_`→`.` (the canonical
Odoo `_name.replace('.', '_')` inverted). This is lossless for the codebook's
dot-separated single-word segments; Odoo localization and module classes with an
underscore inside a segment (`l10n_*`, `im_livechat_*`) are intentionally out of
scope and resolve to `None` — a fail-safe miss, never a wrong id.

---

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
│   ├── examples/              #   emit_account_move, emit_slice_2
│   └── tests/                 #   slice-1 + slice-2 against real fixtures
└── data/
    ├── account_move.spo.ndjson  # 1 647-triple slice-1 fixture
    └── slice_2.spo.ndjson       # 2 739-triple slice-2 fixture (4 models)
```

## Provenance

The corpus is produced upstream by the Python frontend
(`AdaWorldAPI/ruff`'s `ruff_python_dto_check` + `tools/odoo-blueprint-extractor`
in `lance-graph`), `expand()`-ed to the `{s,p,o,f,c}` ndjson the SPO store
loads. This repo only *reads* it. License: Apache-2.0.
