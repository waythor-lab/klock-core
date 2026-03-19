import json
import os
import threading
import time
from typing import Dict
from urllib import error, request


DB_FILE = os.path.join(os.path.dirname(__file__), "shared_state.json")
RESOURCE_PATH = "/examples/tiny_repro/shared_state.json"
BASE_URL = os.environ.get("KLOCK_BASE_URL", "http://localhost:3100")


class KlockHttpClient:
    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip("/")

    def _request(self, method: str, path: str, payload: Dict[str, object] | None = None) -> Dict[str, object]:
        body = None
        headers = {}
        if payload is not None:
            body = json.dumps(payload).encode("utf-8")
            headers["Content-Type"] = "application/json"

        req = request.Request(
            f"{self.base_url}{path}",
            data=body,
            headers=headers,
            method=method,
        )

        try:
            with request.urlopen(req, timeout=5) as response:
                raw = response.read().decode("utf-8")
                return json.loads(raw) if raw else {}
        except error.HTTPError as exc:
            raw = exc.read().decode("utf-8")
            return json.loads(raw) if raw else {}

    def register_agent(self, agent_id: str, priority: int) -> None:
        self._request(
            "POST",
            "/agents",
            {"agent_id": agent_id, "priority": priority},
        )

    def acquire_lease(
        self,
        agent_id: str,
        session_id: str,
        resource_type: str,
        resource_path: str,
        predicate: str,
        ttl: int,
    ) -> Dict[str, object]:
        payload = self._request(
            "POST",
            "/leases",
            {
                "agent_id": agent_id,
                "session_id": session_id,
                "resource_type": resource_type,
                "resource_path": resource_path,
                "predicate": predicate,
                "ttl": ttl,
            },
        )

        if payload.get("success"):
            return {"success": True, "lease_id": payload["data"]["lease_id"]}

        return {
            "success": False,
            "reason": payload.get("reason", "CONFLICT"),
            "wait_time": payload.get("wait_time", 1000),
        }

    def release_lease(self, lease_id: str) -> None:
        self._request("DELETE", f"/leases/{lease_id}")


def reset_db():
    with open(DB_FILE, "w", encoding="utf-8") as handle:
        json.dump([], handle)


def acquire_with_retry(
    client: KlockHttpClient, agent_id: str, session_id: str
) -> str:
    while True:
        result = client.acquire_lease(
            agent_id=agent_id,
            session_id=session_id,
            resource_type="FILE",
            resource_path=RESOURCE_PATH,
            predicate="MUTATES",
            ttl=5_000,
        )

        if result["success"]:
            lease_id = str(result["lease_id"])
            print(f"[{agent_id}] acquired lease {lease_id}")
            return lease_id

        reason = str(result["reason"])
        wait_ms = int(result.get("wait_time") or 1000)
        print(f"[{agent_id}] {reason}; retrying in {wait_ms}ms")
        time.sleep(wait_ms / 1000.0)


def worker(client: KlockHttpClient, agent_id: str, session_id: str):
    lease_id = acquire_with_retry(client, agent_id, session_id)
    try:
        with open(DB_FILE, "r", encoding="utf-8") as handle:
            data = json.load(handle)

        print(f"[{agent_id}] read {len(data)} entries inside lease")
        time.sleep(0.2)
        data.append(agent_id)

        with open(DB_FILE, "w", encoding="utf-8") as handle:
            json.dump(data, handle)

        print(f"[{agent_id}] wrote its update safely")
    finally:
        client.release_lease(lease_id)
        print(f"[{agent_id}] released lease")


def main():
    client = KlockHttpClient(BASE_URL)
    reset_db()

    print("=== WITH KLOCK ===")
    print("The same two workers now coordinate through the local Klock server.\n")

    try:
        client.register_agent("agent_A", 100)
        client.register_agent("agent_B", 200)
    except (error.URLError, error.HTTPError) as exc:
        print(f"Failed to register agents against {BASE_URL}: {exc}")
        print("Start the Klock server first:")
        print("  cargo run --release -p klock-cli -- serve")
        raise SystemExit(1)

    thread_a = threading.Thread(target=worker, args=(client, "agent_A", "session_A"))
    thread_b = threading.Thread(target=worker, args=(client, "agent_B", "session_B"))

    thread_a.start()
    thread_b.start()
    thread_a.join()
    thread_b.join()

    with open(DB_FILE, "r", encoding="utf-8") as handle:
        final_state = json.load(handle)

    print("\nFinal shared_state.json:")
    print(json.dumps(final_state, indent=2))
    print(f"\nExpected entries: 2")
    print(f"Actual entries:   {len(final_state)}")

    if len(final_state) == 2:
        print("\nWAIT-DIE COORDINATION CONFIRMED")
        print("Klock prevented the silent overwrite.")
    else:
        print("\nUnexpected result: Klock should coordinate both writes.")


if __name__ == "__main__":
    main()
