# Odoo Codegraph × Domain Harvest v1: evidence-rich view, DT64 boundary

**Status: PROPOSED SCHEMA / DOCUMENTATION ONLY (2026-10-27).** No extractor, runtime, native DTO store, physical DR64³ layout, permission decision, schema migration or executable method lowering is implemented or authorized by this PR.

## Why here

Grok's generic tree-sitter Codegraph (files/classes/methods/imports/calls) and Odoo declarative ORM harvest (models/fields/relations/dependencies) belong in **one optional exchange view with distinct graph partitions**, not in a new competing ontology. `odoo-rs` already consumes Ruff SPO via `od-ontology` and compiles Python source to OGAR V3 `CompiledClass` via `compile_source`; see [odoo-rs #36](https://github.com/AdaWorldAPI/odoo-rs/pull/36). Ruff owns source harvesting and the *closed* predicate vocabulary; OGAR owns DO authority; Quack/RelAlg/R2IL/mask-risc/ndarray own their existing execution responsibilities.

**Files in this PR:**
- `schemas/odoo-codegraph-harvest-v1.schema.json`: optional JSON Schema (Draft 2020-12) for graph + source evidence.
- `docs/examples/odoo-codegraph-harvest-v1.example.json`: explicitly illustrative, incomplete account.move multi-emitter projection.

Existing input remains `data/account_move.spo.ndjson` (`{s,p,o,f,c}`) and `data/odoo_inheritance_manifest.ndjson`. This PR does **not** replace them, reinterpret their numeric truth bands as authorization, add Ruff predicates, or change `od-ontology::parse_ndjson`.

## Proposed mapping

| Layer / graph | Data in view | Existing owner | Not implied |
| --- | --- | --- | --- |
| `structural` | source files, classes, functions, locations, imports/calls | Ruff Python SPO; generic Python arm [#95](https://github.com/AdaWorldAPI/ruff/pull/95) | Call resolution completeness / execution purity |
| `odoo-declared` | model/field/method declarations, relation targets/inverses, `depends_on`, `emitted_by` | Ruff Odoo harvest → od-ontology; [Ruff #127](https://github.com/AdaWorldAPI/ruff/pull/127) role census | Effective installed-module behavior / permissions |
| `odoo-effective` | resolved fields/methods **only after an actual pinned registry observation** | Odoo instance/source-specific attestor (future gate) | Static inference passing as observed registry truth |
| `documentation` | narrative links, never guards | doc harvester | Semantic source authority |
| `dt64_candidates` | qualified **candidate** associations to existing typed algebra references | DT64/OGAR review boundary | Seat mint, compiled recipe reuse permission, executable code |

`graph` is a partition label, **not** authorization, completeness, provenance or a claim that lance-graph already exposes this JSON as a public ingest API. SPOG projection is optional. Named-graph identity must be preserved separately from snapshot witness.

### Identity and inheritance are not simple name edges

- `_name` creates/names a model; `_inherit` without `_name` extends that model in-place; `_name` plus `_inherit` defines classical inheritance; `_inherits` delegates **fields, not inherited Python methods**. Distinct *declaration identities* and source pins are required even if many declarations extend one effective model. Do not emit a tautological `model:sale.order -[inherits]-> model:sale.order` in lieu of the extension declaration.
- `computed`, `related`, `inverse`, `search`, `store`, `compute_sudo` are characteristics of fields; they are **not** interchangeable base field types. `readonly` does not prove server-side denial; UI `domain` does not attest ACL/record rules.
- A source symbol ID is scoped by `metadata.revision`; source span is a **locator**, not durable identity. Do not use mutable `path#startLine` alone as a global canonical ID. Effective instances require a registry/addon fingerprint.
- Nodes/edges carry `evidence` independently from source `{f,c}` weights. A harvested edge and a witnessed runtime effect are different claims. Missing `calls`, triggers, ACL or body writes means `UNKNOWN`, not a proof of absence.
- The schema can check **shape**, not that edges' endpoints resolve, that lines belong to the pinned blob, that a snapshot is genuinely attested or that a source access check succeeded. These require separate validators/oracles.

### Multiple emitters and declared compute ordering

The current `RecomputeDag::from_triples` uses a `field → single emitter` map; the independently measured composition in [odoo-rs #44](https://github.com/AdaWorldAPI/odoo-rs/pull/44) found **26 missing edges in slice 1** and **37 in slice 2** because distinct methods emit the same field. Preserve all `emitted_by` edges and their MethodIds. Do not silently last-write-wins or treat override history and same-version coexistence as identical. `depends_on` records a declared field-level dependency; it is **not** proof of body purity or of `onchange`/constraint trigger completeness.

[odoo-rs #45](https://github.com/AdaWorldAPI/odoo-rs/pull/45) merged a **test-only** Thought-Git counterfactual probe. It does not authorize production sibling synthesis or turn unknown guards into true guards.

## Proposed minimal validation gates (not run by this PR)

1. **G0 Schema/shape**: validate example against JSON Schema 2020-12, ensure all edge endpoints are declared or typed external refs, ensure only the two *distinct* `currency_id` emitter edges survive and roundtrip; seed wrong endpoint / dropped emitter mutation.
2. **G1 Independent source parity**: pin corpus hashes and separately compare source-derived Odoo fields/methods/depends/relation inverses with the Ruff triples without adding predicates. Explicitly record coverage and missing `onchange`, `constrains`, `calls`, body writes.
3. **G2 Effective registry**: only if a real, isolated Odoo instance is available, attest module order, `_inherit` extension, `_inherits` delegation, effective field overrides and method identity under an immutable module-set/registry snapshot. Refuse pretending a source-only fixture is a registry observation.
4. **G3 DT64/Quack handoff**: source-qualified typed candidate → witnessed borrowed binding → independently approved algebra; preserve METHOD multiplicity, SQL K3 unknownness, owner/permission checks and source-watch freshness. No physical ON×DO 256² or DR64³ decision here.

**Concrete falsifier:** for the pinned `account.move.currency_id` fixture, dropping either `_compute_currency_id` or `_onchange_invoice_vendor_bill` from `emitted_by` must fail independently of JSON syntactic validity. Separately, any renderer claiming an effective model without a real pinned registry observation must fail attestation.

## Existing plans and scope firewalls

- [lance-graph #1814](https://github.com/AdaWorldAPI/lance-graph/pull/1814): source-harvested ON/WHEN/DO incidence and refusal gates (docs-only plan).
- [lance-graph #1819](https://github.com/AdaWorldAPI/lance-graph/pull/1819): proposed factored ON×DO bitmap + **source-witnessed Quack WHERE**, not an admitted registry or runtime. #1819 currently contains stale wording calling odoo-rs #45 OPEN; #45 is now MERGED, but remains test-only.
- [lance-graph #1825](https://github.com/AdaWorldAPI/lance-graph/pull/1825): independent EAW-5 Odoo/legal epistemic reality checks (draft).
- **R2IL correction:** `R2IL` in this codebase is not an RDF triple or SPOG export format. It is a project-specific lowering/execution component. Grok's generic radare2 expansion must not override local architectural ownership.
- Closed, operator-rejected lance-graph **#1812 is quarantined** and explicitly excluded from evidence, schemas and authoritative plans.

## Explicit non-goals and release gate

No new canonical ontology, no changed Ruff predicate set, no database writer, no AST compiler, no source writes, no live Odoo connectivity, no permission bypass, no source simulation passing as observation. To progress, a *separate* executable PR must add schema validation/parity tests with negative fixtures and source-attested witnesses, then receive operator review.

**Validation performed for this PR:** JSON syntax for the schema and example is checked while generating these two files. No JSON Schema validator, Cargo suite or Odoo runtime probe is claimed.
