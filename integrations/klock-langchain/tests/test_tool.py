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


class KlockProtectedTests(unittest.TestCase):
    def test_successful_call_acquires_and_releases(self):
        client = FakeKlockClient([{"success": True, "lease_id": "lease-1"}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        def write_file(path, content):
            return content.upper()

        result = write_file(path="/tmp/auth.ts", content="hello")

        self.assertEqual(result, "HELLO")
        self.assertEqual(client.acquire_calls[0]["resource_path"], "/tmp/auth.ts")
        self.assertEqual(client.release_calls, ["lease-1"])

    @patch("klock_langchain.tool.time.sleep")
    def test_wait_retries_then_succeeds(self, sleep_mock):
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
        def write_file(path):
            return path

        result = write_file(path="/tmp/auth.ts")

        self.assertEqual(result, "/tmp/auth.ts")
        self.assertEqual(len(client.acquire_calls), 2)
        sleep_mock.assert_called_once_with(0.25)
        self.assertEqual(client.release_calls, ["lease-2"])

    def test_die_raises_conflict_error(self):
        client = FakeKlockClient([{"success": False, "reason": "DIE", "wait_time": 1000}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-2",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        def write_file(path):
            return path

        with self.assertRaises(KlockConflictError) as exc_info:
            write_file(path="/tmp/auth.ts")

        self.assertEqual(exc_info.exception.reason, "DIE")
        self.assertEqual(client.release_calls, [])

    def test_release_runs_even_if_wrapped_function_fails(self):
        client = FakeKlockClient([{"success": True, "lease_id": "lease-3"}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
        )
        def write_file(path):
            raise ValueError("boom")

        with self.assertRaises(ValueError):
            write_file(path="/tmp/auth.ts")

        self.assertEqual(client.release_calls, ["lease-3"])

    def test_missing_resource_path_raises_value_error(self):
        client = FakeKlockClient([{"success": True, "lease_id": "lease-4"}])

        @klock_protected(
            klock_client=client,
            agent_id="agent-1",
            session_id="session-1",
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs.get("missing"),
        )
        def write_file(path):
            return path

        with self.assertRaises(ValueError):
            write_file(path="/tmp/auth.ts")


if __name__ == "__main__":
    unittest.main()
