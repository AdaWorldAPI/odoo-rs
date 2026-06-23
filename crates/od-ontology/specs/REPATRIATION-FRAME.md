# Empowerment vs muscle memory — the lance-graph → (OGAR + odoo-rs + ruff) repatriation

> **Filed 2026-06-22 (operator framing).** This is the north star for the
> multi-PR repatriation of Odoo-specific content currently misplaced in
> `lance-graph`. The fix is not a code move; it is an **altitude correction**.
>
> **Updated 2026-06-23** with citations to the parallel OGAR + lance-graph canon
> that shipped the **same framing** in canonical form (OGAR #104, #105, #106,
> #107, #108, #109, #110; lance-graph #591, #592). See § "Canonical references"
> below — the compiler-AST frame this doc names is already canonical upstream;
> the repatriation work and the cross-validation findings that surfaced from it
> remain unique to this consumer side.
>
> Read this BEFORE proposing any "move the X file from lance-graph to Y" PR.

## Canonical references — the parallel work in OGAR + lance-graph

The compiler-AST framing this doc names was independently shipped in canonical
form upstream **before** the repatriation PRs landed. odoo-rs PRs #12-#15
re-derive these framings; this section closes the cite-gap so a future session
reaches for the canon first, not for re-derivation.

| Upstream PR | What it ships | Status this doc aligns to |
|---|---|---|
| **OGAR #104** | `docs/OGAR-AS-IR.md` — the one-page mapping of compiler-phase → OGAR / lance-graph piece (front-end / IR / symbol table / linker / public ABI / semantic-analysis / optimization passes / codegen / native-codegen-via-jitson / runtime). Six tests for IR-design proposals. | **Canonical** — the compiler-AST frame this doc uses in §"The frame correction" is the same lens, in the same vocabulary. odoo-rs PR #13's compiler-AST frame in `triple.rs` was an independent re-derivation. |
| **OGAR #109** | EPIPHANY: *"OGIT was already a semantic compiler's symbol table"* — graded-fences assessment of bardioc's prior art on the OGIT artifact (NTO + SGO + MARS XSD + extract_classes.py). 3/6 IR-shape tests passed by OGIT alone; the **unification of structural + behavioural arms is OGAR's contribution**, not bardioc's. | **Canonical** — the "symbol-table" framing odoo-rs PR #14 used for the alignment table is the same lens this entry names for OGIT. |
| **OGAR #105** | OGIT mirrored **1:1 at pinned SHA** at `vocab/imports/ogit/` (9.3 MB, 2010 files, byte-identical to upstream). `ogar-from-schema` producer: line-oriented TTL parser, **bijective reverse-emit** (semantic round-trip), SGO upper-ontology with 176 verb-decls. MARS XSD as closed-formal calibration oracle. Three mechanically-enforced bijection levels. | **Canonical TTL-import infrastructure** — the SPO predicate schema, edge whitelist, and TTL-source list pulled into `od_ontology::triple` (#13) and `od_ontology::alignment` (#14, #15) are bijection-eligible inputs to `ogar-from-schema::ttl`. |
| **OGAR #106** | 9-domain round-trip test + XSD transcode (drops Python from the oracle) + **`docs/ODOO-DIGEST-TO-OGIT.md`** Foundry-parity collapse table + verb-as-class-as-askama-template framing. | **Canonical Odoo-digest pipeline** — the Foundry-parity collapse (Ingest / Storage / Render / IAM+audit) is the upstream version of the "ODOO teaches us to become better than Palantir Foundry" frame this doc names. |
| **OGAR #107** | **Three corrections** to the #106 framing — **including the one this doc had wrong:** the producer is **`ruff_python_spo + ogar-from-ruff`** (existing crates), NOT a fictional `ogar-from-python` / `ogar-from-odoo`. `lance-graph-arm-discovery` is an Association Rule Mining engine, not a producer. Digests belong in `vocab/exports/`, not `imports/`. | **CORRECTION absorbed below** in § "Producer naming — corrected per OGAR #107". |
| **OGAR #108** | `vocab/exports/` is a **staging tier**, not permanent: producer → `exports/` → promote to OGIT fork → re-vendor to `imports/` → consumers read `imports/`. (Walks back #107's "migrate the 11 Accounting files" task once the false migration-risk premise was verified.) | **Canonical storage-tier model** — the staging-tier flow is the lifecycle for any future TTL the per-app consumer wants to promote to OGIT. |
| **OGAR #110** | Mints `0x0B` AuthStore class family — keystone §7 and the canonical OGIT Auth shape (`NTO/Auth/Configuration`) **converge 1:1**. "Reserving costs nothing"; enforcement stays gated on `PROBE-OGAR-RBAC-AUTHORIZE`. | **Pattern for future Odoo mints** — convergence-shaped class-id mints (e.g. the 11 missing `OdooPort` aliases the cross-axis check in #14 surfaced) follow this template. |
| **lance-graph #591** | OGAR consumer pre-flight spellbook — consumer-side mirror of OGAR #99. Five 90-second questions; the three-move remediation recipe. | **Canonical consumer governance** — odoo-rs is now a consumer of the OGAR Core; this spellbook applies. |
| **lance-graph #592** | `contract::ogar_codebook` mirrors OGAR #97's APP-prefix layer. Pinned allocation: **Odoo `0x0002`**. Wire-compatible mirror; drift-test guarded. | **Canonical APP-prefix** — `ODOO_BUNDLE_ID.graph = 0x0002` constant landed in odoo-rs #15 is wire-compat-correct against this mirror. |
| **q2 #42** | Bumped q2's stale OGAR pin (`b6a12a6` → `302c284`) so q2's Railway build sees `OdooPort/SmbPort/WoaPort`. | **Cross-repo coordination evidence** — the `OdooPort` consumer surface this doc names is load-bearing across q2 / lance-graph / odoo-rs; stale pins propagate breakage. Pin discipline is part of the repatriation lifecycle. |

## Producer naming — corrected per OGAR #107

A previous version of this doc named **`ogar-from-odoo`** as the eventual
producer crate. **That naming was wrong** — OGAR #107 corrected the parallel
framing in `docs/ODOO-DIGEST-TO-OGIT.md`. The canonical producer chain is:

```text
odoo/addons/<module>/models/*.py
        │
        ▼
  ruff_python_spo  ──→  ruff_spo_triplet::Model
   (Python AST frontend,                  │
    sibling of                            ▼
    ruff_ruby_spo)              ogar_from_ruff::lift_model_graph
                                          │
                                          ▼
                                 Vec<ogar_vocab::Class>
```

`ogar-from-ruff` already ships for Ruby (Rails); the missing piece for Odoo is
the **`ruff_python_spo`** frontend. Same correction for medcare-rs:
`ruff_rust_spo + ogar-from-ruff`, NOT a fictional `ogar-from-rust`.

**Throughout this doc, "ogar-from-odoo" should be read as `ruff_python_spo +
ogar-from-ruff`.** The naming is preserved in historical-context references
below for traceability against PR #12 as merged.

## What the parallel canon means for this doc's Phase ordering

The phase ordering ("pull → separate → push to OGAR → push to ruff →
subtract") remains correct. The upstream canon refines two phases:

- **Phase 3 (push empowerments to OGAR)** — the destination is OGAR's existing
  `ogar-vocab` + `ogar-from-ruff` machinery, not a new `ogar-from-odoo` crate.
  The 11 missing `OdooPort` aliases the cross-axis check in #14 surfaced are
  the immediate Phase-3 work; the empowerment shapes (`OwlPivot`,
  family-default-style → `StyleCluster` inheritance, suffix-classifier algorithm)
  land as universal `Class` capabilities per OGAR #104's IR-design tests.
- **Phase 4 (push universal predicates to ruff)** — this IS the
  `ruff_python_spo` frontend per OGAR #107. It is the **keystone** for
  shrinking every consumer crate: with it in place, every per-language
  `ogar-from-<lang>` becomes a thin lifter (the way `ogar-from-rails` is
  986 LOC because `ruff_ruby_spo` does the heavy lift).

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
re-implementation). **CORRECTED per OGAR #107:** the canonical crate is not
a future `ogar-from-odoo`; it is the **existing `ogar-from-ruff` consuming
`ruff_python_spo`** — see § "Producer naming — corrected per OGAR #107"
above. The principle remains: with `ruff_python_spo` in place, every per-app
crate shrinks to a thin lifter.

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
