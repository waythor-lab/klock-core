"""Type stubs for the klock-core native module (PyO3)."""

from types import TracebackType
from typing import Literal, Optional, Type

KlockFileMode = Literal["read", "mutate", "delete", "rename", "provide", "depend"]

class KlockFileGuard:
    """Context manager returned by ``Klock.file(...)``."""

    def __enter__(self) -> "KlockFileGuard":
        ...

    def __exit__(
        self,
        exc_type: Optional[Type[BaseException]],
        exc_val: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> bool:
        ...

    def release(self) -> bool:
        ...


class Klock:
    """Simple facade for protecting local file operations.

    ``Klock.local(...)`` is the default user-facing path and uses the local
    HTTP coordinator with auto-start for cross-process coordination.
    ``Klock.embedded(...)`` is server-free but only coordinates inside one
    process.
    """

    @staticmethod
    def local(
        agent_id: Optional[str] = None,
        session_id: Optional[str] = None,
        priority: Optional[int] = None,
        base_url: str = "http://localhost:3100",
        api_key: Optional[str] = None,
        timeout_ms: int = 5000,
        auto_start: bool = True,
        startup_timeout_ms: int = 5000,
        server_command: Optional[list[str]] = None,
    ) -> "Klock":
        ...

    @staticmethod
    def embedded(
        agent_id: Optional[str] = None,
        session_id: Optional[str] = None,
        priority: Optional[int] = None,
    ) -> "Klock":
        ...

    def file(
        self,
        path: str,
        mode: KlockFileMode = "mutate",
        ttl_ms: int = 60000,
        max_retries: int = 10,
    ) -> KlockFileGuard:
        ...

    def register_agent(self, agent_id: str, priority: int) -> None:
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

            Reason values: "DIE", "WAIT", "CONFLICT", "RESOURCE_LOCKED",
            "SESSION_EXPIRED", "STORAGE_UNAVAILABLE".
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
    """HTTP client for a local or remote Klock coordination server.

    When ``auto_start`` is True (default) and ``base_url`` is local, this
    client spawns a ``klock serve`` subprocess if the server is unreachable.
    The spawned process is owned by the client and terminated by
    ``shutdown()``, the context-manager protocol, or garbage collection.

    Recommended usage::

        with KlockHttpClient() as klock:
            klock.register_agent("a", 100)
            ...

    For long-lived clients without the ``with`` statement, call
    ``shutdown()`` explicitly before exit; ``__del__`` is best-effort.
    """

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

    def shutdown(self) -> None:
        """Terminate the auto-started server subprocess, if owned.

        Safe to call multiple times. Servers that were already running
        when this client was constructed are not touched.
        """
        ...

    def __enter__(self) -> "KlockHttpClient":
        ...

    def __exit__(
        self,
        exc_type: Optional[Type[BaseException]],
        exc_val: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> bool:
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
