import asyncio
import functools
import inspect
import time
from typing import Any, Callable, Dict, Optional

try:
    from langchain_core.tools import BaseTool
except ModuleNotFoundError:  # pragma: no cover - optional dependency for decorator-only usage
    class BaseTool:  # type: ignore[no-redef]
        pass


class KlockConflictError(RuntimeError):
    """Raised when Klock denies a protected tool call."""

    def __init__(
        self,
        agent_id: str,
        resource_path: str,
        reason: str,
        wait_time_ms: Optional[int] = None,
    ):
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
        elif reason == "STORAGE_UNAVAILABLE":
            message = (
                f"Klock storage is unavailable for {agent_id} on {resource_path}: "
                "the server has poisoned its store and is refusing acquires. Try again later."
            )
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
    max_retries: int = 10,
):
    """
    A decorator that protects a LangChain Tool with Klock Wait-Die concurrency control.

    Wraps both sync (`_run`) and async (`_arun`) tool methods. The async path
    uses `await asyncio.sleep` for WAIT backoff so it does not block the
    event loop.

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
    # Support decorating class methods (like _run in BaseTool subclasses)
    if isinstance(klock_client, type) and issubclass(klock_client, BaseTool):
        raise ValueError("klock_protected must be called with a klock_client instance, not a class.")

    def decorator(func: Callable):
        if inspect.iscoroutinefunction(func):

            @functools.wraps(func)
            async def async_wrapper(*args, **kwargs):
                resource_path = _extract_resource_path(resource_path_extractor, kwargs)
                lease_id = await _acquire_lock_with_wait_die_async(
                    klock_client,
                    agent_id,
                    session_id,
                    resource_type,
                    resource_path,
                    predicate,
                    ttl_ms,
                    max_retries,
                )
                try:
                    return await func(*args, **kwargs)
                finally:
                    if lease_id:
                        klock_client.release_lease(lease_id)

            return async_wrapper

        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            resource_path = _extract_resource_path(resource_path_extractor, kwargs)
            lease_id = _acquire_lock_with_wait_die(
                klock_client,
                agent_id,
                session_id,
                resource_type,
                resource_path,
                predicate,
                ttl_ms,
                max_retries,
            )

            try:
                return func(*args, **kwargs)
            finally:
                if lease_id:
                    klock_client.release_lease(lease_id)

        return wrapper

    return decorator


def _extract_resource_path(extractor: Callable[[Dict[str, Any]], str], kwargs: Dict[str, Any]) -> str:
    """Run the user-supplied extractor and convert any failure to a typed ValueError.

    Without this wrapper a KeyError or AttributeError raised inside the
    extractor would propagate through the decorator unchanged, which breaks
    the documented contract of "missing resource path -> ValueError".
    """
    try:
        path = extractor(kwargs)
    except Exception as exc:
        raise ValueError(
            "klock_protected: resource_path_extractor raised; the extractor must "
            "return a resource path string for the tool's kwargs."
        ) from exc
    if not path:
        raise ValueError("klock_protected could not resolve a resource path from the tool arguments.")
    return path


def _acquire_lock_with_wait_die(klock_client, agent_id, session_id, resource_type, resource_path, predicate, ttl_ms, max_retries):
    """Internal helper to repeatedly attempt lock acquisition according to Wait-Die rules (sync)."""
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
            wait_ms = result.get("wait_time")
            if wait_ms is None:
                wait_ms = 1000
            time.sleep(wait_ms / 1000.0)
            total_wait_ms += wait_ms
            retries += 1
        elif reason == "DIE":
            raise KlockConflictError(
                agent_id=agent_id,
                resource_path=resource_path,
                reason="DIE",
                wait_time_ms=result.get("wait_time"),
            )
        else:
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


async def _acquire_lock_with_wait_die_async(
    klock_client,
    agent_id,
    session_id,
    resource_type,
    resource_path,
    predicate,
    ttl_ms,
    max_retries,
):
    """Async variant: uses asyncio.sleep for backoff so the event loop stays responsive.

    The underlying ``klock_client.acquire_lease`` and ``release_lease`` calls
    remain synchronous (they delegate to PyO3-bound HTTP calls). For typical
    millisecond-scale operations this is fine; users with strict event-loop
    budgets can run them under ``loop.run_in_executor``.
    """
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
            wait_ms = result.get("wait_time")
            if wait_ms is None:
                wait_ms = 1000
            await asyncio.sleep(wait_ms / 1000.0)
            total_wait_ms += wait_ms
            retries += 1
        elif reason == "DIE":
            raise KlockConflictError(
                agent_id=agent_id,
                resource_path=resource_path,
                reason="DIE",
                wait_time_ms=result.get("wait_time"),
            )
        else:
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
