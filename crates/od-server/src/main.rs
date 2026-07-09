//! `od-server` — HTTP host for the Odoo→OGAR transpiler frontend.
//!
//! Binds `0.0.0.0:$PORT` (the Railway / PaaS contract) and exposes the
//! `od-codegen` capability over HTTP by invoking the proven CLI as a
//! subprocess. The CLI is the contract: this host serves exactly what
//! `od-codegen` emits — classid resolution (`--classids`, default) and the
//! behavioural-arm action rows (`--actions`) — and cannot drift from it.
//!
//! ## Why a subprocess, not a library link
//!
//! `od-ontology`'s emit surfaces are in flux (SurrealQL emit was deprecated
//! 2026-07-06; the transpiler's SDK emitters live in OGAR, not odoo-rs). Rather
//! than couple this host to an internal API that is mid-migration, it shells to
//! the shipped `od-codegen` binary — the one interface that is stable and
//! tested. When odoo-rs grows an in-process `emit_*` / hydration API, swap the
//! subprocess for a direct call behind the same routes.
//!
//! ## Routes
//!
//! - `GET  /health`   — 200 `ok`; the Railway healthcheck target. Also reports
//!   the configured storage backend so a probe shows hydration intent.
//! - `GET  /`         — onboarding page: the env-var contract + storage choice.
//! - `POST /classids` — body = SPO-triple ndjson; returns `od-codegen --classids`
//!   output (table → OGAR render classid map).
//! - `POST /actions`  — body = SPO-triple ndjson; returns `od-codegen --actions`
//!   output (the ActionDef lowering rows).
//!
//! ## Storage contract (documented, not yet wired)
//!
//! `STORAGE_BACKEND` selects the intended sink for hydrated output —
//! `lance-graph` (native V3, the canonical sink) or `postgres` (the 3×SPOG
//! facet-table ORM shape). The actual hydration writer is a follow-up that
//! belongs with odoo-rs's storage arc; this host surfaces the choice and its
//! readiness so onboarding is honest about what is and isn't live.

use std::process::Stdio;

use axum::{
    body::Bytes,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// The `od-codegen` binary name. In the container it is on `PATH` (copied next
/// to `od-server`); locally, an absolute path can be supplied via `OD_CODEGEN`.
fn codegen_bin() -> String {
    std::env::var("OD_CODEGEN").unwrap_or_else(|_| "od-codegen".to_string())
}

fn storage_backend() -> String {
    std::env::var("STORAGE_BACKEND").unwrap_or_else(|_| "unset".to_string())
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(onboarding))
        .route("/classids", post(|body: Bytes| run_codegen("--classids", body)))
        .route("/actions", post(|body: Bytes| run_codegen("--actions", body)))
        // convenience: /codegen/<mode> for future modes without new routes
        .route("/codegen/:mode", post(codegen_mode));

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    eprintln!("od-server listening on {addr} (storage_backend={})", storage_backend());
    axum::serve(listener, app).await.expect("server error");
}

async fn health() -> impl IntoResponse {
    // 200 keeps the Railway healthcheck green; the body reports hydration intent.
    (
        StatusCode::OK,
        format!("ok\nstorage_backend={}\n", storage_backend()),
    )
}

async fn onboarding() -> Html<String> {
    let backend = storage_backend();
    Html(format!(
        "<!doctype html><meta charset=utf-8><title>od-server — Odoo→OGAR transpiler</title>\
         <h1>od-server</h1>\
         <p>HTTP host for the Odoo→OGAR transpiler frontend (<code>od-codegen</code>).</p>\
         <h2>Storage backend: <code>{backend}</code></h2>\
         <p>Set <code>STORAGE_BACKEND=lance-graph</code> (native V3, canonical sink) \
         or <code>STORAGE_BACKEND=postgres</code> (3×SPOG facet-table ORM shape). \
         Hydration writer is a follow-up; this host currently serves stateless transpile.</p>\
         <h2>Endpoints</h2>\
         <ul>\
         <li><code>GET  /health</code> — liveness (Railway healthcheck)</li>\
         <li><code>POST /classids</code> — SPO ndjson &rarr; table→classid map</li>\
         <li><code>POST /actions</code> — SPO ndjson &rarr; ActionDef rows</li>\
         </ul>\
         <p>Example: <code>curl --data-binary @account.spo.ndjson $URL/classids</code></p>"
    ))
}

async fn codegen_mode(Path(mode): Path<String>, body: Bytes) -> impl IntoResponse {
    // Only the two documented, exit-code-contracted modes are exposed.
    let flag = match mode.as_str() {
        "classids" => "--classids",
        "actions" => "--actions",
        _ => {
            return (
                StatusCode::NOT_FOUND,
                format!("unknown mode {mode:?}; use classids|actions"),
            )
                .into_response()
        }
    };
    run_codegen(flag, body).await.into_response()
}

/// Pipe `body` (SPO ndjson) into `od-codegen <flag> -` and return its stdout.
/// Maps `od-codegen`'s exit-code contract (#512) onto HTTP:
/// `0`→200, `2` (degenerate input)→422, anything else→500.
async fn run_codegen(flag: &str, body: Bytes) -> impl IntoResponse {
    let mut child = match Command::new(codegen_bin())
        .arg(flag)
        .arg("-") // read the corpus from stdin
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to spawn od-codegen: {e}"),
            )
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(&body).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to write corpus to od-codegen stdin: {e}"),
            );
        }
        // drop stdin → EOF so od-codegen finishes reading
    }

    let out = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("od-codegen wait failed: {e}"),
            )
        }
    };

    match out.status.code() {
        Some(0) => (StatusCode::OK, String::from_utf8_lossy(&out.stdout).into_owned()),
        // exit 2 = degenerate input (empty/malformed ndjson, zero triples) per #512
        Some(2) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ),
    }
}
