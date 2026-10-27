# DT64 method registry: ON × WHEN × DO

Status: **lab design, 2026-10-27.** Nothing here is production code. The
probe that tests it is `crates/od-ontology/tests/dt64_on_when_do_cube.rs`;
its measured results are in §9. A claim not listed there as measured is a
design statement, not a finding.

Companion: `crates/od-ontology/tests/dt64_relcomp_parity.rs` (PR #44), which
checks one projection of this cube against `RecomputeDag`.

## 1. The idea in one paragraph

A method registry is a three-axis Boolean tensor. Each axis is a
vocabulary of at most 64 positions per frame:

- **ON**: events (a field changed, a record was created, a trigger item);
- **WHEN**: guards (named conditions on state);
- **DO**: capabilities (writes field f, raises, calls).

A registered method is a set bit at its `(on, when, do)` cell. The axes are
vocabularies and the methods are cells, so the 64 limit bounds the number of
event, guard and capability **kinds** per frame, never the number of methods.
One full frame is 64³ bits = 32 KiB. A vocabulary larger than 64 is split
into 64-wide blocks, as #44 does for relcomp.

## 2. Where each axis comes from

| axis | HIRO knowledge item | OGIT (`NTO/Automation`) | OGAR (`ogar-from-schema::do_arm`) | ruff role bit | Odoo corpus (this repo) |
|---|---|---|---|---|---|
| ON | `on` | `Trigger` contains `Item` (condition tree) | `KausalSpec::LifecycleTrigger { event }` | `TRIGGERS` | `depends_on` objects of the fields a compute method emits (declared `@api.depends`) |
| WHEN | `when` | `ActionApplicability.environmentFilter` ("such as `on ogit/_id`") | `KausalSpec::StateGuard { guard_field, guard_values }` | `REQUIRES` | provisional: `validation_kind` (`range`, `presence`, `format`) on `_check_*` methods |
| DO | `do` | `ActionCapability` (mandatory / optional / result parameters) | `CapabilitySlot`, `ActionDef.predicate` | `ENABLES` (and `EFFECTS` for writes) | `emitted_by` (field written), `raises` (abort) |
| body | KI XML | `KnowledgeItem.knowledgeItemFormalRepresentation` | pointed to, never parsed (lossless-DO) | `OPAQUE` | not in the corpus |
| assembly | | `ActionHandler` provides `ActionApplicability` provides `ActionCapability` | `ActionHandlerSpec` | | |

What OGIT carries for each facet, at schema level (OGIT fork at 2315167):

- **Trigger**: `Item` has only a name. Its condition lives in
  `KnowledgeItemAttribute`, which has no attributes and no edges.
- **Applicability**: `environmentFilter` is a string. OGAR lifts it to a
  `StateGuard` with empty `guard_values`. Values only exist in a deployed arago
  handler config.
- **Capability**: three "key/value mapping" string attributes. OGAR records
  only whether each exists (three booleans). Typed parameters come from a
  deployed handler config.

So OGIT names all three facets and types none of them. The cube's job is to
give them one shared, typed address.

## 3. The operations, and why they are folds

Notation: `C[on][when]` is a 64-bit DO mask. `G` is a 64-bit mask of the
guards that hold in the current state.

- **Dispatch** of event `e` in state `G`: `OR over w in G of C[e][w]`. At most
  64 word ORs per block, no allocation. This is a mask-risc fold.
- **Slices.** The same tensor read along a different axis:
  - "what fires on e": the `C[e]` plane;
  - "who can write f": bit `f` across all `(on, when)`;
  - "what does guard g gate": the `C[·][g]` rows.
- **Chaining.** When DO and ON share a domain (field-change events), "writes f"
  from one method is "f changed" for another. Projecting the tensor onto
  DO → ON and composing is a Boolean relation composition, which relcomp
  computes and egglog proves associative for Distinct relations. #44 checks
  one such projection (`emitted_by` ∘ `reads_field`) against `RecomputeDag`.

## 4. ON and WHEN stay separate

ON is an **edge**: something changed. WHEN is a **level**: something holds.
Merging them is the standard rule-engine mistake. The cube keeps them on
different axes by construction.

WHEN may be three-valued: a guard can be true, false or unknown for a state.
DR64 already carries validity bits for three-valued relations, so `G` may need
a validity mask too, and dispatch then needs the three-valued fold. Not
exercised by the probe: the Odoo corpus has no state preconditions.

## 5. Collisions: replacement versus coexistence

Two methods can occupy one cell. There are two different cases:

- **Replacement** (an override). Registrations are immutable rows in a
  version. An override is a later version that retracts the old registration
  and adds the new one. Replay "as of version v" answers which method held a
  cell then, and history stays readable. Versioning covers this case fully.
- **Coexistence.** Two methods live in the **same** version at the same cell
  must both fire. Odoo example: `_compute_currency_id` and
  `_onchange_invoice_vendor_bill` both write `account_move.currency_id`.
  Versioning cannot order these; neither replaced the other.

Rules that follow:

1. A new version replaces; methods in the same version coexist.
2. A presence bit says only that the cell is occupied. Method identities live
   in a position-keyed identity column beside the tensor, never inside it.
3. The order between coexisting methods must come from a declared source
   (Odoo: definition and inheritance order), never from file order.
4. Two coexisting methods of equal specificity are **reported** by name. The
   registry never picks one.

Rule 4 is the lesson of #44. `RecomputeDag::from_triples` keeps one emitter
per field, the last `emitted_by` line, so it treated coexistence as
replacement and dropped 26 of 70 ordering edges on slice 1 (37 on slice 2).

## 6. Agnostic algebra through a polyfill

The fold and relcomp never know they dispatch Odoo methods or OGIT verbs. They
see 64-bit masks and 64 × 64 registers. A DT64 slot binds them to a domain
through its typed axes. This is the same split as `ndarray::simd`: one facade,
many backends.

Laws are admitted per **(role, population)**, never per domain. Associativity
of Boolean Distinct composition is proven once
(`relcomp::admit_composition_assoc`), and every domain that binds a Distinct
slot inherits it without re-proving it.

## 7. Typed registration is multiple dispatch

Registering a method under typed axes gives:

- **ad-hoc polymorphism**: one name, one implementation per type. OGIT's
  `generates` has 158 source classes, i.e. one verb with 158 typed
  signatures. Today these exist only as `ogit:allowed` whitelist strings.
- **subtype polymorphism**: a class with no method at a cell falls back
  along its parent chain (ClassView parent, Odoo `_inherit`).

Dispatching on ON, WHEN and DO at once, each typed, is multiple dispatch
(Julia, CLOS), not single dispatch on a receiver.

Its known failure mode is **ambiguity**: two applicable methods, neither more
specific. Julia reports this instead of choosing. Typing removes accidental
collisions and turns the rest into genuine ambiguities, which the registry
reports (§5 rule 4).

Odoo caveat: `_inherit` extends a class (methods are inherited); `_inherits`
delegates to another record (fields are delegated, methods are not). The
fallback chain may only follow `_inherit`.

## 8. OGIT predicate registry, measured (OGIT fork 2315167)

| measure | value |
|---|---|
| verb TTL files | 523 (511 `owl:ObjectProperty`, 12 `rdfs:Class`) |
| verbs used in some `ogit:allowed` | 378 |
| declared, never used | 146 |
| used, never declared | 1 (`ogit:assesses`) |
| allowed edges | 2,894 distinct, over 941 entities |
| `ogit:cardinality` | 153 verbs, every one `"many2many"` |
| verbs touching one class, p95 / max | 10 / 123 (2 classes above 64) |
| well-typed verb pairs `v;w` at schema level | 12,229 of 142,884 |

- Verb files carry name, label, prose, validity, creator. No role, no
  inverse, no symmetry, no domain or range. Domain and range live only in
  each entity's `ogit:allowed ( [verb target] )`.
- Law candidates from schema mirroring only (not admissible without instance
  data): inverse `consistsOf`/`isPartOf` (24 class pairs),
  `hasTerminal`/`terminalOf` (15); symmetric `connects` (39% mirrored).
- Fleet code that already reads OGIT verbs:
  - OGAR `ogar-action-handler::ogit::OgitTables`: `Table::Allowed` with
    `src`, `verb`, `dst` as `u32` lanes. This is the typed edge-signature
    table a DT64 slot needs.
  - OGAR `ogar-from-schema::sgo::VerbDecl`: name, label, prose; registry
    index queued; domain/range out of scope.
  - OGAR `ogar-from-schema::do_arm`: the three facets lifted at schema level
    (§2).
  - lance-graph `lance-graph-ontology::ttl_parse`: verbs become edge
    proposals; `ogit:allowed` "not yet expanded".
  - ruff `ruff_spo_triplet::Predicate::role`: 13 role bits, for the ruff
    vocabulary, not for OGIT verbs.

## 9. The Odoo probe

Corpus: `data/account_move.spo.ndjson` (slice 1).

### What the corpus can and cannot supply

| needed | available | consequence |
|---|---|---|
| compute triggers | `depends_on`, field → field (declared `@api.depends`) | ON for compute methods; dotted paths are cross-record events |
| onchange triggers | none (no `onchange` predicate) | onchange methods register at ON = `unattested` |
| constraint triggers | none (no `constrains` predicate) | constraints register at ON = `unattested` |
| writes | `emitted_by` only (no `writes_field`) | DO = emitted field |
| guards | `validation_kind` on 5 check methods, `raises` on 18 | WHEN is provisional and sparse |
| inheritance | `inherits_from` (3 in slice 1, 166 in the manifest) | does not distinguish `_inherit` from `_inherits` |

### Claims and results

Results are in §10.

| # | claim | test |
|---|---|---|
| P1 | the dispatch fold equals a naive scan of live registrations, for every ON event and every guard mask | `dispatch_fold_equals_naive_scan_and_the_guard_axis_bites` |
| P2 | the WHEN axis changes dispatch for some event (the guard axis is not decorative) | `dispatch_fold_equals_naive_scan_and_the_guard_axis_bites` |
| P3 | coexisting writers of one field all appear in the DO slice | `coexisting_writers_all_appear` |
| P4 | replay as of an older version is unchanged by a later version; an override replaces only from its own version on | `replay_separates_replacement_from_coexistence` |
| P5 | cells holding two methods of equal specificity are reported by name, and their count equals an independent count | `equal_specificity_collisions_are_reported` |
| P6 | a class without a method at a cell falls back to its parent's, and a class with one does not | `subtype_fallback` |
| P7 | the DO→ON projection (declared deps) composed through relcomp equals a naive enumeration; its overlap with #44's reads-based relation is reported | `projection_composes_through_relcomp` |

## 10. Measured (slice 1, first green run)

| measure | value |
|---|---|
| axis sizes | ON 194 × WHEN 5 × DO 219 positions (ON and DO each need 4 blocks of 64) |
| live registrations, version 1 | 628 |
| ON events whose dispatch result depends on the guards | 1 (`unattested`) |
| cells holding two or more methods of equal specificity | 8 |
| declared-dependency ordering edges (DO → ON projection) | 209 over 195 methods and 345 field positions |
| `RecomputeDag` (reads-based) edges | 44 |
| edges in both | 0 |

What each result means:

- **P1, P2.** The fold equals the naive scan for all 194 events × 32 guard
  subsets. The guard axis changes the result for only one event, `unattested`,
  because every guard in this corpus comes from a check method and no check
  method has an attested trigger. P2 holds, but on the thinnest possible data.
- **P3.** Both writers of `account_move.currency_id`
  (`_compute_currency_id`, `_onchange_invoice_vendor_bill`) appear in the DO
  slice; the set equals the `emitted_by` set from the triples.
- **P4.** A synthetic version 2 retracts `_onchange_invoice_vendor_bill`
  and registers a replacement at the same cells. Replay as of version 1 is
  unchanged; as of version 2 the replacement holds those cells; the coexisting
  `_compute_currency_id` still writes `currency_id`.
- **P5.** All 8 collisions sit at ON = `unattested`. None is a real
  ambiguity between declared triggers: they exist because the corpus lacks
  onchange and constraint triggers, which collapses those methods onto one
  ON position. One cell groups 9 methods under
  `check:unclassified` → `raise:ValidationError`, including
  `_inverse_company_id`, which raises but is not a check. With declared
  triggers, compute methods produce no collisions at all.
- **P6.** C3 linearization from the inheritance manifest puts
  `mail_activity_mixin` in `account_move`'s chain. A mixin-only cell
  resolves to the mixin; a cell `account_move` also occupies resolves to
  `account_move` (the subclass shadows the base); the mixin, queried
  directly, sees its own. The mixin registrations are synthetic because the
  manifest lists method names but not fields.
- **P7.** The projection composed through relcomp equals a naive enumeration
  pair by pair. The zero overlap with `RecomputeDag` is a scope mismatch, not
  a contradiction: in slice 1, reads-based edges mostly end in methods of
  other models (`account_move_line._compute_currency_rate` reads
  `account_move.date`), and slice 1 does not carry those methods' declared
  dependencies. So the two orderings answer different questions on different
  method sets here; comparing them needs a corpus that covers both ends.

## 11. Open points

- WHEN data. No source in this repo carries state preconditions. OGIT carries
  only filter strings; the Odoo corpus only check-method kinds.
- ON for onchange and constraints needs the harvest to emit the decorator
  arguments (`@api.onchange(...)`, `@api.constrains(...)`).
- Declared order among coexisting methods needs a declared source per
  domain.
- `_inherit` versus `_inherits` must be separate predicates before subtype
  fallback can run on real inheritance.
- An OGIT verb → role-bit table does not exist.
- Laws beyond Boolean associativity (inverse, symmetric, transitive) need
  instance evidence before admission.

## 12. Counterfactual branches resolved by thought-git (proposal, 2026-10-27)

Operator idea: run the DO work of many alternatives in parallel, one per
thought slot ("main" plus 63 versions), and resolve them with thought-git by
comparing the counterfactual results, applying either a supersession rule or
synthesis inference.

### Two different 64s

- **DO axis width**: at most 64 capability kinds per frame. One dispatch
  already evaluates all of them at once (the fold of §3 returns a DO mask).
- **Thought slots**: thought-git's `SlotPool` (lance-graph
  `crates/thought-git-probe`, lab only): slot 0 holds the givens (main),
  slots 1..=63 hold live thoughts, each with a parent link and a
  generation-checked handle.

Branches are alternatives; the DO axis is not. Methods that **coexist** in
one registry version (§5) must all fire inside the **same** branch. Splitting
them into competing branches would turn coexistence back into replacement.

### What branches are for

1. **Unknown guards.** A WHEN guard that is unknown for the current state
   (§4) forks into two thoughts, guard true and guard false. If both land
   the same writes, the guard did not matter and the branches collapse; if
   they differ, the difference is the guard's effect. k unknown guards give
   2^k branches; k ≤ 5 fits in 63 slots.
2. **Registry versions.** Dispatching the same event against two registry
   versions (an override and the method it replaced) shows what the
   override changes.
3. **Hypotheses.** Any alternative input state.

Branches are independent until they land, so they run in parallel. Landing is
the serialization point: one data version per cycle. The landing order must
come from thought ids, never from completion order.

### What thought-git resolves today (read from `src/oracle.rs`, `src/slots.rs`)

| case | current rule |
|---|---|
| a write equal to a given | agreement; not stored |
| a write different from a given | refused; recorded as a `Contradiction` |
| landing thoughts offer one value for a cell | stored |
| landing thoughts offer different values | nothing stored; recorded as a `Divergence`; the cell keeps its previous value |
| a stack squashed into one thought | union of writes, later over earlier (supersession) |
| slot pressure | evict FIFO or least used; a candidate in a dependency chain is squashed instead of lost |
| comparing results | receipts carry folds over reads and writes; replay recomputes both and refuses on any difference |

So supersession exists: squash, younger over older. Comparison by folds
exists: two branches with equal write folds agree, with no decoding.
**Synthesis inference does not exist.** A divergence is recorded and left
unresolved.

### Where synthesis may and may not apply

Synthesis is a new landing rule, so it must be admitted per (role,
population) like every other law (§6):

- **Boolean Distinct writes** (DO presence, a field value): synthesis has
  nothing to combine. Two branches writing different currencies to
  `currency_id` do not average. These divergences stay reported (§5
  rule 4).
- **Truth-valued writes** (`TruthU8` / NARS frequency and confidence): NARS
  revision is a defined synthesis rule with its own algebra. A cell whose
  slot carries a truth population could land the revised value instead of a
  divergence.
- **Supersession** applies only along a parent link (a declared override or a
  squash chain), never between sibling branches. Siblings have no order to
  supersede by.

### What a probe would test (not built)

- Fork on the unknown guards of the cube (§10's 5 guard positions), run the
  fold per branch, land through `next_version`; assert collapse when the
  writes agree and a `Divergence` when they do not.
- Assert coexisting writers stay in one branch (no divergence between
  `_compute_currency_id` and `_onchange_invoice_vendor_bill`).
- A truth-valued cell lands by revision; a Boolean cell with the same
  disagreement stays a divergence.

Status of the pieces: thought-git is a lab probe and its API a proposal
(`.claude/plans/thought-git-api-v1.md`, admission is D-TGB-7). Nothing here is
admitted.
