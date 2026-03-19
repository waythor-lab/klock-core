"""Type stubs for the klock-core native module (PyO3)."""

from typing import Optional

class KlockClient:
    """The Klock coordination client.
    
    Manages agent registration, lease acquisition, and conflict resolution
    through a Rust-powered coordination kernel.
    """

    def __init__(self) -> None:
        """Create a new KlockClient with an empty in-memory store."""
        ...

    def register_agent(self, agent_id: str, priority: int) -> None:
        """Register an agent with a priority.
        
        Lower priority values = older = higher precedence in Wait-Die scheduling.
        
        Args:
            agent_id: Unique identifier for the agent.
            priority: Timestamp-based priority (lower = older = higher priority).
        """
        ...

    def acquire_lease(
        self,
        agent_id: str,
        session_id: str,
        resource_type: str,
        resource_path: str,
        predicate: str,
        ttl: int,
    ) -> dict[str, object]:
        """Acquire a lease on a resource.
        
        Args:
            agent_id: ID of the requesting agent.
            session_id: Session identifier (same agent+session = reentrant).
            resource_type: One of: FILE, SYMBOL, API_ENDPOINT, DATABASE_TABLE, CONFIG_KEY.
            resource_path: Path to the resource (e.g., "/src/auth.ts").
            predicate: One of: PROVIDES, CONSUMES, MUTATES, DELETES, DEPENDS_ON, RENAMES.
            ttl: Time-to-live in milliseconds.
        
        Returns:
            On success: {"success": True, "lease_id": str, "agent_id": str, "resource": str, "expires_at": int}
            On failure: {"success": False, "reason": str, "wait_time": Optional[int]}
            
            Reason values: "DIE", "WAIT", "CONFLICT", "RESOURCE_LOCKED", "SESSION_EXPIRED"
        """
        ...

    def release_lease(self, lease_id: str) -> bool:
        """Release a lease by its ID.
        
        Args:
            lease_id: The ID of the lease to release.
        
        Returns:
            True if the lease was found and released, False otherwise.
        """
        ...

    def active_lease_count(self) -> int:
        """Get the count of currently active leases."""
        ...

    def evict_expired(self) -> int:
        """Remove expired leases.
        
        Returns:
            The number of leases evicted.
        """
        ...


class KlockHttpClient:
    """HTTP client for a local or remote Klock coordination server."""

    def __init__(
        self,
        base_url: str = "http://localhost:3100",
        api_key: Optional[str] = None,
        timeout_ms: int = 5000,
        auto_start: bool = True,
        startup_timeout_ms: int = 5000,
        server_command: Optional[list[str]] = None,
    ) -> None:
        ...

    def register_agent(self, agent_id: str, priority: int) -> None:
        ...

    def auto_start_enabled(self) -> bool:
        ...

    def auto_start_disabled_by_env(self) -> bool:
        ...

    def last_started_pid(self) -> Optional[int]:
        ...

    def acquire_lease(
        self,
        agent_id: str,
        session_id: str,
        resource_type: str,
        resource_path: str,
        predicate: str,
        ttl: int,
    ) -> dict[str, object]:
        ...

    def release_lease(self, lease_id: str) -> bool:
        ...

    def heartbeat_lease(self, lease_id: str) -> bool:
        ...

    def list_leases(self) -> list[dict[str, object]]:
        ...
