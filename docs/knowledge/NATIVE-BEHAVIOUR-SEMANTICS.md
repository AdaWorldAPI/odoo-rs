# Native behaviour semantics — the 5 delete-blocking rows, one level deeper

> **Status:** ANALYSIS (append-only). Read-only pass over
> `docs/W3.3-DELETE-GATE-MATRIX.md` (branch `claude/odoo-transcode-ruff-ast-5ejqvr`,
> HEAD `84d474b`) + OGAR `origin/main @ 748ba11` (per task instruction), verified
> identical-in-relevant-part against the repo's actual `Cargo.lock` pin
> `0dad0c3` (`ogar-from-ruff`/`ogar-vocab` `KausalSpec`/`ActionDef`/`lift_actions`
> unchanged between the two — `git log --oneline 748ba11..0dad0c3 -- crates/
> ogar-vocab/src/lib.rs crates/ogar-from-ruff/src/{lib,mint}.rs` shows only an
> unrelated FK-dedup refactor). No cargo run. Every claim below is a static read.

## 0. Two lowering pipelines, not one — the fact the matrix didn't separate

odoo-rs has **two independent DO-arm lowerings**, both emitting `ActionDef`, and
they disagree on what they populate:

1. **`corpus_to_actions`** (`crates/od-ontology/src/ogar_actions.rs:45-106`) —
   consumes odoo-rs's own pre-extracted SPO **triple corpus** (`Triple.p ∈
   {has_function, reads_field, raises}`). Populates `kausal` directly
   (`Depends`/`LifecycleTrigger`) and `guard_failure_policy`. Consumed today only
   by `od_codegen --actions` (`crates/od-ontology/src/bin/od_codegen.rs:168-180`,
   via `corpus_action_rows`) — an inspection CLI, not a runtime sink.
2. **`compile_source`** (`crates/od-ontology/src/ogar.rs:134-136`) — parses Odoo
   **Python source text** directly via `ruff_python_spo::extract_from_source` +
   OGAR's generic `ogar_from_ruff::mint::compile_graph_python`, which calls
   OGAR's own `lift_actions` (`ogar-from-ruff/src/lib.rs:534-560` at 748ba11).
   This is the pipeline OGAR PR #164 (`748ba11`, "AT-CARRY-1") wired
   `CompiledClass.actions: Vec<ActionDef>` into — but `lift_actions`'s own
   doc-comment (`lib.rs:518-529`) states it **deliberately never populates
   `kausal`** ("a plain … method reading a field is not a reactive
   `@api.depends`-style trigger … `kausal` stays `None` here"). odoo-rs's own
   `compile_source_carries_the_do_arm` test (`crates/od-ontology/src/ogar.rs:
   607-648`) proves `reads`/`writes`/`identity`/classification travel through
   this path — it does **not** assert anything about `kausal`, because there is
   none to assert.

**Consequence:** only pipeline (1) can possibly satisfy AT-CARRY-2/3 as the
matrix defines them (`|ActionDefs| == |native Schema.functions|`,
`ActionDef.kausal == Depends{paths}`), because pipeline (2)'s `kausal` is
structurally `None`. The matrix's checklist doesn't say which pipeline AT-CARRY-2/3
targets; it must be (1), and (1) is not yet parity-tested against
`corpus_to_schema`'s output on the same triples (no test does this — verified by
grep, see §6).

## 1. Row #10 — computed `VALUE fn::<m>::<meth>($this) READONLY`

- **Construction site:** `crates/od-ontology/src/emit.rs:159-171` (the
  `one2many.is_none()` branch inside `corpus_to_schema`'s field loop).
- **Exact template:** `FieldDefinition::fmt_sql` (`surreal_ast.rs:167-184`) —
  `"DEFINE FIELD {name} ON {table} TYPE " + kind + " VALUE {v}"` (only if
  `value.is_some()`, line 174-176) `+ " READONLY"` (only if `readonly`, line
  180-182) `+ ";"`. Golden instance (`tests/account_move_slice.rs:51-54`):
  `"DEFINE FIELD amount_residual ON account_move TYPE option<any> VALUE
  fn::account_move::_compute_amount($this) READONLY;"`.
- **Input facts:** the field's `rdf:type ogit:Property` triple
  (`emit.rs:118-122`) + an `emitted_by` triple keyed on the field IRI
  (`(field, emitted_by, method)`, indexed at `emit.rs:90-95`,
  `pred::EMITTED_BY = "emitted_by"`). The `VALUE` expression is
  `format!("fn::{model}::{method}($this)")` at `emit.rs:167`.
- **Corpus classification:** none — no `MethodKind`/name-prefix gate. ANY
  `emitted_by` edge fires this, regardless of whether the method is literally
  named `_compute_*`.
- **`ActionDef`/`KausalSpec` today:** the `compute_method` name and stored/
  readonly-ness map to `ogar_vocab::ComputedField{field, compute_method,
  depends, stored, …}` (vocab `lib.rs:271-296` at 748ba11) — **and this carrier
  IS populated**, one level deeper than the matrix's "UNVERIFIED": OGAR's
  `project_odoo_fields` (`ogar-from-ruff/lib.rs:230-257`) does
  `class.computed_fields.push(computed)` when `field.emitted_by` is `Some`
  (line 253-256), with `computed.depends = field.depends_on.clone()`. **But**
  `ComputedField::new` (`ogar-vocab/lib.rs:2139-2147`) defaults `stored: false`
  and `project_odoo_fields` never sets it — so the READONLY half of row #10 has
  **no live source today**, in either pipeline. This lift also lives on
  pipeline (2) (`compile_source`/Python-source), not pipeline (1)
  (`corpus_to_actions`), which builds no `ComputedField` at all.

## 2. Row #11 — One2many reactive `VALUE <-child.<inv> READONLY`

- **Construction site:** `emit.rs:143-157` — `field.strip_suffix("_ids")`,
  resolves `target`/`inverse` via `RelationMap` (typed lift) or the
  convention ladder (`resolve_target`, `back_ref_name`); wins over the
  compute-VALUE path (comment `emit.rs:159-163`).
- **Exact template:** same `FieldDefinition::fmt_sql` as row #10; `value =
  Some(format!("<-{target}.{inverse}"))` (`emit.rs:155`), `readonly = true`
  (`emit.rs:156`). Golden instance (`tests/slice_2_typed_lift.rs:183-190`):
  the field block for `invoice_line_ids` contains
  `"VALUE <-account_move_line.move_id"` + `"READONLY"`.
- **Input facts:** the field name's `_ids` suffix (Odoo naming convention, not
  a triple) + `target`/`inverse_name` triples (typed, via `RelationMap`) or
  the focus-set name ladder (`resolve_target`, `emit.rs:422-439`).
- **Corpus classification:** none.
- **`ActionDef`/`KausalSpec` today:** `ogar_vocab::Association{kind:
  HasMany, inverse_of}` carries the target+back-ref (`inverse_of` populated
  at `ogar-from-ruff/lib.rs:249`) — **but there is no field anywhere on
  `Association` (checked the full struct, `ogar-vocab/lib.rs:729-780`) that
  marks "this HasMany is a read-only graph-traversal `VALUE` projection, not a
  stored array."** This is a genuine missing slot: today's `Association` can't
  tell an emitter to choose between `array<record<…>>` (Stage A, row #9) and
  `VALUE <-target.inv READONLY` (row #11) — that binary choice is
  `emit.rs`-local logic (`one2many.is_none()` gate) with no carrier mirror.

## 3. Row #15 — `DEFINE FUNCTION fn::<m>::<meth>(…)` (deferred stub)

- **Construction site:** `emit.rs:113-116`, inside the same `rdf:type` loop,
  `obj::FUNCTION` arm — fires per `(model, method)` pair whenever a
  `rdf:type … ogit:Function` triple exists and `triple::member_of` resolves.
- **Exact template:** `FunctionDefinition::stub` (`surreal_ast.rs:261-272`)
  builds `params = [("this", Record([model]))]`, `body =
  "/* deferred: port from Python */ RETURN NONE;"`; rendered by
  `FunctionDefinition::fmt_sql` (`surreal_ast.rs:275-287`):
  `"DEFINE FUNCTION fn::{model}::{method}(${pname}: {kind}, …) { {body} };"`.
  Golden instance (`tests/account_move_slice.rs:94-97`):
  `"DEFINE FUNCTION fn::account_move::_compute_amount($this: record<account_move>)
  { /* deferred: port from Python */ RETURN NONE; };"`.
- **Input facts:** one `rdf:type` triple per method
  (`(odoo:<model>.<method>, rdf:type, ogit:Function)`).
- **Corpus classification:** none — every method with this triple gets a
  stub, regardless of kind (compute / guard / onchange / action / helper).
- **`ActionDef`/`KausalSpec` today:** `ActionDef.identity/predicate/
  object_class` (Y, both pipelines) already carries the (model, method)
  identity that would let a consumer enumerate the same set — **but the
  method-inventory sets are gated differently in the two arms** (see §6, the
  concrete divergence), so `|ActionDefs|` is not provably `==
  |native Schema.functions|` yet. `ActionDef.body_source: Option<String>`
  (vocab `lib.rs:396`) exists as the natural carrier for the Python body — it
  is **never populated** by either `corpus_to_actions` (no such assignment in
  `ogar_actions.rs`) or `lift_actions` (`ogar-from-ruff/lib.rs:548-558` sets
  only `reads`/`writes`/`calls`). Worse: `ogar_vocab::Class.methods:
  Vec<MethodDecl>` (the OTHER place `body_source` lives, `vocab:299-312`) is
  declared but **`grep -n "class.methods" ogar-from-ruff/lib.rs`
  returns zero hits at 748ba11 — it is never assigned by any lift path.** The
  method body is structurally unreachable today.

## 4. Row #16 — `DEFINE EVENT` cross-record reactive recompute

- **Construction site:** `emit.rs:182-332` (the `depends_on` pass building
  `cross`, then the `for ((parent_model, rel), deps) in cross` loop).
- **Exact template:** `EventDefinition::fmt_sql` (`surreal_ast.rs:233-244`) —
  the literal format string, quoted verbatim (proof-of-read):
  ```
  "DEFINE EVENT {} ON {} WHEN {} THEN {{ {} }};"
  ```
  (`surreal_ast.rs:238-242`, args `self.name, self.table, self.when,
  self.then`). The **content** of `when`/`then` is built in `emit.rs`, not
  `surreal_ast.rs` — `corpus_to_schema`'s cross-record-event block is the
  function that renders `DEFINE EVENT` for this row; its WHEN-clause format
  string, quoted verbatim:
  ```
  format!("$event = \"UPDATE\" AND ({leaf_clause})")
  ```
  (`emit.rs:328`), where `leaf_clause` joins `"$before.{s} != $after.{s}"` per
  **first-segment-collapsed** leaf (`emit.rs:258-268` — a deliberate Core-shape
  concession: `$before`/`$after` are bare snapshots, no relation joins, so a
  multi-hop leaf like `partner_id.country.code` collapses to `partner_id`).
  `then` is `format!("LET $parent = $after.{back_ref}; UPDATE $parent SET
  {recompute}")` (`emit.rs:277`) or an unresolved-audit fallback
  (`emit.rs:280`), with `recompute = "{p} = fn::{parent_model}::_recompute_{p}
  ($parent)"` joined per dependent field (`emit.rs:272`). Golden instances:
  `tests/slice_2.rs:124` (`"DEFINE EVENT recompute_account_move_via_line_ids
  ON account_move_line"`) and `tests/slice_2.rs:141`
  (`"LET $parent = $after.move_id"`).
- **Input facts:** `depends_on` triples whose object is **cross-record**
  (`triple::is_cross_record`, dotted member path, `emit.rs:194`), plus
  `target`/`inverse_name` (typed) or the convention ladder for child-table +
  back-ref resolution.
- **Corpus classification / RecomputeDag:** **`RecomputeDag`
  (`recompute_dag.rs`) is NOT consumed by `corpus_to_schema` at all** — confirmed
  by `grep -rn "RecomputeDag" emit.rs lib.rs` returning only the `lib.rs`
  module declaration/re-export (`lib.rs:59,94`), never an import into
  `emit.rs`. Function/event emission order in the native path is a flat
  `(model,name)`/`(table,name)` sort (`emit.rs:359-361`), not a topological
  order. `RecomputeDag::topological_order` exists purely as an offline
  cycle-detection/ordering **validator** (tested against `MethodKind::Compute`
  subsets in `recompute_dag.rs:452-544`) — it answers "is the reactive graph
  safe," it does not participate in DDL generation.
- **`ActionDef`/`KausalSpec` today:** `KausalSpec::Depends{paths}` — **but
  sourced from a different, weaker-confidence predicate than the native path
  uses** (see §6). `paths` is a flat `Vec<String>`; it carries none of
  `emit.rs`'s derived structure (resolved `child_table`, `back_ref`,
  first-segment WHEN-collapse, or the "over-recompute" audit distinguishing a
  literal leaf from a multi-hop one). A consumer with only `KausalSpec::
  Depends` could not reconstruct the WHEN clause byte-for-byte — it would
  need to re-derive the child/back-ref resolution and the leaf-collapse logic
  itself, duplicating `emit.rs`.

## 5. Row #17 — `DEFINE EVENT` `@api.constrains` guard with `THROW`

- **Construction site:** `emit.rs:334-356` (the `raises` pass, gated by
  `is_guard`, `emit.rs:371-373`: `method.starts_with("_check_") ||
  method.starts_with("_constrain")`).
- **Exact template:** same `EventDefinition::fmt_sql` format string as §4.
  `when = "$event IN [\"CREATE\", \"UPDATE\"]".to_string()` (`emit.rs:350`,
  a fixed literal — not derived per-fact). `then = format!("IF !fn::{model}::
  {method}($after) {{ THROW \"{exc}: {method} failed\" }}")` (`emit.rs:
  351-353`). Golden instance (`tests/account_move_slice.rs:104-113`): the
  block for `check_invoice_currency_rate` contains `"DEFINE EVENT"`,
  `"THROW"`, `"ValidationError"`.
- **Input facts:** a `raises` triple (`(method, raises, exc:<Type>)`).
- **Corpus classification:** `is_guard(method)` (name-prefix) **AND**
  `raises.contains(method)` (triple) must **both** hold for native emit to
  fire this row.
- **`ActionDef`/`KausalSpec` today:** `KausalSpec::LifecycleTrigger{event:
  "before_save"}` + `guard_failure_policy: Reject`
  (`ogar_actions.rs:82-86`) — populated from `raises` **alone, with no
  name-prefix check** (see §6). `event` is a hardcoded literal, matching
  native's fixed `when` literal — that part is faithful. **Missing:** the
  exception type (`exc:ValidationError`) that composes the `THROW` message
  has no carrier slot on `ActionDef`/`KausalSpec` at all — `LifecycleTrigger`
  carries only `event: String`. Without it, a consumer cannot reconstruct
  the `THROW "<exc>: <method> failed"` message, only that a guard exists.

## 6. A concrete, falsifiable gate mismatch (the "one level deeper" finding)

Three independent guard/compute classifiers exist over the same method names,
and they do **not** agree:

| classifier | site | rule | used for |
|---|---|---|---|
| `is_guard` | `emit.rs:371-373` | `_check_*` OR `_constrain*` (no trailing `_` required) | native row #17 emission gate |
| `MethodKind::classify` | `recompute_dag.rs:83-99` | `_check_*` → `Check`; **no `_constrain` arm at all** | DAG-ordering only; also gates `corpus_to_actions`'s Depends arm (`Compute` == `_compute_*`) |
| `corpus_to_actions` guard arm | `ogar_actions.rs:79` | `raises.contains(method)` **alone — no name check** | AT-CARRY-2/3's `ActionDef` guard population |

Consequences, falsifiable without running cargo:

- A method that `raises` but is **not** named `_check_*`/`_constrain*` gets a
  guard `ActionDef` from `corpus_to_actions` but **no** `DEFINE EVENT` from
  native emit (`is_guard` fails) — `corpus_to_actions` over-produces relative
  to native for row #17.
- A method emitting a field (`emitted_by`) but not literally named
  `_compute_*` gets a native `VALUE …` (row #10, no name gate) but is
  **dropped** from `corpus_to_actions`'s Depends arm (`MethodKind::classify
  == Compute` required) — `corpus_to_actions` under-produces relative to
  native for the row #10/#16 pairing.
- **Separately**, `corpus_to_actions`'s `Depends{paths}` is built from the
  `reads_field` predicate (`ogar_actions.rs:55-59`; provenance band
  "body-inferred", confidence 0.85/0.75, `triple.rs:86-88`), while `emit.rs`'s
  rows #10/#11/#16 are built from `depends_on` (`emit.rs:186-213`; provenance
  band "decorator/body-authoritative", confidence 0.95/0.9, `triple.rs:82-85`)
  — two **different predicates** on two **different subject types** (method→
  field for `reads_field`; field→dep for `depends_on`) standing in for the
  same `@api.depends` fact. Whether the enrichment pass (`triple.rs:108-145`)
  keeps these sets in lock-step for every method is **UNVERIFIED** — I read
  only the doc comments and the two consuming code paths, not the extractor
  that produces the corpus. AT-CARRY-2/3 cannot be declared green until this
  is checked against real slice_2 data (a runnable probe, not a static read).

None of the three gate definitions above is "wrong" in isolation — each was
written for its own local purpose — but none of AT-CARRY-2/3's proposed parity
tests (`|ActionDefs| == |native Schema.functions|`, "matched by `predicate`+
`object_class`") can pass today without first reconciling these three gates
onto one shared classifier, or explicitly documenting why they're allowed to
differ.

## AT-CARRY-2 minimal fact-set (template slot → carrier field → exists?)

| template slot | row(s) | required carrier field | exists today? |
|---|---|---|---|
| field name / table | #10,#11 | `Attribute.name` (structural) | Y — already covered (matrix row #3) |
| compute method name | #10 | `ComputedField.compute_method` | Y, pipeline (2) only (`ogar-from-ruff/lib.rs:253-256`); absent from pipeline (1) |
| `store=True` / READONLY flag | #10 | `ComputedField.stored` | **N** — field exists, never set (`ComputedField::new` defaults `false`) |
| same-record depends set | #10 | `ComputedField.depends` | Y, pipeline (2) (`lib.rs:256`, from `field.depends_on`) |
| cross-record dep paths | #16 | `KausalSpec::Depends{paths}` | Y, pipeline (1) (`ogar_actions.rs:97`) — but sourced from `reads_field`, not `depends_on` (§6) |
| resolved child table + back-ref | #16 | *(none)* | **N** — `emit.rs`-local (`resolve_target`, `back_ref_name`); no `Association`/`ActionDef` mirror |
| WHEN first-segment collapse + over-recompute audit | #16 | *(none)* | **N** — `emit.rs`-local derived logic, not a stored fact |
| One2many virtual-projection marker | #11 | *(none)* | **N** — `Association` has `kind:HasMany` + `inverse_of` but no "is this VALUE-projected, not stored" bit |
| One2many back-ref column | #11 | `Association.inverse_of` | Y (`ogar-from-ruff/lib.rs:249`) |
| function/method inventory | #15 | `ActionDef.identity/predicate/object_class` | Y, both pipelines — but gated by mismatched classifiers (§6) |
| method body (Python source) | #15 | `ActionDef.body_source` | **N** — declared, never assigned, either pipeline |
| guard event trigger | #17 | `KausalSpec::LifecycleTrigger{event}` | Y, pipeline (1) (`ogar_actions.rs:82-84`) — literal `"before_save"`, matches native's fixed literal |
| guard failure policy | #17 | `ActionDef.guard_failure_policy` | Y, pipeline (1) (`ogar_actions.rs:85`) |
| exception type (`THROW` message) | #17 | *(none)* | **N** — no slot on `ActionDef`/`KausalSpec` carries the raised exception's type name |

## Honest bounds

- No cargo run, no byte-diff executed; every quote above is a static read of
  the cited `file:line`.
- §6's classifier-mismatch consequences are logical deductions from reading
  three functions side by side, not measured against real slice_2 counts —
  they are falsifiable (a probe could count them) but **not yet falsified
  either way**.
- I did not read `ruff_python_spo`'s or the odoo-blueprint-extractor's source
  (both out-of-repo/upstream); whether `reads_field` and `depends_on` are
  kept in lock-step by the enrichment pass is UNKNOWN from this repo alone.
- OGAR pin: read at `748ba11` per instruction; spot-checked against the
  repo's actual `Cargo.lock` pin (`0dad0c3`) for the specific files/symbols
  used here (`ogar-vocab/src/lib.rs`, `ogar-from-ruff/src/{lib,mint}.rs`) and
  found no diff in the relevant types/functions between the two commits.
