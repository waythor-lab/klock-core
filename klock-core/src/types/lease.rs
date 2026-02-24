use serde::{Deserialize, Serialize};

use super::{Predicate, ResourceRef};

/// Lease states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseState {
    /// Lease is held and valid
    Active,
    /// Lease TTL elapsed without heartbeat
    Expired,
    /// Lease was explicitly released
    Released,
    /// Lease was forcibly revoked (conflict resolution)
    Revoked,
}

/// A time-bound lock on a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// Unique lease ID
    pub id: String,
    /// Agent holding the lease
    pub agent_id: String,
    /// Session the lease belongs to
    pub session_id: String,
    /// The leased resource
    pub resource: ResourceRef,
    /// What operation is being performed
    pub predicate: Predicate,
    /// Current lease state
    pub state: LeaseState,
    /// When the lease was acquired
    pub acquired_at: u64,
    /// Time-to-live in milliseconds
    pub ttl: u64,
    /// When the lease will expire (acquiredAt + ttl)
    pub expires_at: u64,
    /// Last heartbeat timestamp
    pub last_heartbeat: u64,
}

impl Lease {
    pub fn new(
        id: String,
        agent_id: String,
        session_id: String,
        resource: ResourceRef,
        predicate: Predicate,
        ttl: u64,
        now: u64,
    ) -> Self {
        Self {
            id,
            agent_id,
            session_id,
            resource,
            predicate,
            state: LeaseState::Active,
            acquired_at: now,
            ttl,
            expires_at: now + ttl,
            last_heartbeat: now,
        }
    }
}

pub enum LeaseFailureReason {
    /// Another agent holds a conflicting lease
    Conflict,
    /// Wait-Die: older agent, should wait
    Wait,
    /// Wait-Die: younger agent, should abort and retry
    Die,
    /// Resource is locked for another operation
    ResourceLocked,
    /// The session has expired
    SessionExpired,
}

/// Result of attempting to acquire a lease
pub enum LeaseResult {
    Success {
        lease: Lease,
    },
    Failure {
        reason: LeaseFailureReason,
        existing_lease: Option<Lease>,
        wait_time: Option<u64>,
    },
}
