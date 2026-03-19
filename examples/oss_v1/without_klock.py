from __future__ import annotations

import threading
import time

from common import FEATURES, TARGET_FILE, build_update, feature_count, load_workspace, reset_workspace


START_BARRIER = threading.Barrier(2)


def worker(agent_id: str) -> None:
    marker, code = FEATURES[agent_id]
    snapshot = load_workspace()
    print(f"[{agent_id}] read snapshot with {feature_count(snapshot)} feature blocks")

    START_BARRIER.wait()
    time.sleep(0.2)

    TARGET_FILE.write_text(build_update(snapshot, marker, code), encoding="utf-8")
    print(f"[{agent_id}] wrote {marker}")


def main() -> None:
    reset_workspace()

    print("=== WITHOUT KLOCK ===")
    print("Two agents edit the same repo file at the same time.")
    print("Both report success. One update silently disappears.\n")

    older = threading.Thread(target=worker, args=("agent_older",))
    younger = threading.Thread(target=worker, args=("agent_younger",))

    older.start()
    younger.start()
    older.join()
    younger.join()

    final_state = load_workspace()
    print("\nFinal auth.js:\n")
    print(final_state)
    print(f"Feature blocks expected: 2")
    print(f"Feature blocks actual:   {feature_count(final_state)}")

    if feature_count(final_state) != 2:
        print("\nSILENT OVERWRITE DETECTED")
    else:
        print("\nUnexpected result: this repro should lose one feature.")


if __name__ == "__main__":
    main()
