//! C3 method-resolution order over the corpus's `inherits_from` DAG — the
//! substrate for OGAR falsifier **F1** ("Delegation ≡ Odoo `_inherit`",
//! `OGAR/docs/INTEGRATION-MAP.md` §6).
//!
//! # Why this exists NEXT TO [`crate::InheritanceMap`]
//!
//! [`crate::InheritanceMap`] stores each model's bases in a `BTreeSet`, which
//! **destroys the declaration order** of the `_inherit` bases — fine for its
//! own consumer ("which mixins' definitions union into this model's table"),
//! fatal for method *resolution*: Odoo's `_build_model` assembles bases in
//! `LastOrderedSet` declaration/install order and Python's C3 runs over THAT
//! tuple, so the base ORDER is load-bearing. This module therefore keeps
//! **first-occurrence declaration order** (`Vec` + seen-set dedup, NOT a
//! `BTreeSet`) and implements both resolutions the F1 falsifier compares:
//!
//! - [`Mro::resolve_c3`] — the CORRECT one: first definer along the Python C3
//!   linearization of the declaration-order base tuple.
//! - [`Mro::resolve_naive_dfs`] — the WRONG-but-tempting one: parent-first
//!   pre-order DFS over the declaration-order bases. On single chains it
//!   agrees with C3; on diamonds it does not — D(B,C), B(A), C(A) with the
//!   method on A and C resolves to **C** under C3 (`[D, B, C, A]`) but to
//!   **A** under naive DFS (`[D, B, A, C]` visit order). That divergence is
//!   the falsification arm of gate F1: a delegation chain built
//!   parent-first-DFS is NOT ≡ Odoo `_inherit`.
//!
//! The named fix if orders diverge in a real corpus is `Class.mixins`
//! *ordering* carrying the linearization — not the delegation walk itself.

use crate::triple::{strip_ns, Triple};
use std::collections::{BTreeMap, BTreeSet};

/// Linearization failure over the `inherits_from` graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MroError {
    /// The C3 merge stalled: no candidate head appears in no other list's
    /// tail while non-empty lists remain — the hierarchy admits no
    /// consistent linearization (Python would raise `TypeError: Cannot
    /// create a consistent method resolution order`).
    Inconsistent {
        /// The class whose linearization was requested.
        class: String,
    },
    /// The `inherits_from` graph cycles through this class.
    Cycle {
        /// The class re-entered while its own linearization was in progress.
        class: String,
    },
}

impl std::fmt::Display for MroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inconsistent { class } => {
                write!(f, "no consistent C3 linearization for `{class}`")
            }
            Self::Cycle { class } => {
                write!(f, "inherits_from cycle through `{class}`")
            }
        }
    }
}

impl std::error::Error for MroError {}

/// Declaration-order inheritance graph + per-class own-method index, lifted
/// from `inherits_from` / `has_function` triples.
#[derive(Debug, Default, Clone)]
pub struct Mro {
    /// child → declared bases in FIRST-OCCURRENCE declaration order.
    bases: BTreeMap<String, Vec<String>>,
    /// class → method NAMES it defines itself (the member segment after the
    /// last dot of each `has_function` object IRI).
    own: BTreeMap<String, BTreeSet<String>>,
    /// Every class seen: children, bases, and method owners.
    classes: BTreeSet<String>,
}

impl Mro {
    /// Build from a triple slice. `inherits_from` edges keep the order in
    /// which they occur in the corpus (first occurrence wins; duplicates are
    /// dropped without reordering); `has_function` objects contribute the
    /// owning class's own method names. Non-bare-model endpoints (IRIs with a
    /// member dot in a class position) are skipped, mirroring
    /// [`crate::InheritanceMap`].
    #[must_use]
    pub fn from_triples(triples: &[Triple]) -> Self {
        let mut bases: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut own: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut classes: BTreeSet<String> = BTreeSet::new();

        for t in triples {
            match t.p.as_str() {
                "inherits_from" => {
                    let (Some(child), Some(base)) = (bare_model(&t.s), bare_model(&t.o)) else {
                        continue;
                    };
                    classes.insert(child.to_string());
                    classes.insert(base.to_string());
                    let declared = bases.entry(child.to_string()).or_default();
                    if !declared.iter().any(|b| b == base) {
                        declared.push(base.to_string());
                    }
                }
                "has_function" => {
                    let Some(owner) = bare_model(&t.s) else {
                        continue;
                    };
                    let Some((_, name)) = strip_ns(&t.o).rsplit_once('.') else {
                        continue;
                    };
                    classes.insert(owner.to_string());
                    own.entry(owner.to_string())
                        .or_default()
                        .insert(name.to_string());
                }
                _ => {}
            }
        }

        Self {
            bases,
            own,
            classes,
        }
    }

    /// Every class seen in the corpus, in sorted order.
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.classes.iter().map(String::as_str)
    }

    /// The declared bases of `class` in first-occurrence declaration order.
    #[must_use]
    pub fn declared_bases(&self, class: &str) -> &[String] {
        match self.bases.get(class) {
            Some(v) => v,
            None => &[],
        }
    }

    /// Method NAMES `class` defines itself.
    pub fn own_method_names(&self, class: &str) -> impl Iterator<Item = &str> {
        self.own.get(class).into_iter().flatten().map(String::as_str)
    }

    /// Python C3 linearization over the DECLARATION-ORDER base tuple:
    /// `L(C) = C ++ merge(L(B1), …, L(Bn), [B1…Bn])`, where merge repeatedly
    /// takes the head of the first list whose head appears in no other
    /// list's *tail*.
    ///
    /// # Errors
    ///
    /// [`MroError::Inconsistent`] when the merge stalls with lists remaining;
    /// [`MroError::Cycle`] when `inherits_from` cycles through the class.
    pub fn linearize(&self, class: &str) -> Result<Vec<String>, MroError> {
        let mut memo: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut visiting: BTreeSet<String> = BTreeSet::new();
        self.linearize_memo(class, &mut memo, &mut visiting)
    }

    fn linearize_memo(
        &self,
        class: &str,
        memo: &mut BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Vec<String>, MroError> {
        if let Some(done) = memo.get(class) {
            return Ok(done.clone());
        }
        if !visiting.insert(class.to_string()) {
            return Err(MroError::Cycle {
                class: class.to_string(),
            });
        }
        let declared = self.declared_bases(class);
        let mut lists: Vec<Vec<String>> = Vec::with_capacity(declared.len() + 1);
        for base in declared {
            lists.push(self.linearize_memo(base, memo, visiting)?);
        }
        lists.push(declared.to_vec());
        let mut out = vec![class.to_string()];
        out.extend(c3_merge(lists, class)?);
        visiting.remove(class);
        memo.insert(class.to_string(), out.clone());
        Ok(out)
    }

    /// The CORRECT resolution: the first class along the C3 linearization of
    /// `class` that itself defines `method_name`. `None` when no class in the
    /// linearization defines it, or when the hierarchy has no consistent
    /// linearization.
    #[must_use]
    pub fn resolve_c3(&self, class: &str, method_name: &str) -> Option<&str> {
        let lin = self.linearize(class).ok()?;
        for c in &lin {
            if let Some((definer, methods)) = self.own.get_key_value(c.as_str()) {
                if methods.contains(method_name) {
                    return Some(definer.as_str());
                }
            }
        }
        None
    }

    /// The WRONG resolution the F1 falsifier exposes: pre-order DFS over the
    /// declaration-order bases (self first, then each base's subtree
    /// left-to-right, visited-set guarded), first definer wins. Agrees with
    /// C3 on diamond-free chains; on a diamond it reaches the shared apex
    /// through the FIRST base's subtree before ever visiting the second base.
    #[must_use]
    pub fn resolve_naive_dfs(&self, class: &str, method_name: &str) -> Option<&str> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        self.dfs(class, method_name, &mut visited)
    }

    fn dfs<'a>(
        &'a self,
        class: &str,
        method_name: &str,
        visited: &mut BTreeSet<String>,
    ) -> Option<&'a str> {
        if !visited.insert(class.to_string()) {
            return None;
        }
        if let Some((definer, methods)) = self.own.get_key_value(class) {
            if methods.contains(method_name) {
                return Some(definer.as_str());
            }
        }
        for base in self.declared_bases(class) {
            if let Some(hit) = self.dfs(base, method_name, visited) {
                return Some(hit);
            }
        }
        None
    }
}

/// C3 merge: repeatedly take the head of the first list whose head appears in
/// no other list's tail; error if no head qualifies while lists remain.
fn c3_merge(mut lists: Vec<Vec<String>>, class: &str) -> Result<Vec<String>, MroError> {
    let mut out: Vec<String> = Vec::new();
    loop {
        lists.retain(|l| !l.is_empty());
        if lists.is_empty() {
            return Ok(out);
        }
        let good_head = lists.iter().find_map(|l| {
            let head = &l[0];
            let in_a_tail = lists
                .iter()
                .any(|other| other.len() > 1 && other[1..].contains(head));
            if in_a_tail {
                None
            } else {
                Some(head.clone())
            }
        });
        let Some(head) = good_head else {
            return Err(MroError::Inconsistent {
                class: class.to_string(),
            });
        };
        for l in &mut lists {
            l.retain(|x| x != &head);
        }
        out.push(head);
    }
}

/// `odoo:<model>` → `Some("model")`; IRIs with a member dot are not models.
fn bare_model(iri: &str) -> Option<&str> {
    let local = strip_ns(iri);
    if local.contains('.') {
        None
    } else {
        Some(local)
    }
}
