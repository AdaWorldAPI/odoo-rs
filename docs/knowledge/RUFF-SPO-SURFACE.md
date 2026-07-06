# The ruff SPO harvest surface — a census

Read from `/home/user/ruff`, `origin/main` @ `7716391` (fetched via
`git fetch origin main -q`; the local working tree at HEAD `72b7759`
is stale for `ruff_ruby_spo` / `ruff_cpp_spo` / `ruff_sqlalchemy_spo`
— `git diff --stat HEAD origin/main` shows real diffs there, e.g.
`ruff_cpp_spo/src/clang_walker.rs` gained 309 lines). Every file below
was read via `git show origin/main:<path>`, not the working tree.
`ruff_python_spo` itself is identical between HEAD and origin/main
except a 3-line `lib.rs` diff (irrelevant to what's cited here).

**Proof-of-read.** `crates/ruff_spo_triplet/src/triple.rs:946`:
`assert_eq!(Predicate::ALL.len(), 64);`. `crates/ruff_spo_triplet/src/ir.rs:86-88`
(`Model::helpers` doc, first sentence): "Non-public (`private`/`protected`)
defs — same [`Function`] body facts as `functions`, but NOT routable
actions: [`crate::expand`] emits no triples for them (no `has_function`),
keeping the action surface unchanged."

## 1. The IR (`crates/ruff_spo_triplet/src/ir.rs`)

`ModelGraph { namespace: String (l.48), models: Vec<Model> (l.50) }`.

### `Model` (struct l.76-221) — 25 fields

| Field | Type | Line | Who populates |
|---|---|---|---|
| `name` | `String` | 81 | all 5 frontends |
| `fields` | `Vec<Field>` | 83 | Odoo, SQLAlchemy (Ruby: stub, see §3) |
| `functions` | `Vec<Function>` | 85 | Odoo, Ruby (public defs), SQLAlchemy (name-only) |
| `helpers` | `Vec<Function>`, serde-defaulted | 95 | Ruby only (private/protected defs) |
| `inherits` | `Vec<String>`, serde-defaulted | 109 | Odoo (`_inherit`) only |
| `associations`,`validations`,`callbacks`,`concerns`,`attributes`,`delegations`,`scopes`,`acts_as`,`dsl_calls`,`gem_dsl`,`dynamic_methods`,`refinements` (12 `Vec<…>`) + `sti` (1 `Option<StiInfo>`) | OP AR-shape siblings, serde-defaulted | 116-180 | Ruby only, **except** `associations`: also SQLAlchemy (`BelongsTo`/`HasMany` only, §3) |
| `bases`,`member_fields`,`methods`,`templates`,`friends`,`macro_uses`,`static_asserts` (7 `Vec<…>`) | C++ machine-plane siblings, serde-defaulted | 192-220 | C++ only |

**`helpers` vs `functions` (l.86-95):** both carry the same body-fact
shape (`Function`); `functions` are public/routable defs — the
expander (`expand.rs:239-308`) walks `model.functions` and emits
`rdf:type Function` + `has_function` + all body predicates for each.
`helpers` (private/protected Ruby defs) are **never walked by
`expand()`** — no triples at all, not even `rdf:type`. They exist so
a downstream consumer (Rails callback-target resolution, OGAR F17
body triage) can still read a private method's body facts without it
polluting the public action surface. Only `ruff_ruby_spo` splits
public/helper (`functions.rs:61-109`, `extract_functions_from_body`
returns `(Vec<Function>, Vec<Function>)`); Odoo and SQLAlchemy never
populate `helpers` (all `def`s land in `functions`); C++ has no
`helpers` concept (`methods: Vec<CppMethod>` carries `access:
CppAccess`, l.732, instead).

**`guarded_writes` (ir.rs l.304-313, ruff #45/#47):** a subset of
`writes` — fields whose write is guarded by a blank/nil/present test
on that same field (Ruby `self.x = v if self.x.blank?`, `self.x ||=
v`; C# `X ??= v`, `if (X == null) X = v`). Distinguishes a **schema
default** from a **`normalizes` transform** (J1,
`.claude/knowledge/fuzzy-recipe-codebook.md` §5 in the ruff repo).
`self.x &&= v` (present-guarded) is recorded in `writes` only, NOT
`guarded_writes` — the mirror-image guard (functions.rs:429-437).

### `Field` (struct l.224-271) — 8 fields

| Field | Line | Populated by |
|---|---|---|
| `name` | 227 | Odoo, Ruby (schema-arm only), SQLAlchemy |
| `depends_on` | 230 | **Odoo only** (`@api.depends` fan-out) |
| `emitted_by` | 233 | Odoo (declared `compute=`), Ruby schema-arm (heuristic `compute_<field>` name match, `schema.rs:163-172`, NOT declarative) |
| `target` | 238 | Odoo only (relational comodel, raw dotted) |
| `inverse_name` | 242 | Odoo only (One2many inverse) |
| `relation_kind` | 249 | Odoo only (`many2one`/`one2many`/`many2many`) |
| `field_type` | 260 | Odoo (scalar ctor), Ruby schema-arm (migration DSL type token), SQLAlchemy (`types::field_type_of`) |
| `not_null` | 270 | Ruby schema-arm (`null: false`), SQLAlchemy (`nullable=False` / PK) — **never Odoo** (no not_null concept extracted) |

### `Function` (struct l.283-324) — 7 fields

| Field | Line | Populated by |
|---|---|---|
| `name` | 286 | all (Odoo, Ruby, SQLAlchemy, C# via raw triple, C++ via `CppMethod.name`) |
| `reads` | 289 | Odoo (`py_functions.rs:108-119`), Ruby (`rb_functions.rs:392-395`), C# (`Program.cs` `reads_field` arm) |
| `raises` | 292 | Odoo, Ruby, C# — **not SQLAlchemy** (v0 name-only) |
| `traverses` | 295 | Odoo, Ruby — **not SQLAlchemy, not C#** |
| `writes` | 303 | Ruby, C# — **not Odoo** (Odoo's write target is declarative via `Field::emitted_by`, body-write capture deferred; `py_lib.rs:222-227` comment) |
| `guarded_writes` | 313 | Ruby, C# only |
| `calls` | 323 | Ruby (closed `AR_MUTATORS` set, `rb_functions.rs:554-585`), C# (EF Core mutator set) — **not Odoo, not SQLAlchemy** |

## 2. The predicate vocabulary (`triple.rs`) — 64 total, locked

Test: `predicate_count_locked_at_64` (triple.rs:923-947); round-trip
test `predicate_string_round_trips_for_every_canonical` (l.907-921).

| Stratum | # | Default tier | Predicates (`as_str`) |
|---|---|---|---|
| Core 7 (Odoo harvest) | 7 | Structural: `rdf:type`,`has_function`; Authoritative: `emitted_by`,`depends_on`,`raises`; Inferred: `reads_field`,`traverses_relation` | `rdf:type`, `has_function`, `emitted_by`, `depends_on`, `reads_field`, `raises`, `traverses_relation` |
| OpenProject AR-shape | 32 | OpenProjectExtracted (default); Structural override: `concern_class_methods`,`concern_included_block`; Inferred override: `defines_method` | `declares_association`, `validates_constraint`, `normalizes_attribute`, `has_callback`, `includes_module`, `extends_module`, `prepends_module`, `concern_class_methods`, `concern_included_block`, `has_attribute`, `aliases_attribute`, `aliases_method`, `column_override`, `delegates_to`, `has_scope`, `has_default_scope`, `acts_as`, `registers_journal_formatter`, `registers_journal_formatted_fields`, `has_dsl_call`, `mounts_uploader`, `has_paper_trail`, `has_closure_tree`, `counter_cultures`, `auto_strips`, `defines_method`, `uses_refinement`, `field_type`, `association_kind`, `class_name`, `validation_kind`, `validation_param` |
| C++ machine-plane | 18 | CppExtracted (default); Structural override: `is_friend_of`; Inferred override: `uses_macro_expansion`,`template_instantiates` | `inherits_from`\*, `has_field`, `template_specialises`, `template_instantiates`, `virtually_overrides`, `is_friend_of`, `defines_operator`, `uses_macro_expansion`, `is_pure_virtual`, `is_constexpr`, `is_noexcept`, `requires_concept`, `static_asserts`, `returns_type`, `has_param_type`, `is_const`, `is_static`, `has_visibility` |
| Odoo-relational | 3 | Authoritative | `target`, `inverse_name`, `relation_kind` |
| Body-mutation | 2 | Authoritative: `writes_field`; Inferred: `calls` | `writes_field`, `calls` |
| J1 body-mutation | 1 | Authoritative | `writes_if_blank` |
| Schema-stratum | 1 | Authoritative | `column_not_null` |

\* `inherits_from` is **cross-frontend**: emitted by C++ (base
classes, `Provenance::CppExtracted`) AND by the frontend-agnostic
`Model.inherits` path (Odoo `_inherit`, Rails STI —
`Provenance::Authoritative`, triple.rs:316-336, `expand.rs:351-362`).

Provenance truth table (`truth()`, triple.rs:892-899): `Structural
(1.0,1.0)`, `Authoritative (0.95,0.90)`, `Inferred (0.85,0.75)`,
`OpenProjectExtracted (0.95,0.88)`, `CppExtracted (0.95,0.82)` — each
tier one NARS revision-count below the last except `Inferred`, which
also drops frequency.

## 3. Frontend population matrix

Columns: **Od** = `ruff_python_spo` (Odoo), **Rb** = `ruff_ruby_spo`
(Rails), **SA** = `ruff_sqlalchemy_spo` (Flask-SQLAlchemy, "v0
schema-only"), **C#** = `ruff_csharp_spo` (Roslyn harvester —
**does not build a `ModelGraph` at all**; emits `Triple`s directly
from `harvester/Program.cs`, validated against the closed vocab by
`ruff_csharp_spo::load`/`from_ndjson`), **C++** = `ruff_cpp_spo`
(libclang, `extract_dir`/`extract_tree` real; top-level `extract()`
still `todo!()` pending per-TU include auto-detection, `cpp_lib.rs:151-157`).

| IR fact | Od | Rb | SA | C# | C++ |
|---|---|---|---|---|---|
| `Model.name` | POPULATED `py_lib.rs:231` | POPULATED `lib.rs:221` (`Model::new`) | POPULATED `sa_lib.rs:148` | POPULATED (`rdf:type` triple, `Program.cs:63`) | POPULATED `cpp_lib.rs:278` |
| `Field` (any) | POPULATED | EMPTY-BY-DESIGN base arm (`extract_fields` stub, `lib.rs:315-317`, "**D-AR-3 stub** — returns empty"); POPULATED via separate `extract_app_with_schema` (`schema.rs:120-145`) | POPULATED `sa_lib.rs:149-158` | POPULATED (`has_field`/`field_type` triples, `Program.cs:79,142-144`) | N-A (C++ data members go through `CppField`/`member_fields`, never `Field`) |
| `Field.depends_on` | POPULATED `py_lib.rs:202-207` (`@api.depends` fan-out) | EMPTY-BY-DESIGN — no Rails decorator analog to `@api.depends`; not attempted even in the schema arm | N-A (not modeled) | N-A | N-A |
| `Field.emitted_by` | POPULATED (declared `compute=` kwarg, `ruff_python_spo/src/walk.rs:116-119` → joined onto `Field` at `py_lib.rs:201`) | POPULATED, schema-arm only, **heuristic name match** `compute_<field>` (`schema.rs:163-172`) — NOT a declaration read | N-A | N-A | N-A |
| `Field.target`/`inverse_name`/`relation_kind` | POPULATED `ruff_python_spo/src/walk.rs:135-154` (`relation_target_inverse`) | N-A (Rails uses `associations`/`AssocDecl` instead, see below) | N-A (SQLAlchemy also uses `associations`, `sa_lib.rs:160`) | N-A | N-A |
| `Field.field_type` | POPULATED `ruff_python_spo/src/walk.rs:108-113` (scalar ctor, lowercased; mutually exclusive with `relation_kind`) | POPULATED, schema-arm, DSL token verbatim (`schema.rs:73-99`) | POPULATED `sa_columns.rs:46-50` (`types::field_type_of`) | POPULATED (raw C# type string, `Program.cs:144`) | N-A |
| `Field.not_null` | EMPTY-BY-DESIGN — never set (no non-nullable concept extracted) | POPULATED, schema-arm, `null: false` (`schema.rs:298-309`) | POPULATED `sa_columns.rs:58-67` (`nullable=False` or PK) | N-A (no `column_not_null` emitted by the harvester) | N-A |
| `Function.name` | POPULATED | POPULATED (public split, `functions.rs:100-109`) | POPULATED, name-only (`sa_functions.rs:14-19`) | POPULATED (`has_function` triple) | N-A (goes through `CppMethod`) |
| `Function.reads` | POPULATED `py_functions.rs:108-119` | POPULATED `rb_functions.rs:392-395` | EMPTY-BY-DESIGN (v0 doc: "body facts … left empty", `sa_functions.rs:4-7`) | POPULATED (`Program.cs` `reads_field` arm, l.253-256) | N-A |
| `Function.raises` | POPULATED `py_functions.rs:98-104` | POPULATED `rb_functions.rs:369-374` | EMPTY-BY-DESIGN | POPULATED (`Program.cs:230-236`) | N-A |
| `Function.traverses` | POPULATED `py_functions.rs:80-97` | POPULATED `rb_functions.rs:439-467` | EMPTY-BY-DESIGN | EMPTY-BY-DESIGN (no association-traversal walk in `Program.cs`) | N-A |
| `Function.writes` | EMPTY-BY-DESIGN (`py_lib.rs:222-227`, deferred — Odoo's write target is already declarative via `emitted_by`) | POPULATED `rb_functions.rs:378-437` | EMPTY-BY-DESIGN | POPULATED (`Program.cs:277-292`) | N-A |
| `Function.guarded_writes` | EMPTY-BY-DESIGN (same as `writes`) | POPULATED `rb_functions.rs:403-416`, `287-359` (J1 If/IfMod shape) | EMPTY-BY-DESIGN | POPULATED (`Program.cs` `writes_if_blank` arm, l.283-289, 296-306) | N-A |
| `Function.calls` | EMPTY-BY-DESIGN | POPULATED `rb_functions.rs:442-459`, `AR_MUTATORS` (l.554-585) | EMPTY-BY-DESIGN | POPULATED (`Program.cs:238-241`, EF Core mutator set) | N-A |
| `Model.helpers` | EMPTY-BY-DESIGN (Python has no private/protected visibility split; every `def` is a routable action) | POPULATED `functions.rs:61-109` | EMPTY-BY-DESIGN | N-A (no `ModelGraph`) | N-A (no `ModelGraph`-level split; `CppMethod.access` carries C++ visibility instead) |
| `Model.inherits` | POPULATED `py_lib.rs:180-185` (genuine `_inherit`, self-edge filtered) | EMPTY-BY-DESIGN in the base extractor; Rails STI goes through `Model.sti` → `inherits_from` instead (`expand.rs` `sti()`) | N-A | N-A (`inherits_from` triple direct, `Program.cs:70`) | N-A (C++ uses `Model.bases`, not `Model.inherits`) |
| `Model.associations` (OP AR-shape) | N-A | POPULATED `walk.rs:361-373` | POPULATED `sa_relationships.rs` (`BelongsTo`/`HasMany` only — no `HasOne`/`HasAndBelongsToMany`/`AcceptsNestedAttributesFor`) | N-A | N-A |
| `Model.validations`/`callbacks`/`concerns`/`attributes`/`delegations`/`scopes`/`acts_as`/`dsl_calls`/`gem_dsl`/`dynamic_methods`/`refinements`/`sti` (12 Rails-DSL siblings) | N-A | POPULATED (`walk.rs`, per-DSL-keyword dispatch, l.49-562) | EMPTY-BY-DESIGN — not this crate's scope (SPEC-5 Part A is schema+associations only) | N-A | N-A |
| `Model.bases`/`member_fields`/`methods`/`templates`/`friends`/`macro_uses`/`static_asserts` (7 C++ siblings) | N-A | N-A | N-A | N-A | POPULATED `cpp_lib.rs:288-297` (`unpack_declaration`), real walker `clang_walker.rs` (feature `libclang`) |

## 4. Decorator capture — the AT-CARRY-2 answer

**Today, `ruff_python_spo` captures exactly one decorator: `@…depends(...)`.**
`functions.rs:18-27` (`analyze_method`):

```rust
for decorator in &func.decorator_list {
    if let Expr::Call(call) = &decorator.expression
        && terminal_name(&call.func) == Some("depends")
    { depends.extend(call.arguments.args.iter().filter_map(expr_str)); }
}
```

`terminal_name` (l.135-141) matches on the **trailing identifier only**
— `@api.depends(...)` matches because the trailing attr is `depends`;
so would a hypothetical `@foo.depends(...)`. There is no check that
the receiver is literally `api`.

**No other Odoo decorator is captured as a decorator kind, anywhere
in `ruff_python_spo`** — confirmed by grep across
`functions.rs`/`walk.rs`/`lib.rs` for `constrains`/`onchange`/`api.model`:
the only hits are inside the `lib.rs` test fixture string (`@api.constrains`
at `py_lib.rs:290`, exercised only for its `raises` body-fact, not for
the decorator itself). Concretely absent, as of `Predicate`'s closed
vocab too — there is no `constrains`/`onchange`/`is_compute_model`
predicate variant.

**What this means for OGAR's AT-CARRY-2** (populating `KausalSpec`
from decorator kinds): ruff gives you `depends_on` (from `@api.depends`
args) and nothing else decorator-shaped. `@api.constrains` methods are
only indirectly visible via their `raises` body-fact (they usually
raise `ValidationError`) — there is no `validates_constraint`-equivalent
predicate wired for Odoo (that predicate exists but is Rails-only,
`ValidatesConstraint`/OP AR-shape). `@api.onchange` and `@api.model` are
**entirely unrepresented** — no field, no predicate, no body-fact proxy.
AT-CARRY-2 must either (a) treat this as a known gap and only classify
compute/constrain-by-convention (name prefix `_compute_`/`validate_` +
`raises`), or (b) file a ruff-side follow-up to add decorator-kind
capture (a `Function.decorator_kind: Vec<String>` or similar) before
KausalSpec can distinguish onchange/model/constrains reliably.

## 5. View-stratum extractors (`ruff_ruby_spo`, ruff #46)

**`views.rs`** (517 lines) — ERB view field-set extractor. Doctrine
(l.1-58, "detected config becomes data" / fuzzy-recipe-codebook.md
§8c): a view is scanned as a **closed-vocabulary line scanner** (no
ERB/Ruby parse) for `<receiver>.<field>` references against
caller-supplied `ViewTarget{model, receivers, fields}` vocabularies.
Presence-only — records THAT a field is referenced, never markup/
layout/conditionals. Tier: Inferred by construction. Key API:
`extract_view_field_sets_with_report` (l.130), yielding
`ViewFieldSet{resource, fields, referenced}` where `fields ⊆
referenced` always (l.41-53) — `referenced` is the raw denominator so
coverage isn't trivially 1.0.

**`representers.rs`** (676 lines) — `OpenProject` APIv3
representer/schema field-declaration extractor. Same discipline: a
`std`-only, presence-only line scanner over `app/**/*_representer.rb`
(Roar/Representable DSL), matching a closed `KEYWORDS` set (l.106-119:
`property`, `date_property`, `date_time_property`,
`formattable_property`, `associated_resource`, `associated_project`,
`resource`, `resources`, `link`, `links`,
`schema_with_allowed_link`, `schema_with_allowed_collection`, `schema`)
via maximal-munch identifier scan (`date_property` never
misparses as `property`). For `schema`-family keywords, an optional
`type:` wire-type hint is scanned from continuation lines
(best-effort, Inferred). Entry: `extract_representer_field_sets` (l.128).

**Mirror to odoo-rs's `ir.ui.view` FieldMask lane**
(`crates/od-ontology/src/view_mask.rs`): same doctrine, ported to
Odoo XML views instead of ERB — a hand-rolled, zero-new-dep,
presence-only tag scanner (explicitly NOT a general XML parser,
doc l.19-27) that splits `<field name="X">` references into
**top-level** (this model's fields → `ViewFields::fields`) vs
**nested** (comodel hops → `ViewFields::relation_hops`, `(outer,
inner)` pairs) — the same this-model/comodel-hop split `views.rs`'s
doc calls out as the one thing it does NOT attempt (multi-hop chains,
l.33-36: "only the first hop off a registered receiver is read").
Both lanes exist to mint a `FieldMask`/`ClassView` projection
(`view_mask.rs` l.9-17 references `lance-graph`'s `WideFieldMask`
directly) without parsing presentation structure — one doctrine
(config-as-data), two languages' view DSLs.
