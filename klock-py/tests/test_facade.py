import unittest
import uuid

from klock import Klock


class KlockFacadeTest(unittest.TestCase):
    def test_local_construction_is_lazy_when_server_is_unavailable(self):
        klock = Klock.local(
            agent_id="py-lazy",
            session_id="s-lazy",
            priority=100,
            base_url="http://127.0.0.1:9",
            timeout_ms=20,
            auto_start=False,
        )

        with self.assertRaises(RuntimeError):
            klock.file("/tmp/klock-lazy.txt", mode="mutate")

    def test_embedded_file_releases_on_success(self):
        klock = Klock.embedded(agent_id="py-success-a", session_id="s-a", priority=100)
        klock.register_agent("py-success-b", 200)
        path = f"/tmp/klock-success-{uuid.uuid4()}.txt"

        with klock.file(path, mode="mutate"):
            blocked = klock.acquire_lease(
                "py-success-b",
                "s-b",
                "FILE",
                path,
                "MUTATES",
                1000,
            )
            self.assertFalse(blocked["success"])

        granted = klock.acquire_lease(
            "py-success-b",
            "s-b",
            "FILE",
            path,
            "MUTATES",
            1000,
        )
        self.assertTrue(granted["success"])
        self.assertTrue(klock.release_lease(granted["lease_id"]))

    def test_embedded_file_releases_on_exception(self):
        klock = Klock.embedded(agent_id="py-error-a", session_id="s-a", priority=100)
        klock.register_agent("py-error-b", 200)
        path = f"/tmp/klock-error-{uuid.uuid4()}.txt"

        with self.assertRaises(ValueError):
            with klock.file(path, mode="mutate"):
                raise ValueError("boom")

        granted = klock.acquire_lease(
            "py-error-b",
            "s-b",
            "FILE",
            path,
            "MUTATES",
            1000,
        )
        self.assertTrue(granted["success"])
        self.assertTrue(klock.release_lease(granted["lease_id"]))

    def test_file_modes_map_to_expected_conflict_behavior(self):
        compatible_with_consumes = {"read", "provide", "depend"}
        incompatible_with_consumes = {"mutate", "delete", "rename"}

        for mode in compatible_with_consumes | incompatible_with_consumes:
            with self.subTest(mode=mode):
                klock = Klock.embedded(agent_id=f"py-{mode}-a", session_id="s-a", priority=100)
                klock.register_agent(f"py-{mode}-b", 200)
                path = f"/tmp/klock-{mode}-{uuid.uuid4()}.txt"

                with klock.file(path, mode=mode):
                    result = klock.acquire_lease(
                        f"py-{mode}-b",
                        "s-b",
                        "FILE",
                        path,
                        "CONSUMES",
                        1000,
                    )
                    if mode in compatible_with_consumes:
                        self.assertTrue(result["success"])
                        self.assertTrue(klock.release_lease(result["lease_id"]))
                    else:
                        self.assertFalse(result["success"])

    def test_invalid_file_mode_raises_runtime_error(self):
        klock = Klock.embedded(agent_id="py-invalid", session_id="s", priority=100)
        with self.assertRaises(RuntimeError):
            klock.file("/tmp/klock-invalid.txt", mode="overwrite")


if __name__ == "__main__":
    unittest.main()
