# D-BEHAVIOR-EQSAT-0 — Gate A, behavioral non-injectivity

**Status:** Gate A test-only candidate, **UNVERIFIED until CI runs**. 2026-10-30. No changes to production code, data, source vocabularies, Odoo registry, DR/DT, Ruff, OGAR, KinaXe, egglog or MLIR. No effective Odoo installation or execution trace is claimed.

## Source / revision pins

- odoo-rs #44 merged `a59f9e8be01ecf8e0342afa3b84ed989bd5de2fe`; #45 merged `7737a129491a289efde339e3e719539ec662b9b0`; #46 merged `558f12ac65c6c1c4667e1eb28f34c6a4f9c5647b`.
- `data/account_move.spo.ndjson` blob `459659e1c1ece2303de2bc3c8eab715c27e6c53d`.
- `data/odoo_inheritance_manifest.ndjson` blob `becf51e7f73faff45da3f7a17f518048eac0d942`.
- Existing `docs/examples/odoo-codegraph-harvest-v1.example.json` is illustrative **source only**; its `metadata.revision` is #45's merged commit. It is not an Odoo runtime export.
- Ruff #125 (`ruff_metta_spo`) and #127 (`Predicate::role`) merged. No Ruff vocabulary changes here.
- lance-graph #1849/#1850–#1853, ndarray #400, OGAR #357/#363 and KinaXe #21/#22 are reference owners, not modified dependencies beyond existing test-only `dr-math-binding-probe`.

## Reused surfaces and result contract

| Existing surface | What is checked | What is NOT implied |
|---|---|---|
| `od_ontology::parse_ndjson` | Original `emitted_by` and `reads_field` triples | Complete method bodies |
| #46 source example | Two separate `account_move.currency_id` emitter edges | Effective Odoo registry observation |
| #44 `RecomputeDag` | Last-emitter-only ordering subset | Complete behavioral ordering |
| #44 `common::dr64::{Blocked, compose_blocked}` | Boolean relation composition | Runtime effects, dispatch or triggers |
| #45 `common::cube::{facts, registry, live}` | Same-version coexistence in candidate ON/WHEN/DO registry | Verified authority for the `unattested` ON |
| Ruff #127 role vocabulary | `emitted_by` is effect, `reads_field` is read | Evidence that absence of calls/triggers/ACL proves purity |

Independent reference: pair-by-pair enumeration of every `emitted_by` producer and `reads_field` consumer for each shared field. This shares no DR composition code and does not use `RecomputeDag`.

**Expected historical acceptance:** slice 1 has 44 `RecomputeDag` edges plus 26 further distinct edges in the full composition; the executable test pins those counts and asks the independently enumerated pairs to match every DR bit. Do not substitute the old test's output for a new run.

## Independent source-census verification (completed)

The source blob was read anew through the GitHub connector and parsed as NDJSON in an independent JavaScript enumeration, without calling any Rust DR or RecomputeDag implementation. On the exact blob SHA above:

- 1,825 source triples;
- 34 fields both emitted and read;
- 70 distinct non-reflexive producer → reader pairs by enumerating every emitter for each field;
- 44 distinct pairs using the existing last-emitter-per-field rule;
- **26 lost pairs** and zero last-emitter pairs absent from the complete set;
- the two exact `currency_id` writers match the #46 illustrative export, in both identities and multiplicity.

This validates the input/oracle reference and historical #44 counts independently; **it does not run or validate the newly written Rust DR test**, so its pass/fail status remains unverified until Cargo/CI runs.

## Behavioral information loss: at least two different kinds

1. **Emitter multiplicity.** `account_move.currency_id` has two distinct source-declared methods, `_compute_currency_id` and `_onchange_invoice_vendor_bill`. Dropping either from the #46 source projection must be rejected. On this corpus, `RecomputeDag` keeps only the last emitter of a field and misses 26 ordering candidates in slice 1. A complete Boolean composition restores candidate dependencies, not method execution.
2. **Inheritance kind.** `od-ontology/src/inheritance.rs` explicitly notes that Ruff's upstream lift sends both `_inherit` and `_inherits` into `inherits_from`. But `_inherit` participates in Python method inheritance, while `_inherits` delegates fields and does not inherit those Python methods. Thus two behaviorally distinct source declarations map to the same triple: the lift is not injective. Neither C3 nor DR relation composition can recover which source form was used. Gate A therefore **STOPS** at this boundary pending a source-qualified declaration-kind witness. This does not authorize a new Ruff predicate, alternative ontology or canonical IR.
3. **Effect order.** A *synthetic only* scalar example `Set(1); Add(1)` produces 2, whereas `Add(1); Set(1)` produces 1, despite having the same unordered operation names. No Odoo body order was obtained from the coarse source corpus, so applying this equivalence to real methods is UNKNOWN.
4. **Authority / unknownness.** Source harvesting lacks a pinned effective Odoo registry, installed-addon ordering, some triggers, permissions, body writes and calls. `UNATTESTED` must not be reinterpreted as an automatic event, FALSE or a passed authorization guard. The #46 sample supplies no runtime observation.

## Negative controls and CI

The test-only file `crates/od-ontology/tests/d_behavior_eqsat_gate_a.rs` exercises:

- independent source corpus vs source-only example vs DT registry emitter parity, with two deliberate deleted-emitter negative cases;
- DR64 composition vs independent enumeration, while quantifying the `RecomputeDag` information deficit;
- a noncommuting effect order counterexample, labelled synthetic rather than an Odoo trace;
- `_inherit` / `_inherits` projection collision, with UNKNOWN source mode;
- unknown triggers, permissions and absent effective-registry witness.

**Load-bearing disable candidate:** replace `require_all_emitters`'s comparison with unconditional `Ok(())`. The deleted-emitter negative test must turn red. **Not claimed executed** until a reproducible CI or local run verifies it.

Reproduction requested:

```sh
cargo test -p od-ontology --test dt64_relcomp_parity --test dt64_on_when_do_cube
cargo test -p od-ontology --test d_behavior_eqsat_gate_a -- --nocapture
```

The workspace's existing clippy `doc_markdown` finding from #44 may still affect `-D warnings`. Test compilation, execution, timing, allocations and copied bytes are not measured by this document.

## Cross-domain contract inventory (research only)

| Consumer | Shared candidates | Missing before genuine behavioral lowering |
|---|---|---|
| Odoo | `MethodId`, `emitted_by`/`reads_field`, `Live` registry, `DR relcomp`, C3 | Source-declaration kind, ordered body effects, effective registry/version and permission witness |
| HIRO/MARS | ON event, WHEN precondition, DO capability and OGIT permitted edges (OGAR #363) | Attested handler semantics, installation identity and permission check; schema declaration is not a run |
| Ruff MeTTa | #125 parsed rules, engine-specific `Lowerable` / `InterpreterOnly`; #127 closed predicate roles | Existing ModelGraph projection does not carry call edges/verdicts; variable bindings, quotation and effects need an independent engine oracle |
| KPEA | KinaXe #24/#25 read-only method graph (subsequently merged); frozen KPEA oracle, `kinaxe-meta` provenance | No inferred biological effect from source, no replacement for source/evidence roots, no registry-validated ClassId/MethodId |
| egglog / MLIR | Optional cold optimization for already admitted pure expressions | Independent rewrite-law certification, typed proof witness, actual supported IR and execution parity; **not examined by Gate A** |

## Decision: STOP at Gate A loss, do not start B–E in this branch

The smallest missing interface is **not** another graph. It is a read-only, source-qualified *declaration-kind and effect-order witness* associated with the existing method/source identities and versions; unknown kinds and absent order must refuse behavioral lowering. The authoritative owner is Ruff for the source harvest, Odoo for interpreting Odoo declarations, and DR/DT for eventual admission.

**Falsifier for that interface:** present one `_inherit` and one `_inherits` relationship with equal `inherits_from` endpoints and the same graph neighborhood. Both coarse triples are identical. Method-inheritance eligibility must differ only when a separately verified declaration-kind witness is supplied. Without it, both remain UNKNOWN, not interchangeable.

After the owner decides how the witness is carried, a **MeTTa harvesting comparison** is the next best *research* gate, since Ruff #125 already provides a conservative engine-specific lowerability classifier. KPEA CodeGraph can follow, but proof-backed MLIR lowering must wait for independently admitted pure semantics. No new source vocabulary or implementation is authorized here.

No automatic merge.
