//! `od-posting` — GoBD-conformant host for Odoo `account.move._post`.
//!
//! This crate is the **HAND-PORT side** of the
//! [`../../od-ontology/specs/_post.md`] spec. It carries the intrusive
//! posting logic — the gapless Belegnummer, the inalterability hash chain,
//! and the transactional fence around state+freeze — that the projection
//! cannot lower into a thin `DEFINE FUNCTION` adapter without smuggling
//! state (Frankenstein guard).
//!
//! It exists as a **separate workspace crate** for two reasons.
//!
//! First — **zero-dep `od-ontology`**. The DDL projection is byte-stable
//! across target environments; pulling in a runtime `SurrealDB` client
//! plus tokio plus rocksdb would bend it. `od-posting` is allowed to be
//! heavy because it is *the runtime consumer*, not the ontology.
//!
//! Second — **council-resolved boundary**. See
//! [`DECISIONS/D-POST-SEQ.md`][seq] — Option C (HYBRID) routes the
//! immediate posting loop here while a parallel substrate PR proposes
//! a tx-enrolled `gapless DEFINE SEQUENCE` flag on the surrealdb fork.
//! The two paths are independent; this crate stands alone if the
//! substrate PR is rejected, and shrinks to a shim if it lands.
//!
//! [seq]: ../../od-ontology/specs/DECISIONS/D-POST-SEQ.md
//!
//! # The four invariants this crate exists to host
//!
//! Per `_post.md` § `preserves_invariant`:
//!
//! - **`single_tx_counter_create_hash_state`** (PP-15 — load-bearing):
//!   counter RMW, row CREATE, hash-computing `DEFINE EVENT`, and the
//!   state/`posted_before` write share ONE `BEGIN…COMMIT`. Any own-tx
//!   path for the counter (`nextval`, any helper that opens its own
//!   transaction) drops the pessimistic lock at the seam and reintroduces
//!   the gap-or-orphaned-number failure mode.
//! - **`chain_order_per_journal_sequence_prefix`** — hashes are chained
//!   per `(journal_id, sequence_prefix)`; the predecessor lookup uses
//!   the same tx snapshot the counter holds.
//! - **`append_only_no_update_delete_once_hashed`** — the projected table
//!   carries `Permissions { update: NONE, delete: NONE }` once posted;
//!   this host MUST refuse any post-hash mutation path.
//! - **`serialization_byte_exact`** — canonical row bytes use Odoo's
//!   `float_repr` for monetary fields, sorted-compact-ASCII JSON for the
//!   payload, and the `$4$` version prefix is stripped before chaining.
//!
//! # Skeleton-stage status
//!
//! This file is the **published trait surface only** — the actual
//! `BEGIN → pessimistic RMW → CREATE → COMMIT` runner is unimplemented.
//! The implementation is gated on:
//!
//! - disk-gated surrealdb fork build (4.5 GB free on this host, full
//!   `surrealdb-core` build is multi-GB);
//! - the `PROBE-POST-GAPLESS-PARITY` concurrency half (runnable today
//!   against `kvs/tests/multiwriter_same_keys_conflict.rs`, PP-16);
//! - the parity-half oracle (Odoo Python on a host that has it).
//!
//! Until those land, this crate compiles + provides a typed contract that
//! the future implementor must honour.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The intrusive operations a GoBD-conformant `_post` host owns,
/// surfaced as a typed boundary so the projection cannot accidentally
/// emit a DEFINE FUNCTION that claims to do them.
///
/// Per `_post.md` § "The three intrusive operations". An implementor
/// of [`PostingHost`] must guarantee that a single call to
/// [`PostingHost::post`] executes the counter RMW, row CREATE, hash
/// event, and state/freeze write inside ONE `BEGIN…COMMIT`. Splitting
/// any step across transactions is a contract violation — see the
/// `anti_mechanism` slot on `gapless_belegnummer` in the spec.
pub trait PostingHost {
    /// The host's error shape. Concrete implementors will wire this to
    /// their `SurrealDB` client error; the contract here only requires
    /// that a failed `post` left no committed row, no consumed counter
    /// value, and no dangling hash entry.
    type Error;

    /// Atomically: lock the per-`(journal_id, sequence_prefix)` counter
    /// key under `LockType::Pessimistic`, read-modify-write it, CREATE
    /// the row (firing the hash event in the same tx snapshot), set
    /// `state` to `posted` + `posted_before` to `true`, and COMMIT.
    ///
    /// On failure: ABORT the transaction. The counter MUST NOT have
    /// been observed by any other reader, and no row may have leaked.
    ///
    /// # Errors
    ///
    /// Returns `Self::Error` if any step (lock acquisition, RMW,
    /// CREATE, event firing, state write, COMMIT) fails. The contract
    /// is binary — partial posts are forged ledgers.
    fn post(&self, draft: &MoveDraft<'_>) -> Result<PostedMove, Self::Error>;
}

/// A draft `account.move` row, pre-`_post`. Field set mirrors `_post.md`'s
/// `do_in.reads_field` slot. Owned by the caller; the host borrows for
/// the duration of one `post` call.
#[derive(Debug)]
#[non_exhaustive]
pub struct MoveDraft<'a> {
    /// The journal this move belongs to. Drives the counter scope:
    /// every `(journal_id, sequence_prefix)` pair has its own counter
    /// key and its own hash chain.
    pub journal_id: &'a str,

    /// The sequence prefix (Odoo's per-fiscal-year / per-type prefix).
    /// Together with `journal_id` it identifies the contiguous range
    /// the assigned `name` must extend by exactly one.
    pub sequence_prefix: &'a str,

    /// The canonical row bytes the hash event will read. The implementor
    /// MUST NOT touch monetary `float_repr` formatting or JSON key order
    /// here — the bytes are the spec's `serialization_byte_exact` input.
    pub canonical_row: &'a [u8],
}

/// A row after `_post`. The `name` field is the gapless Belegnummer
/// the council resolved must come from the pessimistic-counter path,
/// never `DEFINE SEQUENCE` / `nextval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedMove {
    /// The assigned gapless Belegnummer (e.g. `INV/2026/0042`).
    pub name: String,

    /// The chained inalterability hash for this row.
    /// `crypto::sha256(prev.hash + canonical_row)`.
    pub inalterable_hash: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-only smoke test — confirms the trait surface is callable.
    /// Real implementors will live in downstream sites that own a
    /// `SurrealDB` client; this stub stays as a witness that the contract
    /// shape is stable.
    struct NoopHost;

    impl PostingHost for NoopHost {
        type Error = &'static str;

        fn post(&self, _draft: &MoveDraft<'_>) -> Result<PostedMove, Self::Error> {
            Err("od-posting: skeleton crate — no runtime host wired yet (see _post.md)")
        }
    }

    #[test]
    fn skeleton_trait_compiles() {
        let host = NoopHost;
        let draft = MoveDraft {
            journal_id: "j1",
            sequence_prefix: "INV/2026/",
            canonical_row: b"{}",
        };
        let outcome = host.post(&draft);
        assert!(outcome.is_err(), "skeleton must refuse — no client wired");
    }
}
