use pyo3::prelude::*;
use pyo3::types::PyDict;

use ::klock_core::client::KlockClient as RustClient;
use ::klock_core::types::{LeaseResult as RustLeaseResult, LeaseFailureReason};

/// The Klock coordination client for Python.
/// Manages agent registration, lease acquisition, and conflict resolution.
#[pyclass(unsendable)]
pub struct KlockClient {
    inner: RustClient,
}

#[pymethods]
impl KlockClient {
    /// Create a new KlockClient.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: RustClient::new(),
        }
    }

    /// Register an agent with a priority (lower = older = higher priority).
    pub fn register_agent(&mut self, agent_id: &str, priority: u64) {
        self.inner.register_agent(agent_id, priority);
    }

    /// Acquire a lease on a resource.
    /// Returns a dict with 'success', 'lease_id', 'reason', and 'wait_time'.
    pub fn acquire_lease<'py>(
        &mut self,
        py: Python<'py>,
        agent_id: &str,
        session_id: &str,
        resource_type: &str,
        resource_path: &str,
        predicate: &str,
        ttl: u64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let result = self.inner.acquire_lease(
            agent_id,
            session_id,
            resource_type,
            resource_path,
            predicate,
            ttl,
        );

        let dict = PyDict::new(py);

        match result {
            RustLeaseResult::Success { lease } => {
                dict.set_item("success", true)?;
                dict.set_item("lease_id", &lease.id)?;
                dict.set_item("agent_id", &lease.agent_id)?;
                dict.set_item("resource", format!("{}:{}", resource_type, resource_path))?;
                dict.set_item("expires_at", lease.expires_at)?;
            }
            RustLeaseResult::Failure { reason, wait_time, .. } => {
                let reason_str = match reason {
                    LeaseFailureReason::Wait => "WAIT",
                    LeaseFailureReason::Die => "DIE",
                    LeaseFailureReason::Conflict => "CONFLICT",
                    LeaseFailureReason::ResourceLocked => "RESOURCE_LOCKED",
                    LeaseFailureReason::SessionExpired => "SESSION_EXPIRED",
                };
                dict.set_item("success", false)?;
                dict.set_item("reason", reason_str)?;
                dict.set_item("wait_time", wait_time)?;
            }
        }

        Ok(dict)
    }

    /// Release a lease by its ID.
    pub fn release_lease(&mut self, lease_id: &str) -> bool {
        self.inner.release_lease(lease_id)
    }

    /// Get the number of currently active leases.
    pub fn active_lease_count(&self) -> usize {
        self.inner.get_active_leases().len()
    }

    /// Evict expired leases. Returns number evicted.
    pub fn evict_expired(&mut self) -> usize {
        self.inner.evict_expired()
    }
}

/// The Klock Python module.
#[pymodule]
fn klock(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<KlockClient>()?;
    Ok(())
}
