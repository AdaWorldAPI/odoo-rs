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
use axum::Json;
use serde::Serialize;
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

/// The router — extracted so integration tests can drive it with `oneshot`
/// (no bind, no subprocess for the in-process `/compile` path).
fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(onboarding))
        .route("/classids", post(|body: Bytes| run_codegen("--classids", body)))
        .route("/actions", post(|body: Bytes| run_codegen("--actions", body)))
        // convenience: /codegen/<mode> for future modes without new routes
        .route("/codegen/:mode", post(codegen_mode))
        // in-process OGAR-V3 landing: Odoo .py source -> CompiledClass JSON
        .route("/compile", post(compile))
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let app = app();

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

// ── in-process OGAR-V3 landing ─────────────────────────────────────────────

/// One compiled class as it lands on the OGAR-V3 substrate — the JSON view of a
/// [`od_ontology`]-produced `CompiledClass`. `classid` is the 32-bit render
/// address (`concept‖app`, canon-high); `attributes`/`associations`/`actions`
/// are the THINK-arm structure + DO-arm counts that travel under that one
/// address. No SurrealQL — the AST landed OGAR-V3-shaped.
#[derive(Serialize)]
struct CompiledClassDto {
    name: String,
    classid: String,
    attributes: usize,
    associations: usize,
    actions: usize,
}

#[derive(Serialize)]
struct CompileResponse {
    count: usize,
    storage_backend: String,
    classes: Vec<CompiledClassDto>,
}

/// `POST /compile` — body = Odoo model `.py` source. Runs the in-process
/// `compile_source` (`ruff_python_spo` parse + `compile_graph_python::<OdooPort>`
/// lift+mint) and returns the OGAR-V3 landing as JSON. Source that fails to
/// parse contributes nothing (ruff's silent-skip invariant), so an empty class
/// list means "no Odoo model found", reported as 422 to match the corpus
/// routes' degenerate-input contract.
async fn compile(body: Bytes) -> impl IntoResponse {
    let src = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "request body must be UTF-8 .py source").into_response()
        }
    };

    let classes: Vec<CompiledClassDto> = od_ontology::compile_source(src)
        .into_iter()
        .map(|cc| CompiledClassDto {
            name: cc.class.name,
            classid: format!("0x{:08x}", cc.facet.facet_classid()),
            attributes: cc.class.attributes.len(),
            associations: cc.class.associations.len(),
            actions: cc.actions.len(),
        })
        .collect();

    if classes.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "no Odoo model compiled from source (unparseable or no model class)",
        )
            .into_response();
    }

    Json(CompileResponse {
        count: classes.len(),
        storage_backend: storage_backend(),
        classes,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::app;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt; // for `oneshot`

    const ACCOUNT_MOVE: &str = "\
from odoo import models, fields
class AccountMove(models.Model):
    _name = 'account.move'
    name = fields.Char()
    partner_id = fields.Many2one('res.partner')
    line_ids = fields.One2many('account.move.line', 'move_id')
    def _compute_amount(self):
        self.amount_total = 0
";

    async fn post(uri: &str, body: &str) -> (StatusCode, String) {
        let resp = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The in-process OGAR-V3 landing: real Odoo `.py` → `CompiledClass` JSON,
    /// addressed by the canon-high render classid. `account.move` → `0x02020002`.
    #[tokio::test]
    async fn compile_lands_account_move_on_the_v3_classid() {
        let (status, body) = post("/compile", ACCOUNT_MOVE).await;
        assert_eq!(status, StatusCode::OK, "body: {body}");
        // the OGAR-V3 render address for account.move (concept 0x0202 ‖ Odoo app 0x0002)
        assert!(body.contains("\"0x02020002\""), "classid missing: {body}");
        assert!(body.contains("\"name\":\"account_move\""), "name missing: {body}");
        // 2 associations (partner_id, line_ids) + 1 action (_compute_amount)
        assert!(body.contains("\"associations\":2"), "assoc count: {body}");
        assert!(body.contains("\"actions\":1"), "action count: {body}");
    }

    /// Non-model source compiles nothing → 422, matching the corpus routes'
    /// degenerate-input contract.
    #[tokio::test]
    async fn compile_rejects_non_model_source() {
        let (status, _) = post("/compile", "print('not a model')").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// Non-UTF-8 body → 400.
    #[tokio::test]
    async fn compile_rejects_non_utf8() {
        let resp = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/compile")
                    .body(Body::from(vec![0xff, 0xfe, 0x00]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
