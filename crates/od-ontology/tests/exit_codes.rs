//! Exit-code regression — locks the `process::exit(2)` contract from
//! `od-codegen` against drift.
//!
//! Mirrors the discipline of:
//!
//! - **`lance-graph#512`** — `#[should_panic(expected = "…")]` to lock a
//!   precondition's failure message against drift.
//! - **`openproject-nexgen-rs` follow-up to #31** — spawn-the-binary
//!   equivalent of the above for a `process::exit`-based CLI.
//!
//! Each test spawns `od-codegen` via `env!("CARGO_BIN_EXE_od-codegen")`
//! (Cargo's built-in for binary integration tests — no `assert_cmd` dep),
//! feeds it a degenerate input, and asserts BOTH:
//!
//! 1. `status.code() == Some(2)` — the contract documented in the binary's
//!    module doc, citing `lance-graph#512`'s exit-0/1/2 convention.
//! 2. A literal stderr substring re-grepped from the real guard messages in
//!    `src/bin/od_codegen.rs`, so a future refactor that *renames the message*
//!    fails this test instead of silently passing.
//!
//! Linux-only via the `/dev/null` empty-stream stand-in for the first test
//! (CI is Linux; portability concern intentionally dropped). The other tests
//! pipe through stdin and are platform-independent.

#![cfg(feature = "cli")]

use std::io::Write;
use std::process::{Command, Stdio};

/// **Degenerate-INPUT guard** — empty triple stream → exit 2.
///
/// Locks the guard at `src/bin/od_codegen.rs` empty-triples branch. The
/// directive message names the upstream extractor (`odoo_blueprint_extractor`)
/// so a CI reader can fix the cause, not the symptom.
#[test]
fn empty_input_exits_2_with_zero_triples_message() {
    let out = Command::new(env!("CARGO_BIN_EXE_od-codegen"))
        .arg("/dev/null")
        .output()
        .expect("CARGO_BIN_EXE_od-codegen resolves under --features cli");

    assert_eq!(
        out.status.code(),
        Some(2),
        "empty input must exit code 2 (degenerate-input guard); got {:?}\n\
         stdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("input contains zero triples"),
        "expected literal substring `input contains zero triples` from the \
         degenerate-input guard; got:\n{stderr}",
    );
    // The directive message names the actual upstream tool so wrapper
    // scripts (and humans) know where to look.
    assert!(
        stderr.contains("odoo_blueprint_extractor"),
        "directive must name the upstream extractor; got:\n{stderr}",
    );
}

/// **Degenerate-OUTPUT guard** — focus set intersects no `ObjectType` rows
/// → exit 2.
///
/// Locks the guard at `src/bin/od_codegen.rs` zero-tables branch. A focus set
/// like `["nonexistent_model"]` against a real corpus produces zero tables;
/// rendering and writing an empty `.surql` file would silently ship a no-op
/// schema. The guard exits 2 with the literal `emitted zero tables`
/// substring + the rejected focus set so the cause is readable from CI logs.
#[test]
fn focus_matches_nothing_exits_2_with_zero_tables_message() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_od-codegen"))
        .arg("-")
        .args(["--focus", "nonexistent_model"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn od-codegen");

    // A real triple (so we're past the empty-input guard) but on a model
    // that won't match the focus set above.
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(
            br#"{"s":"odoo:account_move","p":"rdf:type","o":"ogit:ObjectType","f":1.0,"c":1.0}"#,
        )
        .expect("write triple to child stdin");

    let out = child.wait_with_output().expect("wait for od-codegen");

    assert_eq!(
        out.status.code(),
        Some(2),
        "focus-misses-everything must exit code 2 (degenerate-output guard); \
         got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("emitted zero tables"),
        "expected literal substring `emitted zero tables` from the \
         degenerate-output guard; got:\n{stderr}",
    );
    assert!(
        stderr.contains("nonexistent_model"),
        "rejected focus set must appear in the error message so the cause \
         is identifiable; got:\n{stderr}",
    );
}

/// **Degenerate-OUTPUT guard, `--actions` mode** — same focus-miss contract
/// as the default mode (codex P2 on #25): the W3 rewire early-returned from
/// the `--actions` arm before the zero-tables validation, so
/// `od-codegen --actions --focus <typo>` exited 0 with empty output instead
/// of the documented exit-2. This locks the restored guard so wrappers can
/// never silently accept a mistyped focus in either mode.
#[test]
fn actions_focus_matches_nothing_exits_2_with_zero_tables_message() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_od-codegen"))
        .arg("-")
        .args(["--actions", "--focus", "nonexistent_model"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn od-codegen");

    // A real triple (past the empty-input guard) on a model that won't match.
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(
            br#"{"s":"odoo:account_move","p":"rdf:type","o":"ogit:ObjectType","f":1.0,"c":1.0}"#,
        )
        .expect("write triple to child stdin");
    let out = child.wait_with_output().expect("wait for od-codegen");

    assert_eq!(
        out.status.code(),
        Some(2),
        "--actions focus-miss must exit code 2 exactly like the default mode; \
         got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("emitted zero tables") && stderr.contains("nonexistent_model"),
        "expected the zero-tables message naming the rejected focus set; got:\n{stderr}",
    );
}

/// **Malformed-input guard** — non-JSON line → exit 2.
///
/// The parser failure is a degenerate-input cause (corrupted corpus, not a
/// generic I/O error), so it shares exit 2 with the other two guards.
#[test]
fn malformed_ndjson_exits_2_with_parse_error_message() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_od-codegen"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn od-codegen");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(b"NOT JSON AT ALL")
        .expect("write garbage to child stdin");
    let out = child.wait_with_output().expect("wait for od-codegen");

    assert_eq!(
        out.status.code(),
        Some(2),
        "malformed ndjson must exit 2; got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error parsing triple ndjson"),
        "expected literal substring `error parsing triple ndjson`; got:\n{stderr}",
    );
}

/// **Happy-path** — a single-table fixture round-trips through the CLI.
///
/// Locks the exit-0 case so a refactor that mistakenly returned non-zero on
/// success would be caught. The default output mode is `--classids` (the
/// DDL-emit default was deleted with the `SurrealQL` fork — see
/// `docs/W3.3-DELETE-GATE-MATRIX.md`), so the happy path now asserts the
/// `table <TAB> render-classid` row instead of a `DEFINE TABLE` line.
#[test]
fn single_table_corpus_succeeds_and_prints_classid() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_od-codegen"))
        .arg("-")
        .args(["--focus", "account_move"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn od-codegen");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(
            br#"{"s":"odoo:account_move","p":"rdf:type","o":"ogit:ObjectType","f":1.0,"c":1.0}"#,
        )
        .expect("write triple to child stdin");
    let out = child.wait_with_output().expect("wait for od-codegen");

    assert_eq!(
        out.status.code(),
        Some(0),
        "happy-path must exit 0; got {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("account_move\t0x02020002"),
        "happy-path output missing the expected classid row:\n{stdout}",
    );
}
