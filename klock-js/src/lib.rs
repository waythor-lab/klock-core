#![deny(clippy::all)]

use napi_derive::napi;

use klock_core::client::KlockClient as RustClient;
use klock_core::types::{LeaseResult as RustLeaseResult, LeaseFailureReason};

// ─── JS-facing KlockClient ─────────────────────────────────────────────────

#[napi]
pub struct KlockClient {
    inner: RustClient,
}

#[napi]
impl KlockClient {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: RustClient::new(),
        }
    }

    /// Register an agent with a priority (lower = older = higher priority).
    #[napi]
    pub fn register_agent(&mut self, agent_id: String, priority: f64) {
        self.inner.register_agent(&agent_id, priority as u64);
    }

    /// Acquire a lease on a resource.
    /// Returns a JSON string with the result.
    #[napi]
    pub fn acquire_lease(
        &mut self,
        agent_id: String,
        session_id: String,
        resource_type: String,
        resource_path: String,
        predicate: String,
        ttl: f64,
    ) -> String {
        let result = self.inner.acquire_lease(
            &agent_id,
            &session_id,
            &resource_type,
            &resource_path,
            &predicate,
            ttl as u64,
        );

        match result {
            RustLeaseResult::Success { lease } => {
                serde_json::json!({
                    "success": true,
                    "leaseId": lease.id,
                    "agentId": lease.agent_id,
                    "resource": format!("{}:{}", resource_type, resource_path),
                    "expiresAt": lease.expires_at,
                })
                .to_string()
            }
            RustLeaseResult::Failure { reason, wait_time, .. } => {
                let reason_str = match reason {
                    LeaseFailureReason::Wait => "WAIT",
                    LeaseFailureReason::Die => "DIE",
                    LeaseFailureReason::Conflict => "CONFLICT",
                    LeaseFailureReason::ResourceLocked => "RESOURCE_LOCKED",
                    LeaseFailureReason::SessionExpired => "SESSION_EXPIRED",
                };
                serde_json::json!({
                    "success": false,
                    "reason": reason_str,
                    "waitTime": wait_time,
                })
                .to_string()
            }
        }
    }

    /// Release a lease by ID.
    #[napi]
    pub fn release_lease(&mut self, lease_id: String) -> bool {
        self.inner.release_lease(&lease_id)
    }

    /// Get count of active leases.
    #[napi]
    pub fn active_lease_count(&self) -> u32 {
        self.inner.get_active_leases().len() as u32
    }

    /// Evict expired leases. Returns number evicted.
    #[napi]
    pub fn evict_expired(&mut self) -> u32 {
        self.inner.evict_expired() as u32
    }
}
