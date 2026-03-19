from __future__ import annotations

import threading
import time

from klock import KlockHttpClient
from klock_langchain import KlockConflictError, klock_protected

from common import FEATURES, TARGET_FILE, build_update, feature_count, load_workspace, reset_workspace


CLIENT = KlockHttpClient()
RESOURCE_PATH = str(TARGET_FILE)


def ensure_server() -> None:
    try:
        CLIENT.register_agent("agent_older", 100)
        CLIENT.register_agent("agent_younger", 200)
    except Exception as exc:  # pragma: no cover - user-facing demo path
        print(f"Failed to reach the local Klock server: {exc}")
        print("Start it from Klock-OpenSource/:")
        print("  cargo run --release -p klock-cli -- serve")
        raise SystemExit(1) from exc


def run_agent(agent_id: str, session_id: str, start_delay: float = 0.0) -> None:
    if start_delay:
        time.sleep(start_delay)

    marker, code = FEATURES[agent_id]

    @klock_protected(
        klock_client=CLIENT,
        agent_id=agent_id,
        session_id=session_id,
        resource_type="FILE",
        resource_path_extractor=lambda kwargs: kwargs["path"],
        predicate="MUTATES",
        ttl_ms=5_000,
        max_retries=5,
    )
    def protected_write(path: str) -> str:
        snapshot = load_workspace()
        print(f"[{agent_id}] GRANT {marker} on {path}")
        print(f"[{agent_id}] editing inside lease")
        time.sleep(0.2)
        TARGET_FILE.write_text(build_update(snapshot, marker, code), encoding="utf-8")
        return marker

    attempts = 0
    while True:
        try:
            protected_write(path=RESOURCE_PATH)
            print(f"[{agent_id}] finished safely")
            return
        except KlockConflictError as exc:
            attempts += 1
            print(f"[{agent_id}] {exc.reason}; retry {attempts}")

            if exc.reason != "DIE" or attempts >= 5:
                raise

            time.sleep(0.25)


def main() -> None:
    reset_workspace()
    ensure_server()

    print("=== WITH KLOCK ===")
    print("The same two agents coordinate through the local Klock server.")
    print("The younger agent may DIE and retry, but the file stays correct.\n")

    older = threading.Thread(target=run_agent, args=("agent_older", "session-older", 0.0))
    younger = threading.Thread(target=run_agent, args=("agent_younger", "session-younger", 0.05))

    older.start()
    younger.start()
    older.join()
    younger.join()

    final_state = load_workspace()
    print("\nFinal auth.js:\n")
    print(final_state)
    print(f"Feature blocks expected: 2")
    print(f"Feature blocks actual:   {feature_count(final_state)}")

    if feature_count(final_state) == 2:
        print("\nWAIT-DIE COORDINATION CONFIRMED")
    else:
        print("\nUnexpected result: Klock should preserve both updates.")


if __name__ == "__main__":
    main()
