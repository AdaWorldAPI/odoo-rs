# W3.3 behavioral-arm lowering — scope (routed through the trap spellbook)

> System-2 scope for the second of W3.3's two lowerings (see
> `SURREAL-AST-TRAP.md`): Odoo's reactive lifecycle → OGAR's **behavioral arm**,
> NOT `DEFINE EVENT`. This doc is the Q1–Q5 routing the spellbook demands
> **before** any code. No parser was written to produce it.

## Q1–Q5 mirror (run on the proposed lowering)

- **Q1 — reading FROM?** The SPO corpus (`triple.rs`) — `has_function`,
  `reads_field`, `emitted_by`, `@api.depends`/`@api.constrains` decorator facts.
  **Yes, lifecycle vocabulary.**
- **Q2 — source has LIFECYCLE?** Yes — `_compute_*` reactive recompute,
  `_check_*` constrains guards, `_onchange_*` UI loops, `action_*` state
  transitions (draft→posted). This is `recompute_dag.rs`'s whole subject.
- **Q3 — target IR?** `ogar_vocab::ActionDef` + `KausalSpec` + `EnterEffect`
  + `ActionInvocation` — **not** `surreal_ast` / `DEFINE FUNCTION` / `DEFINE
  EVENT`. ✓
- **Q4 — arrow direction?** producer (Odoo corpus) → OGAR (`ActionDef`) →
  adapter (`ogar-emitter` renders). ✓ Not producer → DDL.
- **Q5 — inverse recover?** From `ActionDef` we recover the trigger
  (`KausalSpec`), the guard policy, the Rubicon crossing (`EnterEffect`), the
  ordering — the *behavior*. From `DEFINE EVENT` DDL we recover a row-trigger
  string and nothing of the FSM. The behavioral arm is the only IR that
  round-trips behavior. ✓

**Verdict: GREEN.** The lowering is the System-2 move, and it's not gated.

## FINDING — the behavioral arm is consumable TODAY (correcting the prior caveat)

Earlier I marked this lowering "gated on OGAR's behavioral vocabulary being
consumable — NOT verified wired." **The audit reverses that.** The full arm is
in `ogar-vocab` — the crate `od-ontology` *already pins* (`ogar-emit` feature):

| Type | `ogar-vocab/src/lib.rs` | Shape |
|---|---|---|
| `ActionDef` | `:373` | `predicate` + `object_class` + `kausal` + `on_enter` + `guard_failure_policy` + `state_timeout_millis` + `body_source` + `decorators` |
| `KausalSpec` | `:584` (sum type) | `StateGuard{guard_field,guard_values}` · `LifecycleTrigger{event}` · `Depends{paths}` · `ContextDepends{keys}` · `External` |
| `EnterEffect` | `:430` | `{field, to_value}` — the Rubicon `field := value` crossing |
| `ActionInvocation` | `:474` | per-call S/P/O + provenance + `ActionState` (Pending/Committed/Failed) |
| `ActionSubject`/`TemporalSpec`/`ModalSpec` | `:513`/`:537`/`:561` | SPO+TeKaMoLo grammar of the action |

No new dependency, no gate. The construction precedent is `ogar-emitter`
(builds `def.kausal = Some(KausalSpec::state_guard(…))`,
`def.on_enter = Some(EnterEffect::transition("state","sale"))`,
`def.state_timeout_millis = …`). The `ogar-from-<lang>` producer family
(`ogar-from-rails`/`-elixir`/`-ruff`) is the prescribed shape — **NOTE:**
`ogar-from-rails`'s *behavioral* depth is unverified (its `lib.rs` shows no
`ActionDef` refs yet; may be structural-only). Confirm before citing it as a
behavioral precedent; rely on `ogar-emitter` for the construction pattern.

## The mapping — `recompute_dag.rs` facts → `ActionDef`

| Odoo source fact (corpus) | `od-ontology` today (DDL — the trap) | → behavioral arm (the target) |
|---|---|---|
| `_compute_*` + `@api.depends('a.b')` | `DEFINE FUNCTION` + `VALUE` on field | `ActionDef{ predicate, kausal: Depends{paths:["a.b"]}, default_temporal: OnCommit, default_subject: Trigger }` |
| `reads_field`/`emitted_by` ordering DAG | topological order baked into `VALUE` recompute | the same DAG **validates** `Depends` consistency (keep `RecomputeDag` as the acyclicity check); ordering is the runtime's job, not DDL's |
| `_check_*` (`@api.constrains`) | `DEFINE EVENT … THEN { IF … THROW }` | `ActionDef{ kausal: LifecycleTrigger{event:"before_save"} or StateGuard{…}, guard_failure_policy: Reject }` |
| `action_post` / `action_*` (state xition) | (not currently lowered) | `ActionDef{ on_enter: EnterEffect::transition("state","posted"), default_subject: User }` — the Rubicon crossing |
| `_onchange_*` (UI cooperative loop) | not lowered (correctly) | `ActionDef{ default_subject: User, default_temporal: Immediate }` — legit non-DAG, stays out of the recompute invariant |
| method `body_source` (Python verbatim) | dropped | `ActionDef.body_source: Some(python)` — carried for later codegen, never executed as DDL |

`KausalSpec::Depends.paths` average 3 / p95 8 / max 14 (ogar-vocab's own R3
note) — matches Odoo `@api.depends` cardinality. The fit is not coincidental;
the arm was designed against ERP lifecycle (the `sale.order` worked example in
ogar-vocab's tests IS Odoo-shaped).

## Proposed shape (the deliverable of the *next* step, not this one)

A behavioral lowering in `od-ontology`, sibling to the structural
`ogar_bridge::schema_to_classes`, mirroring the `ogar-from-<lang>` shape:

```rust
// new: crates/od-ontology/src/ogar_actions.rs  (feature = "ogar-emit")
pub fn corpus_to_actions(triples: &[Triple]) -> Vec<ogar_vocab::ActionDef>;
```

- consumes `ogar_vocab` (already a dep) — **no fork, no `surreal_ast`**;
- drives off `recompute_dag::MethodKind::classify` (the classifier stays;
  only its *DDL destination* changes);
- `RecomputeDag` is retained as the **acyclicity validator** for the `Depends`
  set (a real, non-DDL invariant), not deleted.

## W3.3 sequencing (what deletes when)

1. **Structural** (mostly done) — `schema_to_classes` → `Class`; DDL is lossy egress.
2. **Behavioral** (this scope, ungated) — `corpus_to_actions` → `ActionDef`.
3. **Then** the `surreal_ast`/`emit` *lifecycle*-lowering deletes (the
   `Compute → DEFINE FUNCTION` / `Check → DEFINE EVENT` arms). `surreal_ast`'s
   pure `DEFINE TABLE`/`DEFINE FIELD` structural emit may remain as one egress
   adapter; `triple.rs` (the corpus reader) stays; `recompute_dag.rs` stays as
   the validator. The *fork* that deletes is the **lifecycle-as-DDL** code, not
   every file.

## Open checks before coding (the honest gate)

- Confirm `ogar_vocab::ActionDef` is exported under the features `od-ontology`
  enables (it's in `ogar-vocab` lib root; verify no extra feature needed).
- Confirm `ogar-emitter`'s render path (`ActionDef` → triples/DDL) is the
  intended egress, or whether `ogar-class-view` consumes `ActionDef` directly
  (the runtime path) — picks the emit target for `corpus_to_actions` output.
- Verify the corpus actually carries `action_*` state-transition facts (the
  `EnterEffect` source); if only compute/constrains are in the corpus today,
  the `on_enter` rows are a producer-side (lance-graph odoo-spo) ask, filed
  separately.
