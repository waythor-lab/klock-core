use serde::{Deserialize, Serialize};

/// Predicates represent the relationship between an agent and a resource.
/// These are the verbs in the Subject-Predicate-Object (SPO) triples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Predicate {
    /// Agent creates/exports something new
    Provides,
    /// Agent reads/imports something existing
    Consumes,
    /// Agent modifies something existing
    Mutates,
    /// Agent removes something existing
    Deletes,
    /// Agent requires something to exist
    DependsOn,
    /// Agent renames a resource
    Renames,
}

impl Predicate {
    /// Returns the numeric index for O(1) matrix lookup
    pub fn to_index(self) -> usize {
        match self {
            Predicate::Provides => 0,
            Predicate::Consumes => 1,
            Predicate::Mutates => 2,
            Predicate::Deletes => 3,
            Predicate::DependsOn => 4,
            Predicate::Renames => 5,
        }
    }
}

/// Confidence levels for inferred intents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// Types of resources that can be leased and conflict-checked
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// A file path
    File,
    /// An exported symbol (function, class, variable)
    Symbol,
    /// An API route
    ApiEndpoint,
    /// A database table
    DatabaseTable,
    /// A configuration key
    ConfigKey,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::File => write!(f, "FILE"),
            ResourceType::Symbol => write!(f, "SYMBOL"),
            ResourceType::ApiEndpoint => write!(f, "API_ENDPOINT"),
            ResourceType::DatabaseTable => write!(f, "DATABASE_TABLE"),
            ResourceType::ConfigKey => write!(f, "CONFIG_KEY"),
        }
    }
}

/// A reference to a resource in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    pub resource_type: ResourceType,
    /// Normalized path (e.g., "/src/auth.ts" or "User.authenticate")
    pub path: String,
}

impl ResourceRef {
    pub fn new(resource_type: ResourceType, path: impl Into<String>) -> Self {
        Self {
            resource_type,
            path: path.into(),
        }
    }

    /// Creates a canonical string key for the resource (used for hash-based lookups)
    pub fn key(&self) -> String {
        format!("{}:{}", self.resource_type, self.path)
    }
}

/// A Subject-Predicate-Object triple representing an agent's intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SPOTriple {
    /// Unique triple ID
    pub id: String,
    /// Agent ID
    pub subject: String,
    /// What the agent intends to do
    pub predicate: Predicate,
    /// The resource being operated on
    pub object: ResourceRef,
    /// When this intent was registered
    pub timestamp: u64,
    /// How confident we are in this inference
    pub confidence: Confidence,
    /// The session this triple belongs to
    pub session_id: String,
}
