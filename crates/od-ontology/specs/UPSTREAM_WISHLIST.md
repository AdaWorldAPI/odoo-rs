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

### P1 · FK-target-override as a corpus predicate (RATIFIED cross-language by ruff#18)

**This is the most directly actionable item — it has working cross-language precedent.**

`AdaWorldAPI/ruff#18` (merged 2026-06-17) solved EXACTLY this on the
Rails/OpenProject side. Rails `belongs_to :owner, class_name: 'User'`
declares a relation (`:owner`) whose target class (`User`) does not
follow the camelcase-singular convention on the relation name. ruff#18's
fix: lift the override into a **sibling SPO triple keyed by the relation
IRI**:

```
(openproject:WorkPackage.owner, class_name, "User")
```

> ruff#18's own framing: *"Without surfacing this override, downstream
> Schema consumers invent a phantom `record<Owner>` for what should be
> `record<User>`."*

**This is byte-for-byte our `invoice_line_ids` deferred gap** (documented
in `_compute_amount.md` and the slice-2 typed-lift tests). Odoo's
`invoice_line_ids = fields.One2many('account.move.line', 'move_id')` has
a field name (`invoice_line_ids`) whose `<parent>_<stem>` convention
(`invoice_line`) misses the real target (`account.move.line`). The
heuristic invents `record<invoice_line>` for what should be
`record<account_move_line>` — the identical phantom-target failure ruff
just fixed for Rails.

- **Consumer use:** if the Odoo SPO extractor emits the analog —
  `(odoo:account_move.invoice_line_ids, target, "account.move.line")`
  and `(odoo:account_move.invoice_line_ids, inverse_name, "move_id")` —
  then `od-ontology`'s `RelationMap` populates **directly from the
  corpus**, with no sidecar artifact. The current
  `slice_2.relations.ndjson` (14 hand-crafted rows) becomes obsolete,
  and the deferred `od-ontology-bridge` crate (walk `OdooEntity`
  blueprint → relations.ndjson) is **obviated** — the corpus carries
  the truth.
- **Where this request goes:** the **Odoo SPO extractor**
  (`lance-graph/tools/odoo-blueprint-extractor`), NOT the ClassView
  design session. Listed here because it's the same "what odoo-rs would
  consume" surface and the design sessions overlap. The
  `OdooField.target: Option<&'static str>` slot in the typed blueprint
  (verified present this session) already holds this datum — the
  extractor would emit it as a triple, mirroring how ruff#18 lifted
  `AssocDecl.options` into a triple.
- **Why P1 not P3:** unlike the inheritance items (gated on the POC
  ClassView design), this has a *shipped reference implementation* in a
  sibling repo and a *populated source slot* in the blueprint. It is the
  lowest-risk, highest-certainty corpus enrichment on this list. **If the
  extractor adds one predicate, `od-ontology`'s typed lift goes from
  hand-crafted-14-rows to whole-corpus, for free.**

**`od-ontology`'s side is ready:** `RelationMap` already resolves
`(model, field) → (target, inverse)`; a `RelationMap::from_corpus(&[Triple])`
constructor reading a `target` / `inverse_name` predicate is ~30 LOC and
**will be written the moment the corpus carries the predicate** — not
before (building a reader for absent data is the premature-architecture
trap a `core-gap-auditor` pass already corrected this session).

> **2026-06-17 post-rebase corpus check (FINDING).** Re-checked
> `lance-graph/crates/lance-graph/src/graph/spo/odoo_ontology.spo.ndjson`
> (last touched at `d61be8c`, 22 245 triples) after rebasing past PR
> #519. The corpus emits seven predicates — `depends_on`, `emitted_by`,
> `has_function`, `raises`, `rdf:type`, `reads_field`,
> `traverses_relation` — none of which carry a relation-target / inverse-
> name shape. `traverses_relation` is the wrong direction (it's
> `(method) → (model.field)`, naming which methods walk which fields;
> not `(field) → (target_model)`, the shape ruff#18 ratified). **The
> wishlist's P1 ask remains accurate, unimplemented, and high-value.**
> No `RelationMap::from_corpus` will be written until the extractor
> emits the predicate.

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

## 2026-06-17 update — three ruff PRs ratify predicate names

Three PRs merged on `AdaWorldAPI/ruff` (commits `44959d5` /
`01dfdf4` / `50cca02`) after the wishlist's original drafting. They
firm up predicate names the Odoo extractor should adopt for our P1
and add one new cross-language predicate worth requesting.

### ruff#19 — `inherits_from` is the canonical cross-language predicate

`ruff#19` ("feat(ar-shape): route Rails STI parent to inherits_from")
reuses the existing C++ `inherits_from` predicate for Rails STI. **Wire
shape is identical** across frontends — `(class, inherits_from, <base>)`
— differentiated only by `CppExtracted` vs `OpenProjectExtracted`
provenance. Ratifies that `inherits_from` is the shared
inheritance predicate across the whole ruff stack.

**Effect on this wishlist:** the P1 `_inherit` (mixin) and `_inherits`
(delegation) asks now name a **concrete predicate the corpus already
has wire-precedent for**. The Odoo extractor should emit
`(odoo:<model>, inherits_from, odoo:<base_model>)` for every entry
in `_inherit` — same shape, different `OdooExtracted` provenance.
This is the lowest-risk corpus enrichment after #18's
`target` / `inverse_name` (which itself is still pending — see the
2026-06-17 post-rebase corpus check above).

### ruff#21 — `validation_kind` is a NEW cross-language predicate (and a NEW wishlist item)

`ruff#21` ("feat(ar-shape): emit validation_kind triple per recognised
Rails validation key") adds `Predicate::ValidationKind` (vocab 55 → 56).
Per-attribute typed-constraint shape:

```
(openproject:User.email, validation_kind, "presence")
(openproject:User.email, validation_kind, "uniqueness")
```

The existing `validates_constraint` triple still fires for *every*
declaration (existence-of-validation); the new `validation_kind`
sibling lets downstream Schema consumers lower each kind to the right
SurrealQL clause — `presence → ASSERT $value != NONE`,
`uniqueness → DEFINE INDEX UNIQUE`, `length → ASSERT string::len()`,
`format → ASSERT REGEX`, etc.

**Effect on this wishlist (NEW ASK):** Odoo's `@api.constrains`
methods (and `_check_*` guard methods generally) carry the same
shape — every guard is checking a *kind* of invariant. The
projection's `guard_adapter` route (see
`_check_invoice_currency_rate.md`) lowers each `_check_*` to a
`DEFINE EVENT WHEN/THEN THROW`, today undifferentiated. If the Odoo
extractor emits

```
(odoo:<model>.<method>, validation_kind, "presence")
   for "related-row-must-exist" guards
(odoo:<model>.<method>, validation_kind, "uniqueness")
   for unique-tuple guards (e.g. journal+ref pair)
(odoo:<model>.<method>, validation_kind, "range")
   for numeric range guards (e.g. amount > 0)
(odoo:<model>.<method>, validation_kind, "format")
   for string-format guards (e.g. VAT-ID regex)
(odoo:<model>.<method>, validation_kind, "currency_rate_lookup")
   for the cross-table existence-check shape this spec already names
```

then `_check_invoice_currency_rate.md`'s `kind=currency_rate_lookup`
slot becomes a structural annotation on the projection (not a
spec-side note) — the corpus carries the typed shape and the
projection can specialize the `THROW` message + the `WHEN` filter per
kind. Same shape as ruff#21's `validation_kind`; subject is the
*method* IRI (which on the Odoo side is also where the message
template lives), mirroring how ruff#21's subject is the *attribute*
IRI (which on Rails is where the typed validator lives).

**Where this request goes:** the Odoo SPO extractor
(`lance-graph/tools/odoo-blueprint-extractor`), same site as the P1
`target`/`inverse_name` ask.

**Priority:** **P2.** Lower than `inherits_from` (P1, since
`_inherit` flattening is on the critical path for ~50 derived
models like `l10n_de.account_move`); higher than `Selection`
enumeration (P3). Catches the structural shape that the
`guard_adapter` template currently leaves as a doctrine-note.

### ruff#20 — `has_visibility` predicate; informational for odoo-rs

`ruff#20` adds `has_visibility` (vocab 54 → 55) — the
public/protected/private member access specifier the C++ harvester
was previously dropping. Plus the cv-aware method IRI fix
(`(method, " const")` suffix for const overloads) resolves
`GAP-CONST-OVERLOAD`.

**Effect on this wishlist:** **informational only.** Python (Odoo's
host language) is duck-typed; the `_underscore` and `__dunder`
conventions communicate intent but aren't an OO API-surface signal
the way C++ public/protected/private is. **Not requested for the Odoo
extractor.** The C++-only `is_const` sort key + reassemble()'s
67/67 byte-exact round-trip are noted for the substrate-comparison
section below.

### ruff_cpp_codegen — architectural prior art for od-codegen

`ruff#20` also lands `ruff_cpp_codegen` (new crate, depends on
`ruff_spo_triplet` only; no lance-graph edge): `project` → `MethodSig`
manifest, `render` → Rust source naming `lance_graph_contract::codegen_
manifest::MethodSig`. Plus `CPP-CODEGEN-RT` falsifier: 67 classes /
857 methods → 124 KB MethodSig manifest, signature-plane round-trip
holds.

**Effect on this wishlist:** **architectural prior art for `od-codegen`.**
Same C-FIRST pattern: ruff_spo_triplet → ruff_cpp_codegen mirrors the
Odoo-side ruff → od-codegen. The decompile-vs-expand round-trip
(`CPP-REASSEMBLE-RT`: 67/67 byte-exact in #20) is the kind of probe
the wishlist's deferred `--validate` slot would carry for the SurrealQL
side: emit-then-reparse should round-trip the projection's intent.
**Not a request on the design session** — listed so a future
od-codegen contributor knows the C++ side has a working pattern to
copy from.
