# OGAR vocabulary + lift-coverage surface (map, not a plan)

**Source read.** Local `/home/user/OGAR` HEAD (`f5bb789`) predates PR #164's
review-fix commit; `origin/main` (`0dad0c3201e4b...`, fetched via
`git fetch origin main -q`) is **7 commits ahead** of the requested pin
(unrelated `woa` harvest work). Every citation below is `git show
748ba11:<path>` — the exact merge commit of PR #164
(`748ba11dd7a4898b123abb9272a618996fcee3f0`, merges `dc02f96` + `507eafe`,
2 files changed: `crates/ogar-from-ruff/src/{lib.rs,mint.rs}`). File:line
refs are line numbers in that blob, not the working tree.

**Proof of read** — `ActionDef::kausal` doc, first sentence
(`ogar-vocab/src/lib.rs:386-387`): *"Causal precondition — when None,
action fires unconditionally at the right Te point."* `KausalSpec` type
definition line verbatim (`ogar-vocab/src/lib.rs:577`): `pub enum KausalSpec {`

## 1. `ogar_vocab::Class` (`crates/ogar-vocab/src/lib.rs:97-230`)

| Field | Type | Meaning | Line |
|---|---|---|---|
| `name` | `String` | source name, dots preserved (Odoo `account.move`) | 101 |
| `parent` | `Option<String>` | STI superclass name as written | 104 |
| `inheritance` | `Inheritance` (enum: `Root`/`Concrete{parent}`/`Abstract`/`RootedAt{root}`) | agnostic STI/abstract/root slot; mixins excluded | 112, enum 241-262 |
| `language` | `Language` (`Ruby`/`Python`/`Sql`/`TypeScript`/`SurrealQl`/`Elixir`/`Unknown`) | producer discriminant | 114 |
| `associations` | `Vec<Association>` | `belongs_to`/`has_one`/`has_many`/`habtm` (21-field struct, 729-798) | 117 |
| `mixins` | `Vec<String>` | `include`/`_inherit` paths | 120 |
| `enums` | `Vec<EnumDecl>` | `enum`/`Selection` columns (799-848) | 123 |
| `store_accessors` | `Vec<StoreAccessor>` | JSONB pseudo-field bundles (849-868) | 127 |
| `attributes` | `Vec<Attribute>` | typed-attribute overrides (869-928) | 130 |
| `table_name` | `Option<String>` | literal table override | 133 |
| `inheritance_column_disabled` | `bool` | STI dispatch opt-out | 137 |
| `ignored_columns` | `Vec<String>` | runtime blacklist | 140 |
| `scopes` | `Vec<Scope>` | named query scopes, opaque body (936-941) | 142 |
| `scope_predeclarations` | `Vec<String>` | scope-name-only predeclarations | 146 |
| `default_scope` | `Option<String>` | global filter body | 148 |
| `callbacks` | `Vec<Callback>` | lifecycle hooks (954-963) | 150 |
| `validations` | `Vec<Validation>` | validation decls (971-976) | 153 |
| `description` | `Option<String>` | Odoo `_description` | 161 |
| `record_order` | `Option<String>` | Odoo `_order` | 165 |
| `rec_name` | `Option<String>` | Odoo `_rec_name` | 168 |
| `check_company_auto`/`log_access`/`auto_create_table`/`register` | 4× `Option<bool>` | Odoo `_check_company_auto`/`_log_access`/`_auto`/`_register` | 171,174,177,186 |
| `abstract_model`/`transient` | 2× `bool` | Odoo `_abstract`/`_transient` | 180,183 |
| `declared_in_module` | `Option<String>` | manifest module name | 191 |
| `source_version` | `Option<String>` | reserved, always `None` in v1 | 194 |
| `source_domain` | `Option<String>` | coarse curator bucket (`"project"`/`"erp"`/`"german-erp"`) | 202 |
| `source_curator` | `Option<String>` | specific product (`"odoo"`/`"openproject"`/`"redmine"`/`"woa"`) | 211 |
| `canonical_concept` | `Option<String>` | normalized cross-domain identity | 220 |
| `computed_fields` | `Vec<ComputedField>` | Odoo `compute=` fields (271-291) | 224 |
| `methods` | `Vec<MethodDecl>` | CRUD overrides / plain methods (299-312) | 229 |

`Class` is `#[non_exhaustive]`; all enums/structs above likewise. No `Identity`
or `results_in` field exists anywhere in `ogar_vocab::lib.rs` at this pin
(`grep -n results_in` → no hits) despite several `docs/*.md` describing an
`ActionDef.results_in: StateTransition` slot (`OGAR-AST-CONTRACT.md`,
`INTEGRATION-MAP.md`, `HIRO-DO-ARM-LIFT.md`) — that field is **doc-only /
not yet coded**.

## 2. The behaviour arm

### `ActionDef` (lib.rs:370-425)

| Field | Type | Meaning | Line |
|---|---|---|---|
| `identity` | `String` | `<model>::action_def::<method>` | 373 |
| `predicate` | `String` | method/callback name as written | 376 |
| `object_class` | `String` | owning class name | 379 |
| `default_subject` | `ActionSubject` (`User`/`System`\*/`Cron`/`Trigger`/`Cascade`) | who fires by default | 381 |
| `default_temporal` | `TemporalSpec` (`Immediate`\*/`Deferred`/`Scheduled`/`OnCommit`) | when, by default | 383 |
| `default_modal` | `ModalSpec` (`Sync`\*/`Async`/`Idempotent`/`Atomic`) | how, by default | 385 |
| `kausal` | `Option<KausalSpec>` | causal precondition; `None` = fires unconditionally | 389 |
| `body_source` | `Option<String>` | verbatim method body | 391 |
| `decorators` | `Vec<String>` | decorator names driving extraction | 394 |
| `reads` | `Vec<String>` | fields READ (effect fact, not a reactive claim) | 399 |
| `writes` | `Vec<String>` | fields WRITTEN | 404 |
| `calls` | `Vec<String>` | dispatch calls (`<receiver>.<method>`) | 407 |
| `on_enter` | `Option<EnterEffect>` (`{field, to_value}`) | typed state mutation on `Committed` entry | 416 |
| `guard_failure_policy` | `Option<GuardFailurePolicy>` (`Postponable`/`Reject`\*) | disposition on guard failure | 420 |
| `state_timeout_millis` | `Option<i64>` | SLA deadline on `Pending` | 424 |

(\* = enum default via `#[derive(Default)]`.) `ActionDef::new(identity,
predicate, object_class)` (644-659) sets exactly those 3 fields; every other
field is `..Default::default()` — i.e. `kausal`/`body_source`/`on_enter`/
`guard_failure_policy`/`state_timeout_millis` are `None`, `decorators` is
`[]`, unless a producer explicitly assigns them post-construction.

### `KausalSpec` — sum type (lib.rs:571-604), 5 variants, no `Default`

| Variant | Fields | Line |
|---|---|---|
| `StateGuard` | `guard_field: String`, `guard_values: Vec<String>` | 579-584 |
| `LifecycleTrigger` | `event: String` | 586-589 |
| `Depends` | `paths: Vec<String>` | 592-595 |
| `ContextDepends` | `keys: Vec<String>` | 597-600 |
| `External` | (unit) | 603 |

Constructors: `KausalSpec::{state_guard, lifecycle, depends}` (679-701); no
`context_depends`/`external` convenience ctor (call the variant directly).

### `ComputedField` (271-291) / `Validation` (971-976) / `MethodDecl` (299-312)

| Type | Fields | Line |
|---|---|---|
| `ComputedField` | `field`, `compute_method`, `depends: Vec<String>`, `depends_context: Vec<String>`, `stored: bool`, `inverse_method: Option<String>`, `search_method: Option<String>` | 273-290 |
| `Validation` | `target: String`, `rule_source: String` — doc calls this a **"Placeholder shape; the validation-rule grammar is the next sprint"** | 972-975 |
| `MethodDecl` | `name`, `kind: MethodKind` (`CrudOverride`/`ApiModel`/`ApiModelCreateMulti`/`Instance`\*), `body_source: String`, `decorators: Vec<String>`, `semantics: RecordSemantics` (`Record`/`Recordset`\*/`ClassLevel`) | 301-311 |

## 3. Lift coverage matrix

`lift_model_graph`(Ruby, `lib.rs:87`) / `lift_model_graph_python`(`:96`) /
`lift_model_graph_sqlalchemy`(`sqlalchemy.rs:151`) all lift `Vec<Class>` only
— **none of the three ever touch `ActionDef`**. `lift_actions` (`lib.rs:534`)
is a *separate* function producing `Vec<ActionDef>` from a `Model`, called
only inside `mint.rs`'s `compile_graph_{python,sqlalchemy,ruby}` (each
identical: `CompiledClass { class, facet, actions: lift_actions(model) }`,
mint.rs:101/156/191).

| Class field | ruby | python | sqlalchemy |
|---|---|---|---|
| name/parent/inheritance/language | SET (176-179,163-165 shared) | SET (shared) | SET (sqlalchemy.rs:136-137, no `inheritance`/`parent` — stays `Class::default()`) |
| associations | SET (181, + `project_rails_fields`) | SET (181, + `project_odoo_fields`) | SET (141-142) |
| mixins/scopes/enums/callbacks/validations/default_scope | SET (182-198) | SET (182-198) | **NEVER-SET** (module doc 37-40: "Rails-DSL-only slots stay at `Class::default()` empty state") |
| attributes | SET (`project_rails_fields`, not_null→required) | SET (`project_odoo_fields`, type_name only, no not_null wiring) | SET (`project_sqlalchemy_fields`, not_null→required like Rails) |
| computed_fields | SET (via `Model::fields.emitted_by`) | SET (same) | SET (dormant — "no `@property`/compute-linkage pass exists yet") |
| source_domain/source_curator/canonical_concept | SET (per-graph lift wrapper) | SET | SET (`classify_woa_domain`, `german-erp` only) |
| **methods** (`Vec<MethodDecl>`) | **NEVER-SET** (`grep .methods` in lib.rs/sqlalchemy.rs/mint.rs → 0 hits) | **NEVER-SET** | **NEVER-SET** |

| ActionDef field | all 3 frontends (via `lift_actions`, lib.rs:534-560) |
|---|---|
| identity/predicate/object_class | SET (549-553) |
| reads/writes/calls | SET, verbatim from `Function` (554-556) |
| default_subject/default_temporal/default_modal | **NEVER-SET** — stay enum defaults (`System`/`Immediate`/`Sync`) |
| **kausal** | **NEVER-SET** by `lift_actions` — doc explicitly disclaims it (522-529): "a plain Rails method reading a field is not a reactive `@api.depends`-style trigger, so claiming one would leak method-body description into causal semantics" |
| body_source | **NEVER-SET** (doc 517-518: "the vocab `ActionDef` has no `exec` slot… backend routing is consumer-private") |
| decorators | **NEVER-SET** |
| on_enter/guard_failure_policy/state_timeout_millis | **NEVER-SET** |

**Hooks carried, not marked.** P1 fix `507eafe` made `lift_actions` chain
`model.functions.iter().chain(model.helpers.iter())` — public methods AND
private/protected helpers (Rails hook targets; "Redmine measurement: 67/84
hook targets live" in `helpers`) land in the **same** `Vec<ActionDef>`, no
marker field distinguishes them; a consumer must "re-join `Model::callbacks`
by name to tell hook targets from routable actions" (541-543). `mint.rs`'s
three `compile_graph_*` also hardened `debug_assert_eq!`→`assert_eq!` on the
`classes.zip(&graph.models)` 1:1 invariant (mint.rs:84-89, 142-147, 177-182).

**AT-CARRY-2's exact gap** (this repo's `W3.3-DELETE-GATE-MATRIX.md` calls
the shipped `CompiledClass.actions` "AT-CARRY-2/function inventory"; OGAR's
own commit calls the same PR "AT-CARRY-1" — a naming crosswalk, not two
things): `ActionDef.kausal` is **always `None`** out of every `ogar-from-ruff`
lift path. `KausalSpec` IS populated elsewhere — `ogar-from-schema::do_arm.rs`
/`registration.rs` (HIRO `ModelFilter`→`StateGuard`, do_arm.rs:129-149,
registration.rs:149-160) and `ogar-from-elixir` (`gen_statem`→`StateGuard`,
lib.rs:347-362) — but not from Ruby/Python/SQLAlchemy ORM harvests. AT-CARRY-3
(reactive+guard carrier) is unstarted for the odoo-rs producer path.

## 4. Codebook + ports (`ogar-vocab/src/ports.rs`, `app.rs`)

`PortSpec` trait (ports.rs:51-94): `NAMESPACE`, `BRIDGE_ID`, `APP_PREFIX: u16`
(default `0x0000`), `aliases() -> &'static [(&str, u16)]`, `class_id(name)`
= linear scan over `aliases()`.

| Port | `APP_PREFIX` | alias const | aliases (tuples) | line |
|---|---|---|---|---|
| `OpenProjectPort` | `0x0001` | `OPENPROJECT_ALIASES` | 28 | 104-171 |
| `RedminePort` | `0x0007` | `REDMINE_ALIASES` | 28 | 183-232 |
| `HealthcarePort` | `0x0005` | `HEALTHCARE_ALIASES` | 7 | 269-277 |
| `WoaPort` | `0x0003` | `WOA_ALIASES` | 25 (German+English synonyms) | 305-366 |
| `SmbPort` | `0x0004` | `SMB_ALIASES` | 20 | 390-435 |
| `OdooPort` | `0x0002` | `ODOO_ALIASES` | 20 (commerce arm + `account.analytic.line` cross-arm bridge) | 480-518 |

`render_classid_for::<P>(concept: u16) -> u32` = `render_classid(P::APP_PREFIX,
concept)` = `((concept as u32) << 16) | prefix` (app.rs:34-53) — **canon-high**
since the 2026-07-02 flip: concept is the high u16, app prefix the low u16.
`app_of`/`concept_of` (app.rs:63,72) decompose. `canonical_concept_name(id)`
/ `canonical_concept_id(name)` (lib.rs:1443-1471) round-trip against the
private `CODEBOOK: &[(&str,u16)]` table (lib.rs:1078+); `canonical_concept_in_domain`
(lib.rs:2681-2688) is the domain-gated resolver producers call once they know
`source_domain_concept(curator_domain)`.

**COUNT_FUSE** (lib.rs:1938-1959, quoted verbatim):
```rust
assert_eq!(
    ALL.len(),
    79,
    "class_ids::ALL count changed — update this pin AND the \
     lance-graph mirror COUNT_FUSE (crates/lance-graph-ogar/src/lib.rs) \
     in the same PR",
);
```
79 promoted concepts at this pin, mirrored by `lance_graph_ogar::parity::COUNT_FUSE`
in the sibling repo (one-directional fuse: catches OGAR-count drift, not
mirror-side drift).

## 5. Emit + dispatch consumers of the behaviour arm

- **`ogar-adapter-surrealql`** does **NOT** read `ActionDef` at all.
  `emit_surrealql_ddl(classes: &[Class]) -> String` (lib.rs:101) takes no
  actions parameter; `emit_class` (lib.rs:498) only reads
  `class.{name,description,attributes,associations,enums}` (511-519). The
  one mention is a forward comment: `// - DEFINE EVENT (lifecycle ->
  ActionDef)` (lib.rs:200, "not yet supported").
- **`ogar-emitter::emit_action_def`** (lib.rs:653-698) DOES emit `ActionDef`
  to RDF triples — `predicate`/`object_class`/`default_{subject,temporal,modal}`/
  `body_source`/`decorators`/`kausal` (`kausal_triples`, 742-778)/`on_enter`/
  `guard_failure_policy`/`state_timeout_millis` — **but not `reads`/`writes`/
  `calls`** (0 hits for `def\.reads|def\.writes|def\.calls`; `vocab/ogar.ttl`
  declares no such predicate either). AT-CARRY-1's effect-annotation fields
  survive in `CompiledClass.actions` in-memory but are **lost** on RDF/TTL emit.
- **`ogar-action-handler/tests/lifted_action_dispatch.rs`** (the only
  dispatch-side consumer here) needs only `action.predicate: String` — hand-
  builds `("command", format!("echo dispatched:{}", predicate))` and calls
  `CapabilityExecutor::execute(&self, capability: &str, bound:
  &[(String,String)]) -> Result<Vec<(String,String)>, String>`
  (`ogar-from-schema/src/action_ws.rs:376-386`). No `KausalSpec`, `on_enter`,
  or `reads`/`writes`/`calls` is read structurally; the test comment calls
  this "the reference bridge — production routing binds real capability
  params via the schema."

**What AT-CARRY-2 downstream must satisfy:** (1) dispatch only needs string
`predicate` today — no schema change required to keep the dispatch test
green; (2) guard/RBAC enrichment needs a re-join against `Class.callbacks`
by name (no marker on `ActionDef` itself distinguishes helper-hooks from
routable actions); (3) `kausal` is structurally present but always `None`
from every ORM-harvest producer — nothing to dispatch on until AT-CARRY-3;
(4) the RDF emit path silently drops `reads`/`writes`/`calls` — a TTL/RDF
consumer will not see the effect facts AT-CARRY-1/2 exist to carry.
