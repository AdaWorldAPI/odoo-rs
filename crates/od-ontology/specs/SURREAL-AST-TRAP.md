# SurrealQL-AST trap — pre-flight spellbook (od-ontology local mirror)

> **MANDATORY — read before** touching `surreal_ast.rs` as anything more than an
> egress adapter, before any W3 fork-deletion / "lower onto `ogar_vocab::Class`"
> work, before adding a `From<DdlAst> for ActionDef`, before claiming a
> behavioral roundtrip, or before colocating a new `*_dag.rs` / `triple.rs`
> beside the AST. 90 seconds, five questions, one mirror.
>
> **Why here:** OGAR #99's spellbook (`docs/SURREAL-AST-TRAP-PREFLIGHT.md` +
> `docs/SURREAL-AST-AS-ADAPTER.md`) names **this crate** —
> `surreal_ast.rs` + `triple.rs` + `recompute_dag.rs` colocated — as *the*
> diagnostic signature of the trap. So this is exactly where a session reflexes
> in. The reflex is pre-cognitive ("sessions don't argue their way in, they
> reflex their way in"), so the mirror has to fire **before the keyboard**.

## The five questions (any "stop" answer catches the trap pre-materialization)

```
Q1.  What am I reading FROM?          — does the source carry lifecycle vocab?
Q2.  Does the source have LIFECYCLE?  — @api.depends / @api.constrains / compute
                                         / state machines / action methods
Q3.  What is the TARGET IR?           — ogar_vocab::Class + ActionDef,
                                         or surreal_ast?  (Class is the IR.)
Q4.  Which way does my arrow point?   — producer → OGAR → adapter,
                                         or producer → DDL?  (The first.)
Q5.  Would the INVERSE recover?       — distil DDL back; do you recover the
                                         BEHAVIOR?  (No — see one-way-lossy.)
```

## The two-arm split — the load-bearing frame

SurrealQL DDL is a legitimate target for **structure** and an illegitimate one
for **behavior**. These are not the same question:

| Arm | Can SurrealQL AST be the IR? | Why |
|---|---|---|
| **Structural** — `Class` / `Attribute` / `Association` / `Enum` | **Lossy projection, never a substitution.** `From<Class> for catalog::TableDefinition` is the durable bridge; but `Class` STAYS the IR because `dependent:` / `inverse_of:` / `polymorphic:` / mixins / decorators / STI all vanish through DDL. | DDL answers "what shape," not "what the producer knew about how the shape came to be." |
| **Behavioral** — `ActionDef` + `ActionInvocation` + `KausalSpec` + `EnterEffect` + Rubicon `Pending→Committed` FSM + `state_timeout` | **No. Period.** | DDL has *no vocabulary* for the lifecycle FSM / SPO+TeKaMoLo / Rubicon binding. `DEFINE EVENT` / `DEFINE FUNCTION` are row-trigger / stored-proc shapes, not lifecycle-crossing shapes. |

**Roundtrip is one-way-lossy.** `emit` is bijective only on the schema-side
subset; `unmap` from DDL alone reconstructs an empty/default `Class` for
everything source-side. That is the honest version, not a bug — never claim a
behavioral `parse(emit(parse(x))) == parse(x)` through DDL.

## The classid is an address, not a carrier (why DDL *can't* hold the magic)

The deepest cut (cross-session consensus, OGAR #99 / lance-graph #591): a
`classid` carries **no magic at all** — it is pure address. The Rails
"class-magic" (callbacks, validations, `before_save`, associations) is not *in*
the id; it is what the id **resolves to**:

```
classid (dumb 32-bit key) ─┬─►  ClassView              ← the SKIN  (render/structure)
                           └─►  ActionDef + KausalSpec  ← the MAGIC (behavior / FSM)
        hi u16 = which app's SKIN (render lens)   ·   lo u16 = WHAT it is (domain:concept)
```

So even the **high u16 selects render magic, never class magic** — it picks
*whose ClassView/Askama template skins the concept*, not the behavior. Class
magic is a property of the Core type the classid points at, never of the
address. This is the Core-First `identity = classid` rule, and it is *exactly*
why the trap is a trap: **DDL carries the address-shaped structure but not the
Core**, so emitting `DEFINE EVENT` to "hold the lifecycle" is smuggling a Core
property (behavior) into a thing that can only ever carry the address (shape).
The id addresses; the Core behaves. Lower behavior to `ActionDef` (the value),
never into the id, the COMMENT, or the DDL (the key).

## Diagnostic signatures (for review — and an honest note about THIS crate)

- `.surql` files with `DEFINE EVENT … WHEN … THEN …` doing more than row triggers
- sentinel comments: `-- @action:` / `-- @state:` / `-- @lifecycle:`
- an IR file named `surreal_ast.rs` used as a project's **spine** (not adapter)
- `From<DdlAst> for ActionDef` / `to_action_def()` conversions
- behavioral-roundtrip claims through DDL
- a `triple.rs` / `recompute_dag.rs` cluster colocated with `surreal_ast.rs`

**This crate currently exhibits the last signature — on purpose, mid-migration.**
`surreal_ast.rs` + `triple.rs` + `recompute_dag.rs` are the W3 transition state:
the native `ToSql` path is a *documented* gap (see `ogar_bridge.rs` module docs)
being demoted to egress, not the end state. The trap is not "this crate has the
files"; the trap is **treating `surreal_ast` as the spine** instead of routing
through `ogar_vocab::Class` + `ActionDef`. The identity/structural work already
routes correctly (`emit_via_ogar_annotated` → `Class` → canonical adapter).

## Remediation — W3.3 is TWO separable lowerings, not one deletion

1. **Structural** — `triple.rs` shape facts → `ogar_vocab::Class`. DDL stays a
   lossy egress adapter (`ogar-adapter-surrealql`), never the IR.
2. **Behavioral** — `recompute_dag.rs` (Odoo's real compute / `@api.constrains`
   / action lifecycle) → OGAR's **behavioral arm**
   (`ActionDef` / `ActionInvocation` / `KausalSpec` / Rubicon FSM). **Not**
   `DEFINE EVENT` — emitting lifecycle as DDL re-enters the trap.

`surreal_ast` / `triple` / `recompute_dag` delete only **after both** land.
Lowering (2) is gated on OGAR's behavioral vocabulary being consumable from
`od-ontology` — **not verified wired today.** Route Q1–Q5 before writing code.

## Activation triggers (READ-BY)

Load this doc before producing output when a task mentions: `surreal_ast.rs`,
`DEFINE EVENT`, `DEFINE FUNCTION`, `.surql` as a spine, `distil from SurrealQL`,
`unmap`/`from_ddl`, `From<…> for ActionDef`, `recompute_dag`, `triple.rs` IR,
"behavioral roundtrip", or W3.3 / "delete the fork". Carried for
`core-first-architect` / `adapter-shaper` / `core-gap-auditor` / `Plan` agents.

## Canonical upstream

OGAR `docs/SURREAL-AST-TRAP-PREFLIGHT.md` (the spellbook) +
`docs/SURREAL-AST-AS-ADAPTER.md` (the design it extracts from), OGAR #99.
This file is the od-ontology-local mirror so the mirror fires in-repo.
