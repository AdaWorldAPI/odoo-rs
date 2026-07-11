# Odoo region grammar — the render half of the structure oracle

> **Knowledge transfer of ruff PR #76** (region-grammar plane, merged to
> `ruff` `main` @ `a0eb4988` / tip `48f2374`) **into the odoo-rs transcode.**
> PR #76 built the pattern for C# WinForms (`DockStyle`/`TabIndex`/
> `ContextMenuStrip`); the furnace playbook §8.1 says *"Odoo XML arch tags
> (`form`/`tree`/`kanban`) map to regions exactly like `DockStyle`."* This doc
> makes that mapping concrete and grounded, reusing the **same closed-vocab
> predicates** — no new predicate, no hand-rolled layout.
>
> **READ BY:** any odoo-rs session touching view rendering, the Klickweg
> render skin, or the six-region frame. Companion to
> `docs/knowledge/RUFF-SPO-SURFACE.md` (the predicate census) and the
> `klickweg_parity.rs` structure-oracle tests.
> **Cross-ref:** ruff `.claude/knowledge/consumer-transcode-furnace-playbook.md`
> §6 (the render side + render equation) and §8.1 (odoo portability row);
> ruff `crates/ruff_spo_triplet/src/{exam_config,nav_digest}.rs` (the
> `region=` directive + `[regions]` digest section).
> **Status:** KNOWLEDGE-TRANSFER — the vocab exists on ruff main (proof
> below); the Odoo harvester arm + odoo-rs digest section are the two
> remaining *three-edit-recipe* edits (§4), scoped but not yet built.

______________________________________________________________________

## 0. The pattern in one sentence (what #76 established)

**Every legacy screen is a declarative layout in disguise; harvest *where a
control docks* as a fact, then re-render it into ONE universal six-region
frame — never emulate the widget tree.** Three closed-vocab predicates carry
it, and a `region=` config (data, not Rust) names the mapping:

```
{ top_bar, left_nav, center, right_panel, bottom_bar, popup }
```

**Proof-of-read (ruff main `48f2374`):**
- `crates/ruff_spo_triplet/src/exam_config.rs:36-37` — the six canonical
  region names, verbatim: `top_bar / left_nav / right_panel / bottom_bar /
  center / popup`.
- `crates/ruff_spo_triplet/src/triple.rs:1159` —
  `fn predicate_count_locked_at_76()` (was 73; #76 added the three below).
- `crates/ruff_spo_triplet/src/nav_digest.rs:165,172,179` — the digest folds
  `docked_at` → region, `tab_order` → intra-region order, `opens_popup` →
  `→popup` suffix; unmapped dock tokens land in `unmapped:<token>` (never
  dropped).

The convention (playbook §6): `predicate → (region, order, interaction)`:

| Predicate (wire) | `Predicate` variant | Carries | WinForms source (#76) | **Odoo source (this doc)** |
|---|---|---|---|---|
| `docked_at` | `DockedAt` | **region** | `Dock = DockStyle.X` | **XML arch element** (see §2) |
| `tab_order` | `TabOrder` | **order within region** | `TabIndex` literal | **DOM position** in the arch |
| `opens_popup` | `OpensPopup` | **popup interaction** | `ContextMenuStrip = m` | **`act_window target="new"`** / dropdown (see §3) |

______________________________________________________________________

## 1. Why odoo-rs already has half of this

The Klickweg arc (`HANDOVER-2026-07-08-polyglot-transpiler.md` §2) closed the
**structure oracle** — but only its *value/field-set* half:

- `ruff_python_spo::odoo_views.rs` harvests **which fields** a view projects
  (`<field name="X"/>` inside `arch`) → `WideFieldMask` (the "does the Rust
  show the same fields?" question).
- `crates/od-ontology/src/view_mask.rs` is the local hop-aware refinement.

That answers *what is on the screen*. Region grammar answers the orthogonal
*where each thing sits* — the **render** half of the same structure oracle
(playbook §2 "structure parity" + §6 "the render side"). Same harvest source
(the view `arch`), same closed vocab, disjoint question. Together they are the
diverse-redundant structure witness; neither replaces the value oracle
(PostgreSQL rows).

______________________________________________________________________

## 2. The Odoo-arch → six-region map (grounded in the vendored `account` views)

Dock tokens are the **Odoo arch element name** (or a `class=`-qualified
variant); the region is assigned by config (§5), never hardcoded in Rust.
Grounded in the real arch tags present in `data/nav/*.xml`
(`account_account_views.xml` + `account_analytic_line_views.xml`): `form`,
`sheet`, `group`, `field`, `notebook`, `page`, `list`, `kanban`, `search`,
`filter`, `searchpanel`, `button`, `separator`, `div`.

| Odoo arch element | dock token | → region | Rationale |
|---|---|---|---|
| `<header>` (form action bar + `<field widget="statusbar">`) | `header` | **top_bar** | top workflow-button + status strip — the `DockStyle.Top` analogue |
| `<search>` root, `<filter>` | `search` | **top_bar** | the filter/search bar above a list/kanban |
| `<searchpanel>` | `searchpanel` | **left_nav** | Odoo's left category-filter rail (`DockStyle.Left`) |
| `<sheet>`, `<form>` body, `<group>`, `<field>`, `<separator>` | `sheet` | **center** | the main record body (`DockStyle.Fill`) |
| `<list>`/`<tree>`, `<kanban>` | `list` / `kanban` | **center** | the primary data view |
| `<notebook>` / `<page>` | `notebook` | **center** | tabbed sub-sections *within* center (order = page index) |
| chatter (`<div class="oe_chatter">`, `<chatter/>` in 17+) | `chatter` | **right_panel** | messaging/activity side panel (`DockStyle.Right`) |
| `<footer>` (wizard dialogs) | `footer` | **bottom_bar** | wizard action buttons at the dialog bottom (`DockStyle.Bottom`) |
| `<button>` opening `act_window target="new"`, dropdown/context menus | `action_menu` | **popup** | see §3 |

**`tab_order`** = the element's **DOM position** within the arch. The
`odoo_views.rs` scanner is already **position-ordered / full-text** (it was
rewritten from a line-based scan precisely so wrapped multi-line tags don't
leak — handover §2.1), so the same position index is the `tab_order` source
with zero new scan machinery. Ties break by dock token then control id
(mirrors `nav_digest`'s tie rule, `nav_digest.rs:111-113`).

**Faithful default:** preserve Odoo's arch order (muscle memory), exactly as
the nav order preserves legacy first-seen order (playbook §6). A
co-fire/optimistic reorder is an *engineer's-gate* candidate, never
auto-applied.

______________________________________________________________________

## 3. `opens_popup` in Odoo — two spellings

`docked_at`/`tab_order` fold cleanly; `opens_popup` needs Odoo's own popup
idioms (twin of WinForms `ContextMenuStrip`):

1. **Wizard dialog:** a `<button>` (or a menu action) whose `act_window`
   target is `"new"` opens a modal — the `opens_popup` OBJECT is that wizard
   view/action. This reuses the exact `act_window` harvest the Klickweg nav
   arm already parses (`odoo_nav.rs::extract_odoo_nav_edges`, Shape A + Shape
   B) — the popup edge is an `act_window` edge whose target is modal, not a
   navigation to a new screen.
2. **Dropdown / cog menu:** a `<button>` with a dropdown (`<button
   type="action">` grouped under a cog/`⚙` menu) → the control is the popup
   subject; the menu it opens is the object.

Per `nav_digest.rs:113-115`: the popup SUBJECT gets a `→popup` suffix in the
`[regions]` digest; the popup OBJECT (the menu/wizard it opens) is *not*
independently docked — it surfaces only through the interaction edge. Same
rule, Odoo tokens.

______________________________________________________________________

## 4. The three-edit recipe to build it (furnace playbook §5)

Region grammar is "fact in the closed vocab → arm that emits it → golden
section that diffs it." For Odoo, **Edit 1 is already done** (on ruff main);
Edits 2 and 3 remain:

- **Edit 1 — closed vocab: DONE.** `Predicate::{DockedAt, TabOrder,
  OpensPopup}` are on ruff `main` (`triple.rs`, count-lock 76). odoo-rs
  reuses them; **no predicate mint here** (mint gate untouched — region
  tokens are layout, not domain concepts, so they never earn a codebook
  classid; playbook §3 two-axis refusal).
- **Edit 2 — the harvester arm `[H]`:** extend
  `ruff_python_spo::odoo_views.rs` (or a sibling `odoo_regions.rs` arm) to
  emit, per view record: `docked_at(control, arch_token)`,
  `tab_order(control, dom_index)`, and `opens_popup(control, wizard|menu)` —
  the same `Triple` shape + provenance (`Authoritative`, f/c = 0.95/0.90) as
  the existing field-set arm. A neutral fixture (a synthetic `<form>` with a
  `<header>`, a `<searchpanel>`, a `<notebook>`, and a `target="new"` button)
  exercises all three. **This is ruff-side work** — filed as the odoo render
  arm, mirror of #76's WinForms arm.
- **Edit 3 — the digest section `[H]`:** wire odoo-rs's Klickweg digest to
  read the new facts into a `[regions]` golden section (controls grouped by
  `(screen, region)`, ordered by `tab_order`, `→popup` suffix) — the same
  shape `nav_digest.rs` emits. Lands next to `klickweg_parity.rs` as a pinned
  render-side test.

______________________________________________________________________

## 5. The sanctioned config (data, not Rust) — `data/nav/odoo_regions.conf`

Shipped alongside this doc: the `region=<dock_token>:<region_name>` rows that
map the §2 Odoo arch tokens onto the six canonical regions. This is a
**convention config** — one of the four furnace-sanctioned outputs (playbook
§4 #2), consumed by ruff's `exam_config` `region=` directive
(`exam_config.rs:68-70`), never a hand-drawn menu. Region names are free
strings *from config*; a session that wants a seven-region skin edits the
conf, not the code. A `region=` row with no `:` is dropped identically to
ruff (`exam_config.rs:80`), so a malformed row can't silently mis-dock.

______________________________________________________________________

## 6. The render equation (unchanged — inherited from #76 / playbook §6)

Once `docked_at`/`tab_order`/`opens_popup` are harvested, the odoo-rs render
skin is the **same masked projection** the playbook defines — no new struct:

```
live(R)   = region_basis[R] ∩ global_mask ∩ local_mask
render(R) = live(R).ordered_by(tab_order).as(interaction[opens_popup])
```

- `region_basis[R]` — every arch element the harvest placed in region `R`.
- `global_mask` — session-wide `reachable_routes ∩ RBAC(role)`. In Odoo this
  is `ir.ui.menu` + `ir.model.access`/record rules — the value the mint gate
  *refuses* to make a concept becomes the mask (playbook §3, `user_right →
  global_mask`).
- `local_mask` — the active view's own field narrowing (already produced by
  `view_mask.rs`'s `WideFieldMask` — **this is where the value half and the
  render half meet**: the field-set mask IS the per-screen `local_mask`).

The frame is a projection under two masks, not a wrapper struct. odoo-rs's
`LayoutFrame` (when built) is exactly this projection, proven by a
render→parse→re-derive test (the render-side twin of the PostgreSQL value
oracle).

______________________________________________________________________

## 7. Anti-patterns this transfer must not trip (playbook §10)

- **Emulating the Odoo widget tree.** Do not port `<form>`/`<notebook>`
  nesting verbatim into Rust widgets; harvest the dock facts and re-render
  into the six-region frame.
- **Minting a region token as a concept.** `top_bar`/`sheet`/`searchpanel`
  are layout, not domain concepts — method+structure but **no storage** (no
  PG table persists "the sheet region"). Two axes ⟹ refuse the mint; they
  stay `region=` config rows, never codebook classids.
- **Hand-rolling the region map in Rust.** It is `odoo_regions.conf` (§5). If
  you feel the urge to write a `match arch_tag { "header" => TopBar, … }` in
  Rust, that urge is the signal to extend the config / the ruff arm instead.
- **Corpus tokens in a public repo.** The arch-token→region *map* is neutral
  (generic Odoo vocabulary); any digest carrying real screen/field *names*
  stays in odoo-rs (private), never in ruff/OGAR/lance-graph.
