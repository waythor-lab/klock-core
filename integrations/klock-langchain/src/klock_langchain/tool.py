import time
import functools
from typing import Any, Callable, Dict, Optional

try:
    from langchain_core.tools import BaseTool
except ModuleNotFoundError:  # pragma: no cover - optional dependency for decorator-only usage
    class BaseTool:  # type: ignore[no-redef]
        pass

class KlockConflictError(RuntimeError):
    """Raised when Klock denies a protected tool call."""

    def __init__(self, agent_id: str, resource_path: str, reason: str, wait_time_ms: Optional[int] = None):
        self.agent_id = agent_id
        self.resource_path = resource_path
        self.reason = reason
        self.wait_time_ms = wait_time_ms

        if reason == "DIE":
            message = (
                f"Klock denied {agent_id} on {resource_path}: DIE. "
                "A younger agent attempted to mutate a resource already owned by an older agent. Retry later."
            )
        elif reason == "WAIT":
            wait_note = f" after waiting {wait_time_ms}ms" if wait_time_ms is not None else ""
            message = f"Klock still requires {agent_id} to WAIT on {resource_path}{wait_note}."
        elif reason == "WAIT_TIMEOUT":
            wait_note = f" after {wait_time_ms}ms of backoff" if wait_time_ms is not None else ""
            message = f"Klock exceeded max WAIT retries for {agent_id} on {resource_path}{wait_note}."
        else:
            message = f"Klock denied {agent_id} on {resource_path}: {reason}."

        super().__init__(message)

def klock_protected(
    klock_client: Any,
    agent_id: str,
    session_id: str,
    resource_type: str,
    resource_path_extractor: Callable[[Dict[str, Any]], str],
    predicate: str = "MUTATES",
    ttl_ms: int = 60000,
    max_retries: int = 10
):
    """
    A decorator that protects a LangChain Tool with Klock Wait-Die concurrency control.
    
    Args:
        klock_client: An instance of the KlockClient from the 'klock' package.
        agent_id: The ID of the agent executing the tool.
        session_id: The ID of the session/workflow.
        resource_type: The type of resource (e.g., "FILE", "TABLE").
        resource_path_extractor: A function that takes the tool's kwargs and returns the resource path string.
        predicate: The intent of the operation (e.g., "MUTATES", "CONSUMES"). Default: "MUTATES".
        ttl_ms: How long the lease should be held before automatic eviction if the tool crashes. Default: 60s.
        max_retries: Maximum number of times to wait before giving up.
    """
    def decorator(func: Callable):
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            # Try to extract kwargs if called directly or via LangChain's _run
            # In LangChain BaseTool, arguments are passed as kwargs.
            resource_path = resource_path_extractor(kwargs)
            if not resource_path:
                raise ValueError("klock_protected could not resolve a resource path from the tool arguments.")
            
            lease_id = _acquire_lock_with_wait_die(
                klock_client, agent_id, session_id, resource_type, resource_path, predicate, ttl_ms, max_retries
            )
            
            try:
                # Execute the actual tool
                return func(*args, **kwargs)
            finally:
                # Always release the lease when done
                if lease_id:
                    klock_client.release_lease(lease_id)
                    
        return wrapper

    # Support decorating class methods (like _run in BaseTool subclasses)
    if isinstance(klock_client, type) and issubclass(klock_client, BaseTool):
        raise ValueError("klock_protected must be called with a klock_client instance, not a class.")

    return decorator

def _acquire_lock_with_wait_die(klock_client, agent_id, session_id, resource_type, resource_path, predicate, ttl_ms, max_retries):
    """Internal helper to repeatedly attempt lock acquisition according to Wait-Die rules."""
    retries = 0
    total_wait_ms = 0
    while retries < max_retries:
        result = klock_client.acquire_lease(
            agent_id, session_id, resource_type, resource_path, predicate, ttl_ms
        )
        
        if result.get("success"):
            return result.get("lease_id")
            
        reason = result.get("reason")
        if reason == "WAIT":
            # Senior agent waiting for younger to finish.
            # Klock returns wait_time in milliseconds (might be None if unspecified)
            wait_ms = result.get("wait_time")
            if wait_ms is None:
                wait_ms = 1000
                
            time.sleep(wait_ms / 1000.0)
            total_wait_ms += wait_ms
            retries += 1
        elif reason == "DIE":
            # Junior agent aborts to prevent deadlock.
            # LangChain handles exceptions by passing them back to the LLM to process and try again later.
            raise KlockConflictError(
                agent_id=agent_id,
                resource_path=resource_path,
                reason="DIE",
                wait_time_ms=result.get("wait_time"),
            )
        else:
            # General conflict
            raise KlockConflictError(
                agent_id=agent_id,
                resource_path=resource_path,
                reason=reason or "CONFLICT",
                wait_time_ms=result.get("wait_time"),
            )
            
    raise KlockConflictError(
        agent_id=agent_id,
        resource_path=resource_path,
        reason="WAIT_TIMEOUT",
        wait_time_ms=total_wait_ms,
    )
