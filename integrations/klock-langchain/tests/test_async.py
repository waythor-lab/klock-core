"""Async coverage for the v0.1.3 @klock_protected decorator.

Mirrors test_tool.py but exercises the coroutine path (async _run / async
tools). The async path uses asyncio.sleep for WAIT backoff so the event
loop stays responsive.
"""

import asyncio
import unittest
from unittest.mock import patch

from klock_langchain import KlockConflictError, klock_protected


class FakeKlockClient:
    def __init__(self, responses):
        self._responses = list(responses)
        self.acquire_calls = []
        self.release_calls = []

    def acquire_lease(self, agent_id, session_id, resource_type, resource_path, predicate, ttl_ms):
        self.acquire_calls.append(
            {
                "agent_id": agent_id,
                "session_id": session_id,
                "resource_type": resource_type,
                "resource_path": resource_path,
                "predicate": predicate,
                "ttl_ms": ttl_ms,
            }
        )
        return self._responses.pop(0)

    def release_lease(self, lease_id):
        self.release_calls.append(lease_id)
        return True


class KlockProtectedAsyncTests(unittest.TestCase):
    def test_async_successful_call_acquires_and_releases(self):
        client = FakeKlockClient([{"success": True, "lease_id": "lease-1"}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        async def write_file(path, content):
            return content.upper()

        result = asyncio.run(write_file(path="/tmp/auth.ts", content="hello"))

        self.assertEqual(result, "HELLO")
        self.assertEqual(client.acquire_calls[0]["resource_path"], "/tmp/auth.ts")
        self.assertEqual(client.release_calls, ["lease-1"])

    def test_async_wait_uses_asyncio_sleep(self):
        client = FakeKlockClient(
            [
                {"success": False, "reason": "WAIT", "wait_time": 250},
                {"success": True, "lease_id": "lease-2"},
            ]
        )

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        async def write_file(path):
            return path

        # Patch asyncio.sleep on the tool module so we can assert non-blocking
        # backoff was used (vs `time.sleep` which would block the event loop).
        with patch("klock_langchain.tool.asyncio.sleep") as sleep_mock:
            async def fake_sleep(_):
                return None

            sleep_mock.side_effect = fake_sleep
            result = asyncio.run(write_file(path="/tmp/auth.ts"))

            self.assertEqual(result, "/tmp/auth.ts")
            self.assertEqual(len(client.acquire_calls), 2)
            sleep_mock.assert_called_once_with(0.25)
            self.assertEqual(client.release_calls, ["lease-2"])

    def test_async_die_raises_conflict_error(self):
        client = FakeKlockClient([{"success": False, "reason": "DIE", "wait_time": 1000}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-2",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        async def write_file(path):
            return path

        with self.assertRaises(KlockConflictError) as exc_info:
            asyncio.run(write_file(path="/tmp/auth.ts"))

        self.assertEqual(exc_info.exception.reason, "DIE")
        self.assertEqual(client.release_calls, [])

    def test_async_release_runs_on_exception(self):
        client = FakeKlockClient([{"success": True, "lease_id": "lease-3"}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        async def write_file(path):
            raise RuntimeError("boom")

        with self.assertRaises(RuntimeError):
            asyncio.run(write_file(path="/tmp/auth.ts"))

        self.assertEqual(client.release_calls, ["lease-3"])

    def test_extractor_exception_becomes_value_error_async(self):
        """resource_path_extractor raising must surface as ValueError, not the original error type."""
        client = FakeKlockClient([{"success": True, "lease_id": "lease-4"}])

        def bad_extractor(kwargs):
            return kwargs["missing_key"]  # KeyError without the wrapper

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=bad_extractor,
        )
        async def write_file(path):
            return path

        with self.assertRaises(ValueError):
            asyncio.run(write_file(path="/tmp/auth.ts"))

    def test_extractor_exception_becomes_value_error_sync(self):
        """Same property on the sync path — the v0.1.3 fix."""
        client = FakeKlockClient([{"success": True, "lease_id": "lease-5"}])

        def bad_extractor(kwargs):
            return kwargs["missing_key"]

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=bad_extractor,
        )
        def write_file(path):
            return path

        with self.assertRaises(ValueError):
            write_file(path="/tmp/auth.ts")


if __name__ == "__main__":
    unittest.main()
