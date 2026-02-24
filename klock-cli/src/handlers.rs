use serde::{Deserialize, Serialize};

// ─── Validation Constants ───────────────────────────────────────────────────

const VALID_PREDICATES: &[&str] = &[
    "PROVIDES", "CONSUMES", "MUTATES", "DELETES", "DEPENDS_ON", "RENAMES",
];

const VALID_RESOURCE_TYPES: &[&str] = &[
    "FILE", "SYMBOL", "API_ENDPOINT", "DATABASE_TABLE", "CONFIG_KEY",
];

// ─── Validation Helpers ─────────────────────────────────────────────────────

pub fn validate_predicate(predicate: &str) -> Result<(), String> {
    if VALID_PREDICATES.contains(&predicate.to_uppercase().as_str()) {
        Ok(())
    } else {
        Err(format!(
            "Invalid predicate '{}'. Must be one of: {}",
            predicate,
            VALID_PREDICATES.join(", ")
        ))
    }
}

pub fn validate_resource_type(resource_type: &str) -> Result<(), String> {
    if VALID_RESOURCE_TYPES.contains(&resource_type.to_uppercase().as_str()) {
        Ok(())
    } else {
        Err(format!(
            "Invalid resource_type '{}'. Must be one of: {}",
            resource_type,
            VALID_RESOURCE_TYPES.join(", ")
        ))
    }
}

// ─── Request Types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterAgentRequest {
    pub agent_id: String,
    pub priority: u64,
}

#[derive(Deserialize)]
pub struct AcquireLeaseRequest {
    pub agent_id: String,
    pub session_id: String,
    pub resource_type: String,
    pub resource_path: String,
    pub predicate: String,
    pub ttl: u64,
}

impl AcquireLeaseRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.agent_id.is_empty() {
            return Err("agent_id is required".to_string());
        }
        if self.session_id.is_empty() {
            return Err("session_id is required".to_string());
        }
        if self.resource_path.is_empty() {
            return Err("resource_path is required".to_string());
        }
        validate_predicate(&self.predicate)?;
        validate_resource_type(&self.resource_type)?;
        if self.ttl == 0 {
            return Err("ttl must be greater than 0".to_string());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct ReleaseLeaseRequest {
    pub lease_id: String,
}

#[derive(Deserialize)]
pub struct DeclareIntentRequest {
    pub session_id: String,
    pub agent_id: String,
    pub intents: Vec<IntentItem>,
}

impl DeclareIntentRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.agent_id.is_empty() {
            return Err("agent_id is required".to_string());
        }
        if self.session_id.is_empty() {
            return Err("session_id is required".to_string());
        }
        if self.intents.is_empty() {
            return Err("intents must not be empty".to_string());
        }
        for (i, intent) in self.intents.iter().enumerate() {
            validate_predicate(&intent.predicate)
                .map_err(|e| format!("intents[{}]: {}", i, e))?;
            validate_resource_type(&intent.resource_type)
                .map_err(|e| format!("intents[{}]: {}", i, e))?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct IntentItem {
    pub predicate: String,
    pub resource_type: String,
    pub resource_path: String,
}

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Serialize)]
pub struct LeaseResponse {
    pub lease_id: String,
    pub agent_id: String,
    pub resource: String,
    pub expires_at: u64,
}

#[derive(Serialize)]
pub struct LeaseFailureResponse {
    pub reason: String,
    pub wait_time: Option<u64>,
}

#[derive(Serialize)]
pub struct ActiveLeaseInfo {
    pub id: String,
    pub agent_id: String,
    pub resource: String,
    pub predicate: String,
    pub expires_at: u64,
}

#[derive(Serialize)]
pub struct EvictResponse {
    pub evicted: usize,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub active_leases: usize,
    pub version: String,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub renewed: bool,
    pub lease_id: String,
}
