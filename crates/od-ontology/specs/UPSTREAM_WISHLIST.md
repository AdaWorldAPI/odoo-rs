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

## 2026-06-18 — CORE-FIRST CORRECTION (read before the resolved-asks list below)

The "RESOLVED via `spo_enrich.py` predicate" annotations below are factually
true about what shipped, but **do not read them as "the way to satisfy this
wishlist is to add another SPO harvest predicate."** That cadence was drift,
reversed upstream by `lance-graph` PR #530 (`E-ODOO-CORE-FIRST-STRUCTURAL`).

The **structural** asks — `target`/`inverse_name`, `inherits_from`,
`selection_value` — are **Core** facts. Their authoritative home is the typed
`OdooEntity` Core in `lance-graph-ontology::odoo_blueprint`
(`OdooField.target` already existed there; `inherits_from` + `selection_value`
now live in `odoo_blueprint::structural`). The `spo_enrich.py` harvest is the
**Extracted-leg breadth feeder** for the ~322 ObjectTypes the curated Core has
not reached — subordinate, not the home (Core wins on convergence).

**Consumer consequence for us:** for structural facts on a *curated* model,
prefer the typed Core (project from `OdooEntity` / `structural`) over the SPO
harvest; consume the SPO corpus for **behavioural** facts (`reads_field` deep
lifts, `emitted_by`, transitive `depends_on`) where it is the genuine source.
Our `RelationMap::from_corpus` / `InheritanceMap::from_corpus` consumers still
work (the harvest projects the same triples), but the source-of-truth ordering
is Core-first.

**`virtually_overrides` (the last open item) is NOT a harvest predicate.** It
is a ClassView/Core MRO capability — do not request it as `spo_enrich`
predicate #6.

## What odoo-rs needs — prioritized

> **2026-06-17 — P0 + P1 RESOLVED upstream.** `lance-graph` PR
> [#523](https://github.com/AdaWorldAPI/lance-graph/pull/523)
> (commit `69a3b0a`) shipped BOTH `target` / `inverse_name`
> sibling triples on relation IRIs (P1) AND deep-`reads_field`
> lifts of `@api.depends('a.b')` leaves (P0). Consumer side:
> `od_ontology::RelationMap::from_corpus(&[Triple])` (~30 LOC,
> 2 tests) now reads them directly. `slice_2.relations.ndjson`
> hand-crafted overrides are obsolete. The recompute-DAG probe
> ratifies the corpus: 332-method / 46-edge `MethodKind::Compute`
> subset on the fresh slice 2 topologically sorts cleanly,
> resolving the audit's MISSED-1 case as a one-directional
> ordering dep (residual-before-amount), not a cycle.
>
> The P2 (`validation_kind`) and P1 (`_inherit` / `inherits_from`)
> asks remain open. The bridge-binary deferral (walk `OdooEntity`
> blueprint → relations.ndjson) is also obsolete — the corpus
> carries the truth.

> **2026-06-17 (later) — P1b + P2 EXTRACTOR RESOLVED, corpus regen
> pending.** lance-graph PR
> [#526](https://github.com/AdaWorldAPI/lance-graph/pull/526) (commit
> `7947487`, merged `01b9509`) extends `spo_enrich.py` with
> **`inherits_from`** (per ruff#19 — `_inherit` mixin + `_inherits`
> delegation; self-inherits + `_inherit`-only classes correctly
> dropped) and **`validation_kind`** (per ruff#21 — five recognised
> kinds detected from `@api.constrains` bodies; three codex P2 fixes
> baked in). 41 python + 11 rust tests green. **All P0 + P1 + P1b +
> P2 EXTRACTOR work is now landed**; only `virtually_overrides`
> (a genuine ClassView design question — not a single-predicate
> emission) and `Selection` enumeration (P3) remain. **Corpus regen
> for #526 still requires a session with `/home/user/odoo/addons` on
> disk** (this host does not carry it); the Rust loader's predicate-
> histogram match arm is forward-compat ready.

> **2026-06-18 — CORPUS REGEN'D + CONSUMED.** lance-graph PR
> [#527](https://github.com/AdaWorldAPI/lance-graph/pull/527) (commit
> `ca810e5`, regen'd on a session with the Odoo source) added 413
> triples (24 166 → 24 579): **166 `inherits_from`** + **247
> `validation_kind`** (presence 108 / range 80 / lookup 31 /
> uniqueness 18 / format 10). Consumer side: shipped
> `od_ontology::InheritanceMap::from_corpus(&[Triple])` (~120 LOC,
> 6 tests) — slice 2 lifts 8 `inherits_from` edges and the
> `account_move` row carries its `mail.thread` base correctly. Slice
> refresh (slice 1: 1825 / slice 2: 3065 triples).
>
> **Consumer FINDING — `validation_kind` describes AST pattern, not
> SurrealQL emit shape.** The corpus classifies
> `account_move._check_invoice_currency_rate` as `range`, NOT the
> `lookup` kind this consumer's spec intuited. The AST classifier
> reading the Python body detects a numeric comparison. The
> semantic shape (related-row-must-exist guard scoped by (currency,
> company, date)) is unchanged at the SurrealQL emit layer; only the
> framing of *why the kind exists* needed correction. Logged in
> `_check_invoice_currency_rate.md` 2026-06-18 entry. **The kind
> predicate is useful for AST-level filtering (find every guard that
> AST-classifies as range), not for direct SurrealQL emit
> dispatch — a consumer that wants per-kind specialization has to
> pair the kind with method-level semantic context.**

### P0 · Cross-method recompute-ordering DAG  ✓ RESOLVED 2026-06-17

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

> **2026-06-17 PROBE FINDING — recompute-DAG cycle detector shipped
> (`od_ontology::RecomputeDag`).** A topological-sort + cycle-detection
> module landed in `od-ontology` (`crates/od-ontology/src/recompute_dag.rs`,
> 100% safe, 8 tests). Built from corpus `reads_field` + `emitted_by`
> triples, with a `MethodKind` filter
> (`Compute`/`Check`/`Onchange`/`Inverse`/`Search`/`Other` by
> underscore-prefix) so the projection-relevant `Compute` subset can be
> queried in isolation.
>
> Two findings against slice 1 / slice 2:
>
> 1. **`MethodKind::Compute` subsets ARE acyclic.** Slice 2 (move +
>    line + currency + partner) topologically sorts cleanly. The audit's
>    MISSED-1 P0 cycle (move._compute_amount via line.reconciled emitted
>    by line._compute_amount_residual) is **STRUCTURALLY INVISIBLE** to
>    the corpus-only DAG because `_compute_amount`'s only `reads_field`
>    triple points at `account_move.line_ids` (the relation), not the
>    transitive `account_move_line.reconciled` read.
> 2. **The full graph DOES carry a real cycle** between two
>    `_onchange_*` methods in slice 1
>    (`_onchange_invoice_vendor_bill` ↔ `_onchange_quick_edit_total_amount`,
>    both emit+read `invoice_line_ids`). This is a **legitimate** Odoo UI
>    cooperative loop — NOT a recompute-DAG bug — and demonstrates why
>    restricting to `MethodKind::Compute` is the correct invariant
>    (cycle on the full graph is a false positive at the projection layer).
>
> **The wishlist's P0 ask, sharpened:** the topological-sort *machinery*
> is shipped corpus-side. What's missing — and what blocks catching the
> audit's MISSED-1 — is **deep-`reads_field` on the EXTRACTOR side**:
> when `@api.depends('line_ids.reconciled')` is declared, the extractor
> should emit BOTH `(_compute_amount, reads_field, account_move.line_ids)`
> (which it already does) AND `(_compute_amount, reads_field,
> account_move_line.reconciled)` (which is the new lift). Same wire shape
> as the existing `reads_field` predicate; same cost as ruff#18's
> sibling-IRI emission. Once landed, this exact `RecomputeDag` catches
> the MISSED-1 case without any further work on this side. **The ask is
> now narrower and concrete: one extractor change, no new
> predicate, no ClassView interface required.**
>

### P1 · `_inherit` (mixin) flattening  ✓ RESOLVED 2026-06-18 (corpus regen'd via lance-graph#527; consumer's `InheritanceMap::from_corpus` lifts 8 edges from slice 2)

Odoo's `_inherit = 'mail.thread'` is mixin composition. The parent's
fields / methods / decorators flatten into the child as if declared
there.

> **2026-06-17 — EXTRACTOR RESOLVED, regen pending.** lance-graph PR
> [#526](https://github.com/AdaWorldAPI/lance-graph/pull/526)
> (commit `7947487`, merged `01b9509`) extends `spo_enrich.py` with
> `inherits_from` extraction — `_inherit` (string or list) emits
> `(odoo:<this>, inherits_from, odoo:<base>)` per ruff#19's
> cross-language wire shape. Self-inherits + `_inherit`-only
> classes correctly dropped at scan time. **Wire ready; corpus regen
> requires a session with `/home/user/odoo/addons` on disk.** Once
> regenerated, the consumer's `od_ontology` can compose a
> `ClassView`-style MRO from the corpus directly.

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

### P1 · `_inherits` (delegation) flattening  ✓ RESOLVED 2026-06-18 (corpus regen'd; same `inherits_from` predicate per ruff#19 convention)

Odoo's `_inherits = {'res.partner': 'partner_id'}` is **delegation**
(distinct from mixin): `self.name` proxies through `self.partner_id.name`.
SurrealDB has no native delegation.

> **2026-06-17 — EXTRACTOR RESOLVED, regen pending.** Same lance-graph
> PR #526 lifts `_inherits` dict keys via the same `inherits_from`
> predicate (flat, not distinguished from `_inherit` mixin at the wire
> level — per ruff#19's "one predicate, both sources" convention).
> The delegation FK itself is already captured via the standard
> `target`/`inverse_name` channel on the relation field
> (`partner_id → res.partner` in the example), so downstream
> `RelationMap::from_corpus` already knows the FK; `inherits_from`
> now also marks the delegation parent for MRO purposes.

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

### P2 · `virtually_overrides` precedence  ✓ RESOLVED 2026-06-18 (computed ClassView relation, NOT a harvest predicate)

When a child model `_inherit`s a parent and overrides a `_compute_*` or
`_check_*` method, the corpus needs to name the precedence so the
projection emits the WINNING body, not both.

- **Consumer use:** the override winner gets the `DEFINE FUNCTION`
  body; the loser is suppressed or annotated as `-- shadowed_by: <id>`.

> **2026-06-18 — RESOLVED the Core-correct way (NOT harvest extension).**
> The earlier "harvest extension would be needed" note was the drift the
> Core-first correction reversed. `virtually_overrides` is **not a
> harvested fact** — it is the **derivation** the ClassView computes from
> the `has_function` + `inherits_from` manifest (the doctrine's
> `(has_function / inherits_from / virtually_overrides)` triad: the first
> two are facts, the third is resolved). lance-graph PR
> [#533](https://github.com/AdaWorldAPI/lance-graph/pull/533) lands it as
> `odoo_blueprint::mro::resolve_overrides` — nearest-base-wins BFS up the
> `_inherit` chain (= Python C3 for the linear mixin chains Odoo uses),
> with `project_virtually_overrides` emitting `(odoo:<child>.<m>,
> virtually_overrides, odoo:<base>.<m>)` for consumers wanting the corpus
> shape. **Consumer consequence for us:** to pick the winning body, build
> the manifest (`has_function` + `inherits_from`, either from the typed
> Core or the SPO corpus) and call the resolver — do NOT request a
> `virtually_overrides` harvest predicate. With this, **every wishlist
> item is home-correct**: P0/P1/P1b/P2/P3 as typed-Core facts +
> Extracted-leg breadth, and this one as a computed ClassView relation.

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

### P1 · FK-target-override as a corpus predicate (RATIFIED cross-language by ruff#18)  ✓ RESOLVED 2026-06-17

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

> **2026-06-17 — IMPLEMENTED.** `RelationMap::from_corpus(&[Triple])`
> shipped (`relations.rs`, ~30 LOC + 2 tests, exactly as predicted).
> Lance-graph PR #523 landed both predicates; fresh slice 2 yields
> `map.target("account_move", "line_ids") == Some("account_move_line")`
> and `map.inverse(...) == Some("move_id")` directly from the
> corpus. The hand-crafted `slice_2.relations.ndjson` is obsolete and
> can be retired once the projection wiring picks up the corpus path.

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

### P2 · `action_*` state-transition predicate (the `EnterEffect` / Rubicon-crossing source)

> Filed 2026-06-22 (behavioral-arm lowering, `corpus_to_actions` / od-ontology PR #9).

`corpus_to_actions` (`ogar_actions.rs`) lowers two of the three behavioral
arms from facts the corpus already carries:

- **Compute** — `_compute_*` + `reads_field` → `ActionDef{ kausal: Depends{paths} }`.
- **Constrains** — a method that `raises` → `ActionDef{ kausal: LifecycleTrigger, guard_failure_policy: Reject }`.

The **third arm — the `action_*` state crossing** (Odoo's
`def action_post(self): ... self.write({'state': 'posted'})`) — has **no corpus
source.** The extractor emits `has_function` / `reads_field` / `emitted_by` /
`depends_on` / `raises` (compute + guard signals) but **nothing that says a
method WRITES a state field to a value.** So the Rubicon crossing — OGAR's
`ActionDef.on_enter = EnterEffect::transition("state", "posted")` + the
`Pending → Committed` FSM — cannot be lowered.

**The ask (same shape ruff#18 ratified — a per-method override lifted into a
keyed sibling SPO triple):** emit a state-write fact for `action_*` methods,
e.g.

```
(odoo:account_move.action_post, transitions_to, "account_move.state:posted")
```

or the decomposed pair `(method, writes_field, "account_move.state")` +
`(method, writes_value, "posted")`.

- **Consumer use:** with the predicate, `corpus_to_actions` lowers each
  `action_*` method → `ActionDef{ on_enter: EnterEffect::transition(field,
  value), default_subject: User }`, completing the behavioral arm so the **full**
  Odoo lifecycle (draft → posted → cancelled), not just reactive compute +
  guards, maps onto OGAR's Rubicon FSM (`ogar_vocab::EnterEffect` / `ActionState`).
- **Status:** **OPEN.** This is a *producer-side fact gap, NOT a Core gap* — the
  Core vocabulary (`EnterEffect`, OGAR #97) already exists and od-ontology already
  pins it; only the corpus predicate is missing. Until it lands, `corpus_to_actions`
  emits the compute + guard arms and skips `action_*` (documented in
  `ogar_actions.rs` module docs). No `EnterEffect` lowering will be written until
  the extractor emits the predicate — the same discipline as the P1 FK-target ask
  above (no synthesis from a missing fact).

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

## PROBE-OGAR-ID-TO-CONCEPT-NAME (P2 · gates the classid→COMMENT name enrichment)

> Filed 2026-06-22 (classid-consume sprint, convergence-architect review R4).
> Concrete pass/fail successor to open-question #2 above. Decoupled from the
> hex-id stamp, which already shipped — see below.

> **✓ REVERSE MAP + NAME-IN-COMMENT SHIPPED 2026-06-22.** OGAR #98 landed
> `canonical_concept_name(u16) -> Option<&'static str>` (round-trip-verified
> over all 39 CODEBOOK entries). `emit_via_ogar_annotated` now consumes it:
> the COMMENT carries the readable concept name + classid —
> `COMMENT 'commercial_document (classid:0x00020202)'` — via
> `ogar_vocab::canonical_concept_name`, never re-derived locally. The APP
> prefix + `(prefix<<16)|concept` composition likewise route through OGAR #97
> (`OdooPort::APP_PREFIX`, `app::render_classid_for`). **Still follow-on:** the
> `Class.canonical_concept`-on-shells fusion that collapses `schema_classids`
> into a derived view over `Class::canonical_id()` — R4 graded that
> WORTH-EXPLORING (it changes the shared lowering path), and it carries the
> `sale.order` lexical-vs-alias asymmetry as its own guard. Not blocking; the
> shipped readable COMMENT already delivers the operator-facing value.
>
> **#99 reframing (SurrealQL-AST-trap governance).** W3.3 ("delete the
> `surreal_ast`/`triple`/`recompute_dag` fork") is **two separable lowerings**,
> not one deletion: (a) structural shape → `ogar_vocab::Class` (DDL stays a
> lossy egress adapter); (b) behavioral lifecycle → OGAR's behavioral arm
> (`ActionDef`/`ActionInvocation`/`KausalSpec`/Rubicon FSM), **not** a
> `DEFINE EVENT` emit (which re-enters the trap). The fork deletes only after
> both; (b) is gated on OGAR's behavioral vocabulary being consumable here (NOT
> verified wired). This probe's COMMENT work is identity-layer (a slice of the
> structural arm) and stays clear of that boundary. **Full spellbook — the
> two-arm frame, one-way-lossy roundtrip, Q1–Q5 mirror, activation triggers:
> `specs/SURREAL-AST-TRAP.md`** (the od-ontology-local mirror of OGAR #99).
> Read it before any W3 fork-deletion work.

**Already shipped, no capability needed:** `emit_via_ogar_annotated`
(`ogar_bridge.rs`) stamps the full APP‖class render id into the
`DEFINE TABLE … COMMENT 'classid:0x00020202'` clause, so the id rides into
SurrealDB's own catalog (queryable via `INFO FOR TABLE`). This needs only the
forward `OdooPort::class_id` (name → id) we already consume.

**The gap this probe captures:** to put the human-readable concept *name*
(`COMMERCIAL_DOCUMENT`) in that COMMENT — and to collapse `schema_classids`
into a derived view over `Class::canonical_id()` by populating
`Class.canonical_concept` at lowering time — odoo-rs needs a **reverse**
`u16 → &'static str` lookup that `OdooPort`/`class_ids` do NOT expose today
(`OdooPort::aliases()` is forward-only `&[(&str, u16)]`).

- **Capability under test:** an OGAR-side `class_ids::name_of(0x0202) ==
  Some("COMMERCIAL_DOCUMENT")` (or equivalent reverse index), derivable from
  the existing `class_ids` const module / `CODEBOOK`.
- **PASS:** `table_to_class` sets `class.canonical_concept` (+ a
  `COMMERCIAL_DOCUMENT (classid:0x00020202)` COMMENT); `schema_classids`
  becomes a one-liner over the populated shells (no second traversal); the
  asymmetry guard holds — `sale.order` resolves to `COMMERCIAL_DOCUMENT`,
  **not** lexical `order` (this is the regression that proves the fusion safe).
- **FAIL / stay deferred:** OGAR exposes no reverse map → keep the forward-only
  `concept_classid` surface and the hex-only COMMENT (the current shipped state).
- **Decoupling note:** the hex-id COMMENT (shipped) has **no** dependency on
  this probe; only the concept-*name* enrichment + the `canonical_concept`
  fusion wait on it. They are the *same* gate (verified: both need the reverse
  lookup), so one OGAR capability unblocks both.

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

### ruff#21 — `validation_kind` is a NEW cross-language predicate (and a NEW wishlist item)  ✓ RESOLVED 2026-06-18 (corpus regen'd via lance-graph#527; 247 kind triples in master, 20 in slice 2; consumer FINDING: kind describes AST pattern, not SurrealQL emit shape — see `_check_invoice_currency_rate.md` 2026-06-18 entry)

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

> **2026-06-17 — EXTRACTOR RESOLVED, regen pending.** lance-graph PR
> [#526](https://github.com/AdaWorldAPI/lance-graph/pull/526)
> (commit `7947487`, merged `01b9509`) extends `spo_enrich.py` with
> `validation_kind` classification. Five recognised kinds:
> `presence` / `uniqueness` / `range` / `format` / `lookup`, detected
> conservatively from `@api.constrains` method bodies. Three codex
> P2 specificity fixes baked in (range skips `search_count(...)` LHS;
> presence skips negated calls; constraint binding follows #525's
> `model_names` for `_inherit`-only classes). **Wire ready; corpus
> regen requires a session with `/home/user/odoo/addons` on disk.**
> Once regenerated, `_check_invoice_currency_rate.md`'s
> `currency_rate_lookup` kind becomes a structural annotation on the
> projection (the spec's `CORE-FIT verdict` block already names this
> as the receiving slot).

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

## 2026-06-30 — recipe-bitmask probe surfaced two crate-path gaps

> Filed by `tests/recipe_redundancy_probe.rs` (OGAR `D-RECIPE-BITMASK` /
> `PROBE-OGAR-AR-RECIPE-COLLAPSE`). The probe measures how much of Odoo's
> lifted behavioural arm collapses to the shared ActiveRecord-lifecycle
> recipe + a per-class override (OGAR = Open Graph **Active Record**, so the
> recipe IS the AR protocol). Two capture gaps cap the measurable collapse.

### A · `ruff_python_spo` (the live-source CRATE path) drops `_inherit`  · P2

The **corpus** (`spo_enrich.py`) already emits `inherits_from` (resolved via
lance-graph #526/#527 — slice_2 carries 8 edges). But the **crate** path that
`od_ontology::compile_source` uses — `ruff_python_spo::extract_from_source` →
`build_graph` — discards `RawClass.inherits` via `..Default::default()`, so a
**live-source** transpile sees no MRO at all. The two Odoo extraction paths
disagree on inheritance.

- **Ask:** lift `RawClass.inherits` into the `Model` (emit `inherits_from`,
  the same wire shape the corpus extractor and `ruff_ruby_spo` already use).
  Low risk — the corpus extractor proves the shape; this just brings the crate
  path to parity.
- **Why it matters for the recipe-bitmask:** inheritance is the biggest
  collapse lever (inherited-default = clear bit). Without it on the live-source
  path, the override-vs-inherit mask can't be computed from source — only the
  shape/payload redundancy can (what the probe measures today, an upper bound).

### B · no method-body hash / decorator-type capture  · P3

The strict "redundant = content-hash-equal-to-default" test (lossless-DO §1)
needs **method-body identity**; today only `@api.depends` args + `reads` /
`raises` facts are captured (and only `@api.depends` among decorators — not
`@api.constrains` / `@api.onchange` / `@api.model` as distinct types). So the
probe can dedup on *shape + dependency-set*, not on *body*. A per-method body
hash (or a fuller decorator set) would let the probe measure TRUE behavioural
dedup, tightening the upper bound toward the real leftover. Optional — body
dedup can only LOWER the measured leftover, so its absence is conservative.
