# Empowerment vs muscle memory — the lance-graph → (OGAR + odoo-rs + ruff) repatriation

> **Filed 2026-06-22 (operator framing).** This is the north star for the
> multi-PR repatriation of Odoo-specific content currently misplaced in
> `lance-graph`. The fix is not a code move; it is an **altitude correction**.
>
> Read this BEFORE proposing any "move the X file from lance-graph to Y" PR.

## The mis-altitude (what's wrong today)

`lance-graph` is supposed to be the **generic graph spine** — query engine,
codec stack, semantic transformer. Today it hosts **~14 000+ LOC of
Odoo-specific code** scattered across four crates:

```
lance-graph/.../graph/spo/odoo_ontology.rs            509 LOC + 24 579-line .ndjson
lance-graph-ontology/.../odoo_blueprint/{l1..l15…}  14 384 LOC across 22 files
lance-graph-ontology/.../hydrators/{odoo,dolce_odoo}.rs  325 LOC
lance-graph-callcenter/.../odoo_alignment.rs          704 LOC
```

This is the same Core-First violation as the deprecated bridges (#589), one
layer deeper — at the *domain-knowledge* level. lance-graph carries the
muscle memory of one app; the spine should know no apps.

## The frame correction (the operator's words)

> *"ODOO teaches us to become better than Palantir Foundry, but they need to
> be reusable and the shapes need to be universal and the fact that it is
> odoo-rs lives in classes and codebooks and odoo-rs."*

Three altitudes, three homes:

| Altitude | Home | What lives there |
|---|---|---|
| **Identity** | **OGAR Class + codebook** | `OdooPort::class_id("account.move") → 0x0202 COMMERCIAL_DOCUMENT`; the *name* of an Odoo concept. Already canonical (OGAR #93/#94/#95/#97/#98). |
| **Empowerment** (reusable idea factories) | **OGAR (universal class shapes)** | The *patterns* Odoo taught us — alignment via `owl:equivalentClass`, predicate vocabulary for ORM lifecycle, MRO/inheritance, the SPO→`ActionDef` lowering. Parameterised so any consumer reuses them. |
| **Muscle memory** | **odoo-rs (the Odoo glue)** | The specifics — the .ndjson, the l1..l15 blueprints, the alignment-table content, the Odoo namespace strings. |
| **Generic spine** | **lance-graph** | What's *left over* after identity, empowerment, and muscle memory are repatriated — i.e. the graph engine with **zero Odoo**. |

The "vastness in simplicity": once identity sits in OGAR codebook + empowerments
sit in OGAR universal shapes, the 14K LOC of l1..l15 blueprints collapses to
**OGAR `Class` definitions × empowerment instantiation**. The same job, a
fraction of the LOC — because the shapes are doing the work instead of the
copy-pasted Odoo specifics.

## Inventory re-sorted per *shape*, not per file

| Lance-graph file | What it actually IS | Re-homes to |
|---|---|---|
| `odoo_ontology.rs` predicate schema (`emitted_by`, `depends_on`, `traverses_relation`, `validation_kind`, …) | **Empowerment** — a universal SPO predicate vocabulary every ORM/lifecycle producer wants (Rails, Elixir, Odoo, Django, …) | **OGAR** (canonical SPO predicates) + **ruff** (emit these natively → "OGAR class detection in ruff") |
| `odoo_ontology.spo.ndjson` (24 579 lines) | **Muscle memory** — pure Odoo corpus data | **odoo-rs** (already has it under `data/`) |
| `odoo_blueprint/{l1..l15}.rs` (14 384 LOC) | **Identities pretending to be code** | **OGAR `Class` definitions** (identity is the codebook); vastness collapses because `Class` + empowerment = the same job |
| `odoo_alignment.rs` `resolve_odoo_to_family` table (704 LOC) | **Empowerment** (alignment-via-`owl:equivalentClass` is universal) + **muscle memory** (the per-class table content) | Empowerment → **OGAR** (`Class` carries `owl:equivalentClass`); content → **odoo-rs** |
| `hydrators/odoo.rs` + `dolce_odoo.rs` (325 LOC) | **Empowerment** (generic TTL hydration + DOLCE classification) + **muscle memory** (Odoo namespace strings) | Empowerment → **lance-graph** (stays, becomes generic); strip the Odoo specifics out |
| `bridges/odoo_bridge.rs` | Already `#[deprecated]` (#589) → consumers pull via `OdooPort::class_id` | Delete on the next bridge-deletion PR (gated on consumer migrations, not this work) |

## Ruff implication — the keystone for shrinking everything

> *"ruff might need an ogar class detection"*

If the harvester emits OGAR-class-shaped output natively — predicates that
already speak OGAR vocabulary, not raw `odoo:`-namespaced SPO that needs
lifting — then **every** `ogar-from-<lang>` shrinks to a thin pass-through
(`ogar-from-rails` is 986 LOC because `ruff_ruby_spo` does the heavy lift;
`ogar-from-odoo`, when it exists, should be the same shape, not a 14K-LOC
re-implementation).

This is its own ask, filed against `ruff`:

> **`ogar-class-detection` in ruff.** Add `ruff_python_spo` (Python frontend
> mirroring `ruff_ruby_spo`) and a canonical OGAR-predicate vocabulary so the
> harvested SPO already speaks `(class, has_function, fn)` /
> `(field, emitted_by, fn)` / `(class, owl:equivalentClass, …)` instead of an
> Odoo-private namespace. Without this, the empowerment repatriation still
> works but every per-app crate carries its own lifter; with it, the lifters
> become uniform.

## The pragmatic ordering (operator-named)

> *"it's probably cleaner to move the migrated to odoo-rs because it's
> impossible to do it at once and the fact that the arm crate for codegen
> lives in lance-graph was probably the main reason"*

So the sequencing is **not** "move 14K LOC to OGAR." It is:

1. **Pull Odoo-specific content from lance-graph into odoo-rs**, where it
   belongs as muscle memory. Bounded, reversible, no architectural commitment
   yet about what's "empowerment" vs "muscle memory."
2. **In odoo-rs, separate** the universal patterns (empowerments) from the
   Odoo-specifics (muscle memory). The separation is easier *after* the
   content sits with its consumer.
3. **Push the separated empowerments to OGAR** as universal `Class` shapes /
   predicate vocabularies / alignment mechanisms (parameterised, reusable).
4. **Push universal predicates to ruff** so the harvester emits OGAR-shaped
   output natively (shrinks every consumer crate).
5. **lance-graph becomes generic by subtraction**, not by an explicit "move."
   The end state: zero Odoo in the spine.

Per-phase PRs are bounded; the whole effort is **not** one PR.

## What this doc is NOT

- **Not a 14K-LOC migration plan.** The next session reading this should pick
  ONE file from the inventory above and pull it (phase 1), not all of them.
- **Not a code review of lance-graph.** lance-graph today is wrong-altitude,
  not wrong-code. The 14K LOC works; it's just not where it should live.
- **Not a blocker on the keystone or the producer predicate.** This is its own
  axis: a repatriation of identity / empowerment / muscle memory. It runs in
  parallel with the keystone (W3.4) and the `EnterEffect` producer ask (#11).

## Cross-refs

- OGAR #93/#94 (`OdooPort` — identity already canonical), #99
  (`SURREAL-AST-TRAP-PREFLIGHT.md` — the spine doctrine), #100 (the
  classid-address pin), #102 (the spine-vs-membrane split).
- lance-graph #589 (the deprecation beacon — same pattern, one layer up),
  #591 (the consumer spellbook), #592 (the contract codebook mirror).
- odoo-rs #6 (local `SURREAL-AST-TRAP.md` spellbook), #7 (W3 behavioral-arm
  scope), #8 (classid-address doctrine), #11 (the `EnterEffect` producer ask).
