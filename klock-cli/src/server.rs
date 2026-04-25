use std::sync::Arc;
use tokio::sync::Mutex;

use axum::{
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
    Json, Router,
};
use tower_http::cors::CorsLayer;

use klock_core::client::KlockClient;
use klock_core::types::{LeaseFailureReason, LeaseResult};

use crate::handlers::*;

pub type AppState = Arc<Mutex<KlockClient>>;

/// Build the HTTP router with the supplied state. Pulled out of `run` so
/// integration tests can drive the router directly via `tower::oneshot`.
pub fn build_router(state: AppState) -> Router {
    // NOTE: Rate limiting should be handled at the infrastructure level
    // (nginx, envoy, cloud load balancer) for production deployments.

    Router::new()
        // Health is always open (no auth)
        .route("/health", get(health))
        // Protected routes
        .route("/agents", post(register_agent))
        .route("/leases", post(acquire_lease))
        .route("/leases", get(list_leases))
        .route("/leases/{id}", delete(release_lease))
        .route("/leases/{id}/heartbeat", post(heartbeat_lease))
        .route("/intents", post(declare_intent))
        .route("/evict", post(evict_expired))
        .layer(middleware::from_fn(auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run(host: &str, port: u16, storage: &str) {
    let client = create_client(storage);
    let state: AppState = Arc::new(Mutex::new(client));
    let app = build_router(state);

    let addr = format!("{}:{}", host, port);

    if std::env::var("KLOCK_API_KEY").is_ok() {
        tracing::info!("🔐 API key authentication enabled");
    } else {
        tracing::warn!("⚠️  No KLOCK_API_KEY set — server is open (dev mode)");
    }

    tracing::info!("🔒 Klock server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server error");
}

// ─── Auth Middleware ────────────────────────────────────────────────────────

async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // If no API key is configured, allow all requests (dev mode)
    let expected_key = match std::env::var("KLOCK_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => return Ok(next.run(request).await),
    };

    // Always allow health check without auth
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    // Check the Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = strip_bearer_prefix(auth_header);

    if tokens_match(token, &expected_key) {
        Ok(next.run(request).await)
    } else {
        tracing::warn!("🚫 Unauthorized request to {}", request.uri().path());
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Strip a case-insensitive `Bearer ` prefix from an Authorization header.
/// Returns the empty string if the header doesn't start with the scheme.
fn strip_bearer_prefix(header: &str) -> &str {
    const SCHEME: &str = "bearer ";
    if header.len() >= SCHEME.len() && header[..SCHEME.len()].eq_ignore_ascii_case(SCHEME) {
        &header[SCHEME.len()..]
    } else {
        ""
    }
}

/// Constant-time comparison for the bearer token.
/// Equal-length check first (subtle requires equal-length slices); if the
/// lengths differ we still iterate over the longer one to avoid leaking the
/// length of the expected key via early-exit timing.
fn tokens_match(received: &str, expected: &str) -> bool {
    use subtle::ConstantTimeEq;
    let r = received.as_bytes();
    let e = expected.as_bytes();
    if r.len() != e.len() {
        // Compare against itself to keep the operation length-stable.
        let _ = r.ct_eq(r);
        return false;
    }
    r.ct_eq(e).into()
}

// ─── Handlers ───────────────────────────────────────────────────────────────

async fn health(State(state): State<AppState>) -> (StatusCode, Json<ApiResponse<HealthResponse>>) {
    let client = state.lock().await;
    if client.storage_poisoned() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::ok(HealthResponse {
                status: "storage_unavailable".to_string(),
                active_leases: 0,
                version: env!("CARGO_PKG_VERSION").to_string(),
            })),
        );
    }
    (
        StatusCode::OK,
        Json(ApiResponse::ok(HealthResponse {
            status: "ok".to_string(),
            active_leases: client.get_active_leases().len(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })),
    )
}

async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentRequest>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    if req.agent_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::err("agent_id is required")),
        );
    }
    if let Err(e) = validate_len("agent_id", &req.agent_id, MAX_AGENT_ID_LEN) {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::err(e)));
    }

    let mut client = state.lock().await;
    client.register_agent(&req.agent_id, req.priority);
    tracing::info!(agent_id = %req.agent_id, priority = req.priority, "Agent registered");
    (
        StatusCode::CREATED,
        Json(ApiResponse::ok(format!(
            "Agent '{}' registered with priority {}",
            req.agent_id, req.priority
        ))),
    )
}

async fn acquire_lease(
    State(state): State<AppState>,
    Json(req): Json<AcquireLeaseRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate request
    if let Err(e) = req.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": e,
            })),
        );
    }

    let mut client = state.lock().await;
    let result = client.acquire_lease(
        &req.agent_id,
        &req.session_id,
        &req.resource_type,
        &req.resource_path,
        &req.predicate,
        req.ttl,
    );

    match result {
        LeaseResult::Success { lease } => {
            tracing::info!(
                agent_id = %req.agent_id,
                lease_id = %lease.id,
                resource = %format!("{}:{}", req.resource_type, req.resource_path),
                "Lease acquired"
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "success": true,
                    "data": {
                        "lease_id": lease.id,
                        "agent_id": lease.agent_id,
                        "resource": format!("{}:{}", req.resource_type, req.resource_path),
                        "predicate": req.predicate.to_uppercase(),
                        "expires_at": lease.expires_at,
                    }
                })),
            )
        }
        LeaseResult::Failure {
            reason, wait_time, ..
        } => {
            let (status, reason_str) = match reason {
                LeaseFailureReason::Wait => (StatusCode::CONFLICT, "WAIT"),
                LeaseFailureReason::Die => (StatusCode::CONFLICT, "DIE"),
                LeaseFailureReason::Conflict => (StatusCode::CONFLICT, "CONFLICT"),
                LeaseFailureReason::ResourceLocked => (StatusCode::CONFLICT, "RESOURCE_LOCKED"),
                LeaseFailureReason::SessionExpired => (StatusCode::CONFLICT, "SESSION_EXPIRED"),
                LeaseFailureReason::StorageUnavailable => {
                    (StatusCode::SERVICE_UNAVAILABLE, "STORAGE_UNAVAILABLE")
                }
            };
            tracing::info!(
                agent_id = %req.agent_id,
                reason = reason_str,
                "Lease denied"
            );
            (
                status,
                Json(serde_json::json!({
                    "success": false,
                    "reason": reason_str,
                    "wait_time": wait_time,
                })),
            )
        }
    }
}

async fn release_lease(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    let mut client = state.lock().await;
    if client.release_lease(&id) {
        tracing::info!(lease_id = %id, "Lease released");
        Json(ApiResponse::ok(format!("Lease '{}' released", id)))
    } else {
        Json(ApiResponse::<String>::err(format!(
            "Lease '{}' not found",
            id
        )))
    }
}

async fn heartbeat_lease(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<HeartbeatResponse>>) {
    let mut client = state.lock().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    if client.heartbeat_lease(&id, now) {
        tracing::info!(lease_id = %id, "Lease heartbeat renewed");
        (
            StatusCode::OK,
            Json(ApiResponse::ok(HeartbeatResponse {
                renewed: true,
                lease_id: id,
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err(format!(
                "Lease '{}' not found or expired",
                id
            ))),
        )
    }
}

async fn list_leases(State(state): State<AppState>) -> Json<ApiResponse<Vec<ActiveLeaseInfo>>> {
    let client = state.lock().await;
    let leases: Vec<ActiveLeaseInfo> = client
        .get_active_leases()
        .iter()
        .map(|l| ActiveLeaseInfo {
            id: l.id.clone(),
            agent_id: l.agent_id.clone(),
            resource: l.resource.key(),
            predicate: format!("{:?}", l.predicate),
            expires_at: l.expires_at,
        })
        .collect();
    Json(ApiResponse::ok(leases))
}

async fn declare_intent(
    State(state): State<AppState>,
    Json(req): Json<DeclareIntentRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Validate request
    if let Err(e) = req.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": e,
            })),
        );
    }

    let mut client = state.lock().await;

    // Build SPOTriples from the request
    let intents: Vec<klock_core::types::SPOTriple> = req
        .intents
        .iter()
        .map(|item| {
            let id = client.next_id();
            klock_core::types::SPOTriple {
                id,
                subject: req.agent_id.clone(),
                predicate: match item.predicate.to_uppercase().as_str() {
                    "PROVIDES" => klock_core::types::Predicate::Provides,
                    "CONSUMES" => klock_core::types::Predicate::Consumes,
                    "MUTATES" => klock_core::types::Predicate::Mutates,
                    "DELETES" => klock_core::types::Predicate::Deletes,
                    "DEPENDS_ON" => klock_core::types::Predicate::DependsOn,
                    "RENAMES" => klock_core::types::Predicate::Renames,
                    _ => klock_core::types::Predicate::Consumes, // validated above
                },
                object: klock_core::types::ResourceRef::new(
                    match item.resource_type.to_uppercase().as_str() {
                        "SYMBOL" => klock_core::types::ResourceType::Symbol,
                        "API_ENDPOINT" => klock_core::types::ResourceType::ApiEndpoint,
                        "DATABASE_TABLE" => klock_core::types::ResourceType::DatabaseTable,
                        "CONFIG_KEY" => klock_core::types::ResourceType::ConfigKey,
                        _ => klock_core::types::ResourceType::File,
                    },
                    &item.resource_path,
                ),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                confidence: klock_core::types::Confidence::High,
                session_id: req.session_id.clone(),
            }
        })
        .collect();

    let manifest = klock_core::state::IntentManifest {
        session_id: req.session_id,
        agent_id: req.agent_id,
        intents,
    };

    let verdict = client.declare_intent(&manifest);
    (StatusCode::OK, Json(serde_json::json!(verdict)))
}

async fn evict_expired(State(state): State<AppState>) -> Json<ApiResponse<EvictResponse>> {
    let mut client = state.lock().await;
    let evicted = client.evict_expired();
    tracing::info!(evicted = evicted, "Expired leases evicted");
    Json(ApiResponse::ok(EvictResponse { evicted }))
}

// ─── Storage Backend Selection ──────────────────────────────────────────────

pub fn create_client(storage: &str) -> KlockClient {
    if storage == "memory" {
        tracing::info!("💾 Storage backend: in-memory (leases will not persist)");
        KlockClient::new()
    } else if let Some(path) = storage.strip_prefix("sqlite:") {
        #[cfg(feature = "sqlite")]
        {
            tracing::info!("💾 Storage backend: SQLite ({})", path);
            match KlockClient::with_sqlite(path) {
                Ok(client) => client,
                Err(e) => {
                    tracing::error!("Failed to open SQLite: {}. Falling back to in-memory.", e);
                    KlockClient::new()
                }
            }
        }
        #[cfg(not(feature = "sqlite"))]
        {
            tracing::error!(
                "SQLite storage requested but `sqlite` feature is not enabled. \
                 Rebuild with: cargo build --features sqlite"
            );
            tracing::warn!("Falling back to in-memory storage.");
            let _ = path;
            KlockClient::new()
        }
    } else {
        tracing::error!(
            "Unknown storage backend: '{}'. Use 'memory' or 'sqlite:<path>'",
            storage
        );
        tracing::warn!("Falling back to in-memory storage.");
        KlockClient::new()
    }
}
