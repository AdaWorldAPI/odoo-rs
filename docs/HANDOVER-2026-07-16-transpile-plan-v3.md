# Transpile plan v3 — post-merge rebase; the arc is in the mains

> **Date:** 2026-07-16 · **Supersedes** `HANDOVER-2026-07-08-polyglot-transpiler.md`
> **§1 (branch table) and §4/§5 (remaining/first-moves)** — those sections
> described branches and gaps that have since MERGED. §0 of that doc (the
> polyglot-transpiler framing: IN→IR→OUT; upstream `odoo/` is read-only
> harvest + parity oracle, never the product) is **unchanged and still
> authoritative** — read it first if you haven't.
> **Goal (unchanged):** odoo → odoo-rs on OGAR V3; SurrealQL absolutely
> deprecated. **Model policy:** Opus plan/review, Sonnet grindwork.
> **Mode:** don't stop; auto-resolve; small PRs; fuses over prose.

---

## 1. Merged ledger — what the arc shipped (all in mains as of 2026-07-16)

| Repo | PR | What |
|---|---|---|
| ruff | #51 | `ruff_python_spo` DTO arm (writes / guarded_writes / calls) |
| ruff | #49 | kausal arms B+D fields (`constrains`/`onchange`, `Field::stored`) |
| ruff | #66/#67/#68 | Odoo nav (Klickweg Shape A+B) + view field-sets arms |
| ruff | #76 | region-grammar vocab: `Predicate::{DockedAt,TabOrder,OpensPopup}` (73→76) + `region=` config + `[regions]` digest |
| ruff | #79 | **Odoo region harvester** `ruff_python_spo::extract_odoo_view_regions` (merge promoted `RegionFact`/`region_triples` into shared `ruff_spo_triplet`; canonical `{screen}.{control}` subject) |
| OGAR | #168 | kausal Arm A+C (`lift_actions` → `KausalSpec::Depends`, `raises`) |
| OGAR | #169 | kausal Arm B+D wiring (post ruff #49) |
| OGAR | **#192** | **W1 kausal-parity pin** (real `account_payment_term.py` witness, 11/11 = 8 Depends + 3 Constrains) + **W2 V3 SoA sink** (`ogar-from-ruff::lance_sink`, feature `lance-sink`: `CompiledClass → FacetCascade / 512-B CANON NodeRow / as_le_bytes`, zero-dep contract, no `*Bridge`) + e2e capstone. Ledger: `D-KAUSAL-CONSUME-PIN-ODOO`, `D-V3-SINK-COMPILEDCLASS` |
| lance-graph | #669 | contract nav brick (`screens_reachable_from` / `nav_is_fully_connected`) |
| odoo-rs | #32 | transcode arm: Klickweg parity, view masks, kausal, corpus schema pins |
| odoo-rs | #34 | `od-server` Railway HTTP host (binds `0.0.0.0:$PORT`) |
| odoo-rs | **#35** | **region grammar Edit 3**: `ODOO-REGION-GRAMMAR.md` + `odoo_regions.conf` + `region_grammar.rs`/`region_digest.rs` incl. `live_harvest_reproduces_the_frozen_corpus` (the merged-main arm reproduces `data/nav/account_regions.spo.ndjson` byte-for-byte; no `unmapped:` leak over real source) |
| odoo-rs | #36 | `od-server` in-process **`/compile`** — Odoo `.py` → `CompiledClass` JSON, OGAR-V3-shaped |

**Oracles now standing:** value = PostgreSQL (facet-table DDL in
`ogar-adapter-postgres-ddl`); structure = Klickwege nav graph (field-set half
via `view_mask.rs`, **render half via region grammar**). The furnace's
two-oracle bar (`ruff/.claude/knowledge/consumer-transcode-furnace-playbook.md`
§2) is met in *harvest*; the render/storage *executors* are the remaining work.

## 2. State per repo (rebased + verified this session)

| Repo | main | Branch `claude/odoo-rs-v3-ogar-transpile-nwriny` | Verify |
|---|---|---|---|
| ruff | `6604f45` | (merged; no live branch) | — |
| OGAR | `df7331e` | reset to main | `cargo test -p ogar-from-ruff --features lance-sink` green |
| lance-graph | `e3e51dc` | reset to main (no unique commits — W2 lives OGAR-side by dependency direction) | — |
| odoo-rs | `09c62c1` | reset to main | `cargo test -p od-ontology --features cli,fieldmask` green |

## 3. What's NEXT (re-derived against TODAY's mains, not the 07-08 list)

The 07-08 §4 list is retired: W1 ✅, W3 ✅, od-hydrate host = **shipped
as `od-server`** (#34/#36). The live seams now:

1. **od-server hydration writer (the storage skins, now concretely seamed).**
   `/compile` produces `CompiledClass`; `main.rs` names
   `STORAGE_BACKEND=memory|postgres` and says *"the actual hydration writer
   is a follow-up."* Wire BOTH legs through existing, merged bricks — invent
   nothing:
   - **postgres**: `ogar-adapter-postgres-ddl::{emit_facet_table_ddl,
     emit_postgres_ddl}` (the 12-col SMALLINT facet table + per-class typed
     tables), tokio-postgres writes.
   - **lance**: `ogar_from_ruff::lance_sink` (OGAR #192, feature
     `lance-sink`): `compiled_class_to_noderow` → `NodeRowPacket::as_le_bytes`
     — the 512-B CANON rows. The `Dataset::write` I/O is the named
     out-of-scope tail of W2; this is where it lands. Consult OGAR
     `.claude/knowledge/hotplug-consumer-migration.md` (`HotPlug` const +
     `resolve_hotplug`) before making odoo-rs a capability *executor*.
2. **Region grammar × a2ui (the render skin, reframed).** OGAR's a2ui arc
   (#204 proposal, #206 `ogar-a2ui-frame`, #207 `ogar-render-askama`
   fieldview) rules that a screen is a **nested ClassView projection** —
   "address the screen, not the pixels." The Odoo feed for that projection
   is exactly this repo's region harvest: `docked_at`/`tab_order`/
   `opens_popup` facts + `odoo_regions.conf` + the `[regions]` digest. The
   seam: map the six-region frame onto the a2ui `X:Y` u8:u8 rail layout
   address (region → position in the parent ClassView's ordered set;
   `tab_order` → intra-region order). Do NOT build a standalone
   `LayoutFrame` first — check the a2ui frame crates; the render equation's
   `global_mask ∩ local_mask` is their `WideFieldMask` cast. ⚠ Heed the
   codex-P2 correction on #204/#205: RBAC masks must be **WIDE**
   (`>64`-field surfaces break narrow `FieldMask` u64) — use the wide-mask
   path (`render_class_with_methods_wide` lineage), never
   `ClassRbac::field_mask` narrow.
3. **`inherit_id` join for extension views.** `<xpath>` patch controls still
   dock at the honest `root` fallback; resolving their true region needs the
   parent-view join (mirror of `odoo_nav`'s cross-file Shape B). Unlocks
   correct regions for the analytic-line extension views in the committed
   corpus (the `root`-docked entries in `account_regions.spo.ndjson`).
4. **Scale the harvest.** The witnesses are one addon (`account`, vendored
   slices). The transpiler is proven; running the full-addon sweep
   (`extract` over `odoo/addons/account` and beyond → od-server `/compile`
   → storage) is now an execution task, not a design task.

## 4. Standing gotchas (carried forward + this session's additions)

- **D-NEVER-PIN-BUMP:** all cross-repo deps float on `branch="main"`;
  `Cargo.lock` gitignored here — `cargo update -p <crates>` then re-verify.
- **Push/PR mechanics:** odoo-rs/odoo push via the proxy remote; OGAR/ruff
  push via the PAT URL. PR creation: the GitHub **MCP `create_pull_request`
  tool works for ALL four repos** (proven: ruff #79, OGAR #192, odoo-rs #35)
  — raw `curl` POSTs to the API get classifier-blocked; don't fight it.
- **Merged-branch policy:** after a PR merges, the branch restarts from
  `origin/main` (same name, force-with-lease) — never stack on merged history.
- **fmt-churn discipline (burned once):** after staging your files, never
  blanket `git checkout -- <paths-you-staged-from>` to drop unrelated fmt
  churn — it reverted a regenerated corpus mid-commit once (caught by
  re-verification, amended). Discard churn with an explicit file list that
  excludes your staged targets, and re-run the tests after any restore.
- **Region corpus regeneration:** if the ruff arm intentionally changes,
  `live_harvest_reproduces_the_frozen_corpus` fails loudly — regenerate
  `data/nav/account_regions.spo.ndjson` from the live arm (a 10-line scratch
  example over `data/nav`), never hand-edit it.
- **Odoo upstream default branch is `19.0`, not `main`** (oracle repo only).
