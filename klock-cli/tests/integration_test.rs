//! HTTP-level integration tests for the Klock CLI server.
//! Drives the Axum router via `tower::ServiceExt::oneshot` so we never
//! bind a real socket.
//!
//! Note: the auth middleware reads `KLOCK_API_KEY` from the process
//! environment, which is global. We guard every test with a single mutex
//! and clear / set the env var at the start so parallel test execution
//! cannot cross-contaminate.

use std::sync::{Arc, Mutex as StdMutex, MutexGuard};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use klock_cli::server::{build_router, AppState};
use klock_core::client::KlockClient;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower::ServiceExt;

static ENV_GUARD: StdMutex<()> = StdMutex::new(());

/// Acquire the global env mutex and reset auth state for the test.
/// Returns the guard so it lives until the end of the test.
fn no_auth_env() -> MutexGuard<'static, ()> {
    let g = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    unsafe {
        std::env::remove_var("KLOCK_API_KEY");
    }
    g
}

fn with_auth_key(key: &str) -> MutexGuard<'static, ()> {
    let g = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    unsafe {
        std::env::set_var("KLOCK_API_KEY", key);
    }
    g
}

fn fresh_state() -> AppState {
    Arc::new(Mutex::new(KlockClient::new()))
}

async fn read_json(body: Body) -> Value {
    let bytes = body.collect().await.expect("read body").to_bytes();
    serde_json::from_slice(&bytes).expect("parse body as JSON")
}

#[tokio::test]
async fn health_returns_200_when_store_is_healthy() {
    let _g = no_auth_env();
    let app = build_router(fresh_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("call");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response.into_body()).await;
    assert_eq!(body["data"]["status"], "ok");
    assert_eq!(body["data"]["active_leases"], 0);
}

#[tokio::test]
async fn register_then_acquire_then_release_roundtrip() {
    let _g = no_auth_env();
    let state = fresh_state();
    let app = build_router(state.clone());

    // Register
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agents")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"agent_id": "alpha", "priority": 100}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Acquire
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/leases")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": "alpha",
                        "session_id": "s1",
                        "resource_type": "FILE",
                        "resource_path": "/src/x.ts",
                        "predicate": "MUTATES",
                        "ttl": 60000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = read_json(resp.into_body()).await;
    assert_eq!(body["success"], true);
    let lease_id = body["data"]["lease_id"].as_str().unwrap().to_string();

    // Release
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/leases/{}", lease_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn conflicting_acquire_returns_409_die() {
    let _g = no_auth_env();
    let state = fresh_state();
    let app = build_router(state.clone());

    // Register two agents.
    for (id, priority) in &[("older", 100u64), ("younger", 200u64)] {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/agents")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"agent_id": id, "priority": priority}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Older acquires Mutates.
    let r1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/leases")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": "older",
                        "session_id": "s",
                        "resource_type": "FILE",
                        "resource_path": "/x",
                        "predicate": "MUTATES",
                        "ttl": 60000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);

    // Younger conflicts and must DIE -> 409.
    let r2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/leases")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": "younger",
                        "session_id": "s",
                        "resource_type": "FILE",
                        "resource_path": "/x",
                        "predicate": "MUTATES",
                        "ttl": 60000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
    let body = read_json(r2.into_body()).await;
    assert_eq!(body["reason"], "DIE");
}

#[tokio::test]
async fn payload_caps_reject_oversized_resource_path() {
    let _g = no_auth_env();
    let app = build_router(fresh_state());

    // 5000-byte resource_path > MAX_RESOURCE_PATH_LEN (4096).
    let oversized = "/".to_string() + &"a".repeat(5000);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/leases")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_id": "a",
                        "session_id": "s",
                        "resource_type": "FILE",
                        "resource_path": oversized,
                        "predicate": "MUTATES",
                        "ttl": 60000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn payload_caps_reject_oversized_agent_id() {
    let _g = no_auth_env();
    let app = build_router(fresh_state());

    let oversized_id = "a".repeat(300);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agents")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"agent_id": oversized_id, "priority": 100}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─── Auth tests ──────────────────────────────────────────────────────────────

async fn call(app: axum::Router, header: Option<&str>) -> StatusCode {
    let mut req = Request::builder()
        .method("POST")
        .uri("/agents")
        .header("content-type", "application/json");
    if let Some(h) = header {
        req = req.header("authorization", h);
    }
    let resp = app
        .oneshot(
            req.body(Body::from(
                json!({"agent_id": "a", "priority": 1}).to_string(),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    resp.status()
}

#[tokio::test]
async fn auth_missing_header_returns_401_when_key_set() {
    let _g = with_auth_key("supersecret");
    let app = build_router(fresh_state());
    let status = call(app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_wrong_token_returns_401() {
    let _g = with_auth_key("supersecret");
    let app = build_router(fresh_state());
    let status = call(app, Some("Bearer wrong")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_correct_token_allows_request() {
    let _g = with_auth_key("supersecret");
    let app = build_router(fresh_state());
    let status = call(app, Some("Bearer supersecret")).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn auth_lowercase_bearer_prefix_allowed() {
    let _g = with_auth_key("supersecret");
    let app = build_router(fresh_state());
    let status = call(app, Some("bearer supersecret")).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "Bearer prefix must be case-insensitive per RFC 7235"
    );
}
