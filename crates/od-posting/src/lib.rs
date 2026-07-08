//! `od-posting` — GoBD-conformant host for Odoo `account.move._post`.
//!
//! This crate is the **HAND-PORT side** of the
//! [`../../od-ontology/specs/_post.md`] spec. It carries the intrusive
//! posting logic — the gapless Belegnummer, the inalterability hash chain,
//! and the transactional fence around state+freeze — that the projection
//! cannot lower into a thin declarative adapter without smuggling state
//! (Frankenstein guard).
//!
//! It exists as a **separate workspace crate** for two reasons.
//!
//! First — **zero-dep `od-ontology`**. The DDL projection is byte-stable
//! across target environments; pulling in a runtime `PostgreSQL` client
//! plus tokio would bend it. `od-posting` is allowed to be heavy because
//! it is *the runtime consumer*, not the ontology.
//!
//! Second — **council-resolved boundary, retargeted to the V3 storage
//! matrix**. See [`DECISIONS/D-POST-SEQ.md`][seq] — the gapless-counter
//! mechanism (single-transaction pessimistic row lock) targets
//! **`PostgreSQL`** as System-of-Record: the facet table emitted by
//! `ogar-adapter-postgres-ddl::emit_facet_table_ddl` (columns `p0..p11`
//! = 3×SPOG), with `lance-graph` V3 as the read hot-path over the same
//! data. This crate is the runtime consumer against that PG table; there
//! is no parallel substrate-fork proposal anymore (the original
//! `surrealdb`-fork `DEFINE SEQUENCE` flag idea is dead — see D-POST-SEQ).
//!
//! *(council Q5, 2026-07-08): retargeted from deprecated `SurrealQL` to the
//! V3 storage matrix (PG facet `SoR` + lance-graph V3 read path). `SurrealQL`
//! is ABSOLUTELY DEPRECATED per operator directive 2026-07-06 — "OGAR V3
//! for transpile substrate, lance-graph V3 for database." The 4
//! invariants and the `PostingHost` trait API are UNCHANGED; only the
//! storage vocabulary below is retargeted.*
//!
//! [seq]: ../../od-ontology/specs/DECISIONS/D-POST-SEQ.md
//!
//! # The four invariants this crate exists to host
//!
//! Per `_post.md` § `preserves_invariant`:
//!
//! - **`single_tx_counter_create_hash_state`** (PP-15 — load-bearing):
//!   counter RMW, row INSERT, hash computation, and the
//!   state/`posted_before` write share ONE `PostgreSQL` `BEGIN…COMMIT`.
//!   The counter RMW is a `SELECT … FOR UPDATE` on the counter row —
//!   this pessimistic row lock is what serializes concurrent posters.
//!   Any own-tx path for the counter (a bare `nextval()` on a PG
//!   `SEQUENCE`, or any helper that opens its own transaction) drops the
//!   lock at the seam and reintroduces the gap-or-orphaned-number
//!   failure mode.
//! - **`chain_order_per_journal_sequence_prefix`** — hashes are chained
//!   per `(journal_id, sequence_prefix)`; the predecessor lookup uses
//!   the same tx snapshot the counter-row lock holds.
//! - **`append_only_no_update_delete_once_hashed`** — the PG facet table
//!   is append-only once posted: `REVOKE UPDATE, DELETE` on the table
//!   from the application role, reinforced by a `BEFORE UPDATE OR DELETE`
//!   trigger that raises once `posted_before = true`; this host MUST
//!   refuse any post-hash mutation path regardless.
//! - **`serialization_byte_exact`** — canonical row bytes use Odoo's
//!   `float_repr` for monetary fields, sorted-compact-ASCII JSON for the
//!   payload, and the `$4$` version prefix is stripped before chaining.
//!   Storage-agnostic — unaffected by the PG retarget.
//!
//! # Skeleton-stage status
//!
//! This file is the **published trait surface only** — the actual
//! `BEGIN → SELECT … FOR UPDATE (counter row) → INSERT → COMMIT` runner
//! against `PostgreSQL` is unimplemented. The implementation is gated on:
//!
//! - a wired `PostgreSQL` client (the facet-table DDL comes from
//!   `ogar-adapter-postgres-ddl::emit_facet_table_ddl`; the counter table
//!   + row-lock path is new, purpose-built for this host);
//! - the `PROBE-POST-GAPLESS-PARITY` concurrency half (same-row `SELECT
//!   … FOR UPDATE` contention under K concurrent posters + one mid-batch
//!   abort — testable against a real `PostgreSQL` instance);
//! - the parity-half oracle (Odoo Python on a host that has it).
//!
//! Until those land, this crate compiles + provides a typed contract that
//! the future implementor must honour.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// The intrusive operations a GoBD-conformant `_post` host owns,
/// surfaced as a typed boundary so the projection cannot accidentally
/// emit a declarative adapter that claims to do them.
///
/// Per `_post.md` § "The three intrusive operations". An implementor
/// of [`PostingHost`] must guarantee that a single call to
/// [`PostingHost::post`] executes the counter RMW, row INSERT, hash
/// computation, and state/freeze write inside ONE `PostgreSQL`
/// `BEGIN…COMMIT`. Splitting any step across transactions is a contract
/// violation — see the `anti_mechanism` slot on `gapless_belegnummer` in
/// the spec.
pub trait PostingHost {
    /// The host's error shape. Concrete implementors will wire this to
    /// their `PostgreSQL` client error (e.g. `tokio_postgres::Error`); the
    /// contract here only requires that a failed `post` left no
    /// committed row, no consumed counter value, and no dangling hash
    /// entry.
    type Error;

    /// Atomically: lock the per-`(journal_id, sequence_prefix)` counter
    /// row via `SELECT … FOR UPDATE` (the pessimistic row lock), read-
    /// modify-write it, INSERT the row into the PG facet table (computing
    /// the inalterability hash in the same tx snapshot — application-side
    /// or via a `BEFORE INSERT` trigger), set `state` to `posted` +
    /// `posted_before` to `true`, and COMMIT.
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

    /// The canonical row bytes the hash computation will read. The
    /// implementor MUST NOT touch monetary `float_repr` formatting or
    /// JSON key order here — the bytes are the spec's
    /// `serialization_byte_exact` input.
    pub canonical_row: &'a [u8],
}

/// A row after `_post`. The `name` field is the gapless Belegnummer
/// the council resolved must come from the pessimistic-counter path —
/// a `SELECT … FOR UPDATE` row lock on `PostgreSQL`, never a bare
/// `SEQUENCE`/`nextval()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedMove {
    /// The assigned gapless Belegnummer (e.g. `INV/2026/0042`).
    pub name: String,

    /// The chained inalterability hash for this row.
    /// `sha256(prev.hash + canonical_row)`.
    pub inalterable_hash: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-only smoke test — confirms the trait surface is callable.
    /// Real implementors will live in downstream sites that own a
    /// `PostgreSQL` client; this stub stays as a witness that the contract
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
