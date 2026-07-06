//! View-stratum seam — mint `FieldMask`-ready projections from REAL Odoo
//! `ir.ui.view` records (E-MIRROR-EXTERNALIZATION, mirror of D-AR-3.5).
//!
//! # The seam
//!
//! Odoo externalizes its views as `ir.ui.view` XML records. A view is a
//! **field-projection of a model** — exactly what lance-graph's
//! `ClassView × FieldMask` expresses. This module is the *harvest side* of
//! that seam: it scans real view XML, splits the referenced fields into
//! this-model fields vs comodel hops, and mints the projection as plain
//! bit-words.
//!
//! `account.move` carries well over 64 fields, which is WHY the lance-graph
//! `FieldMask` >64 ceiling is lifted by `WideFieldMask` (lance-graph #651, MERGED
//! 2026-07-06): `Repr::Small(u64)`/`Repr::Wide(Box<[u64]>)`, bit N = logical field N.
//! [`MaskWords`] is deliberately **NOT** that contract type — it is the
//! harvest-side artifact the widened `FieldMask` will consume: plain
//! `Vec<u64>` bit-words, LSB-first within each word, word `i` covering
//! universe indices `[64·i, 64·i + 64)`.
//!
//! # Deliberately NOT an XML parser
//!
//! This is a hand-rolled tag scanner, not a general XML parser — zero new
//! deps by design. The Odoo view XML in-repo is well-formed, and this is a
//! harvest probe: the scanner handles exactly what real `ir.ui.view` records
//! contain (tags spanning lines, quoted attribute values, `<!-- -->`
//! comments, self-closing elements). It does not handle CDATA, processing
//! instructions, or malformed markup, and it does not need to.
//!
//! # Field stratification inside `arch`
//!
//! Inside a view's `<field name="arch">` payload, `<field name="X">`
//! elements come in two strata:
//!
//! * **top-level** — not nested inside another `<field>` element's subtree:
//!   a field of *this* view's model → [`ViewFields::fields`] (document
//!   order, first-occurrence dedup).
//! * **nested** — inside another field's subtree (e.g.
//!   `<field name="invoice_line_ids">` embedding `<field name="quantity"/>`):
//!   a field of the **comodel**, not this model →
//!   [`ViewFields::relation_hops`] as `(outer_field, inner_field)`
//!   (first-occurrence dedup, mirroring the `fields` rule).
//!
//! The `ir.ui.view` metadata fields themselves (`name`, `model`,
//! `inherit_id`, the `arch` wrapper, …) are record plumbing, never counted.

use std::collections::BTreeSet;

use crate::triple::{is_cross_record, member_of, model_of, Triple};

/// The field-projection harvested from ONE `ir.ui.view` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewFields {
    /// The view's `<field name="name">` text (e.g. `account.move.form`).
    pub view_name: String,
    /// The view's `<field name="model">` text (e.g. `account.move`).
    pub model: String,
    /// Top-level arch fields — fields of *this* model, document order,
    /// first-occurrence dedup.
    pub fields: Vec<String>,
    /// `(outer_field, inner_field)` pairs for fields nested inside another
    /// field's subtree — the inner field belongs to the outer field's
    /// comodel. First-occurrence dedup, document order.
    pub relation_hops: Vec<(String, String)>,
}

/// The minted projection bits — the harvest artifact the widened
/// lance-graph-contract `FieldMask` consumes. Deliberately NOT that type
/// (`WideFieldMask`, lance-graph #651 — merged; wiring `MaskWords` onto it is the
/// named follow-up once od-ontology gains the contract dep);
/// this is the wire-plain shape: LSB-first within each `u64`, word `i`
/// covers universe indices `[64·i, 64·i + 64)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskWords(pub Vec<u64>);

impl MaskWords {
    /// Number of set bits across all words.
    #[must_use]
    pub fn popcount(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    /// Whether universe index `i` is set. Out-of-range indices read as
    /// unset (the mask is conceptually zero-extended).
    #[must_use]
    pub fn is_set(&self, i: usize) -> bool {
        self.0
            .get(i / 64)
            .is_some_and(|w| (w >> (i % 64)) & 1 == 1)
    }
}

/// Mint the projection mask: bit `i` is set iff `universe[i]` ∈ `present`.
/// Yields `universe.len().div_ceil(64)` words (zero words for an empty
/// universe). Names in `present` that are not in `universe` simply don't
/// set a bit — the caller reports that gap, this function doesn't hide it.
#[must_use]
pub fn mint_mask(universe: &[String], present: &[String]) -> MaskWords {
    let present_set: BTreeSet<&str> = present.iter().map(String::as_str).collect();
    let mut words = vec![0u64; universe.len().div_ceil(64)];
    for (i, field) in universe.iter().enumerate() {
        if present_set.contains(field.as_str()) {
            if let Some(word) = words.get_mut(i / 64) {
                *word |= 1u64 << (i % 64);
            }
        }
    }
    MaskWords(words)
}

/// The model's ordered field universe from the SPO corpus: the member names
/// of `odoo:<model>.<field>` subjects typed `rdf:type ogit:Property`, in
/// sorted (`BTreeSet`) order. Dotted (cross-record) members are excluded —
/// the universe is the model's *direct* fields.
#[must_use]
pub fn field_universe(triples: &[Triple], model: &str) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for t in triples {
        if t.p == "rdf:type" && t.o == "ogit:Property" && model_of(&t.s) == model {
            if let Some(member) = member_of(&t.s) {
                if !is_cross_record(member) {
                    set.insert(member.to_string());
                }
            }
        }
    }
    set.into_iter().collect()
}

/// Harvest every `ir.ui.view` record in an XML document (or fragment) into
/// [`ViewFields`]. Records whose `model=` attribute is not `ir.ui.view` are
/// skipped wholesale.
#[must_use]
pub fn extract_view_fields(xml: &str) -> Vec<ViewFields> {
    let mut out = Vec::new();
    let mut cur: Option<RecordScan> = None;
    let mut pos = 0;

    while let Some(tok) = next_tag(xml, pos) {
        pos = tok.end;
        let tag = tok.text;

        let Some(rec) = cur.as_mut() else {
            if is_open(tag, "record")
                && !is_self_closing(tag)
                && attr_value(tag, "model") == Some("ir.ui.view")
            {
                cur = Some(RecordScan::default());
            }
            continue;
        };

        if rec.in_arch {
            if is_open(tag, "field") {
                let name = attr_value(tag, "name").unwrap_or_default().to_string();
                if let Some(outer) = rec.stack.last() {
                    let hop = (outer.clone(), name.clone());
                    if !rec.hops.contains(&hop) {
                        rec.hops.push(hop);
                    }
                } else if !rec.fields.contains(&name) {
                    rec.fields.push(name.clone());
                }
                if !is_self_closing(tag) {
                    rec.stack.push(name);
                }
            } else if is_close(tag, "field") && rec.stack.pop().is_none() {
                // The close with an empty stack is the arch wrapper's own.
                rec.in_arch = false;
            }
            continue;
        }

        // Outside arch: metadata fields + record close.
        if is_close(tag, "field") {
            if let Some((slot, text_start)) = rec.pending.take() {
                let text = xml
                    .get(text_start..tok.start)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                match slot {
                    MetaSlot::ViewName => rec.view_name = text,
                    MetaSlot::Model => rec.model = text,
                }
            }
        } else if is_open(tag, "field") {
            rec.pending = None;
            if !is_self_closing(tag) {
                match attr_value(tag, "name") {
                    Some("arch") => rec.in_arch = true,
                    Some("name") => rec.pending = Some((MetaSlot::ViewName, tok.end)),
                    Some("model") => rec.pending = Some((MetaSlot::Model, tok.end)),
                    _ => {}
                }
            }
        } else if is_close(tag, "record") {
            let done = cur.take().unwrap_or_default();
            out.push(ViewFields {
                view_name: done.view_name,
                model: done.model,
                fields: done.fields,
                relation_hops: done.hops,
            });
        } else {
            // Any other tag interrupts a pending simple-text capture.
            rec.pending = None;
        }
    }

    out
}

/// Which record-metadata text field a pending capture targets.
#[derive(Debug, Clone, Copy)]
enum MetaSlot {
    ViewName,
    Model,
}

/// In-flight scan state for one `ir.ui.view` record.
#[derive(Debug, Default)]
struct RecordScan {
    view_name: String,
    model: String,
    fields: Vec<String>,
    hops: Vec<(String, String)>,
    /// `<field>` nesting stack inside arch (names of open field elements).
    stack: Vec<String>,
    in_arch: bool,
    /// A metadata text capture in flight: which slot + the byte offset
    /// where the open tag ended (text runs from there to the close tag).
    pending: Option<(MetaSlot, usize)>,
}

/// One scanned tag: `[start, end)` byte span and the tag text including
/// both angle brackets. Comments are skipped, not yielded.
struct TagToken<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

/// Find the next tag at or after byte offset `from`. Skips `<!-- -->`
/// comments and honors quoted attribute values (a `>` inside quotes does
/// not terminate the tag). Returns `None` at end of input or on an
/// unterminated construct (well-formedness caveat above).
fn next_tag(xml: &str, mut from: usize) -> Option<TagToken<'_>> {
    let bytes = xml.as_bytes();
    loop {
        let start = from + xml.get(from..)?.find('<')?;
        if xml[start..].starts_with("<!--") {
            from = start + xml[start..].find("-->")? + 3;
            continue;
        }
        let mut quote: Option<u8> = None;
        let mut j = start + 1;
        while j < bytes.len() {
            let c = bytes[j];
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == b'"' || c == b'\'' {
                quote = Some(c);
            } else if c == b'>' {
                return Some(TagToken {
                    start,
                    end: j + 1,
                    text: &xml[start..=j],
                });
            }
            j += 1;
        }
        return None;
    }
}

/// Whether `tag` opens element `elem` (`<elem …>` / `<elem>` / `<elem/>`),
/// without matching longer names (`<field` must not match `<fieldset`).
fn is_open(tag: &str, elem: &str) -> bool {
    tag.strip_prefix('<')
        .and_then(|t| t.strip_prefix(elem))
        .is_some_and(|rest| rest.starts_with([' ', '\t', '\n', '\r', '/', '>']))
}

/// Whether `tag` closes element `elem` (`</elem>`).
fn is_close(tag: &str, elem: &str) -> bool {
    tag.strip_prefix("</")
        .and_then(|t| t.strip_prefix(elem))
        .is_some_and(|rest| rest.trim_start() == ">")
}

/// Whether `tag` is self-closing (`… />`).
fn is_self_closing(tag: &str) -> bool {
    tag.trim_end_matches('>').trim_end().ends_with('/')
}

/// The value of `attr="…"` inside a tag's text, requiring a whitespace
/// boundary before the attribute name so `name=` never matches
/// `inverse_name=`.
fn attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pat = format!("{attr}=\"");
    let mut search = 0;
    while let Some(rel) = tag.get(search..)?.find(&pat) {
        let pos = search + rel;
        let val_start = pos + pat.len();
        let bounded = tag[..pos]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
        if bounded {
            let len = tag.get(val_start..)?.find('"')?;
            return tag.get(val_start..val_start + len);
        }
        search = val_start;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = r#"
        <odoo>
          <record id="not_a_view" model="ir.actions.act_window">
            <field name="name">ignored</field>
          </record>
          <record id="v" model="ir.ui.view">
            <field name="name">demo.form</field>
            <field name="model">demo.model</field>
            <field name="arch" type="xml">
              <form>
                <!-- a comment with <field name="ghost"/> inside -->
                <field name="alpha"/>
                <field name="alpha"/>
                <field name="line_ids">
                  <list>
                    <field name="qty"/>
                    <field name="qty"/>
                  </list>
                </field>
                <field name="beta" invisible="state != 'draft'"/>
              </form>
            </field>
          </record>
        </odoo>
    "#;

    #[test]
    fn mini_view_parses_with_strata() {
        let views = extract_view_fields(MINI);
        assert_eq!(views.len(), 1, "non-view record must be skipped");
        let v = &views[0];
        assert_eq!(v.view_name, "demo.form");
        assert_eq!(v.model, "demo.model");
        assert_eq!(v.fields, vec!["alpha", "line_ids", "beta"]);
        assert_eq!(
            v.relation_hops,
            vec![("line_ids".to_string(), "qty".to_string())]
        );
    }

    #[test]
    fn mask_roundtrip_small() {
        let universe: Vec<String> = (0..70).map(|i| format!("f{i:02}")).collect();
        let present = vec!["f00".to_string(), "f63".to_string(), "f69".to_string()];
        let mask = mint_mask(&universe, &present);
        assert_eq!(mask.0.len(), 2);
        assert_eq!(mask.popcount(), 3);
        assert!(mask.is_set(0) && mask.is_set(63) && mask.is_set(69));
        assert!(!mask.is_set(1) && !mask.is_set(64) && !mask.is_set(9999));
    }

    #[test]
    fn attr_value_respects_word_boundary() {
        let tag = r#"<field inverse_name="move_id" name="line_ids">"#;
        assert_eq!(attr_value(tag, "name"), Some("line_ids"));
    }
}
