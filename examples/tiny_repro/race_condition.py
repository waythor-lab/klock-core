import json
import os
import threading
import time


DB_FILE = os.path.join(os.path.dirname(__file__), "shared_state.json")
START_BARRIER = threading.Barrier(2)


def reset_db():
    with open(DB_FILE, "w", encoding="utf-8") as handle:
        json.dump([], handle)


def worker(worker_id: str):
    with open(DB_FILE, "r", encoding="utf-8") as handle:
        data = json.load(handle)

    print(f"[{worker_id}] read {len(data)} entries")

    # Force both workers to read the same state before either writes.
    START_BARRIER.wait()
    time.sleep(0.2)

    data.append(worker_id)

    with open(DB_FILE, "w", encoding="utf-8") as handle:
        json.dump(data, handle)

    print(f"[{worker_id}] wrote its update")


def main():
    reset_db()
    print("=== WITHOUT COORDINATION ===")
    print("Two workers read the same file, both update it, and both report success.")
    print("The final state is silently wrong.\n")

    thread_a = threading.Thread(target=worker, args=("agent_A",))
    thread_b = threading.Thread(target=worker, args=("agent_B",))

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

    if len(final_state) != 2:
        print("\nSILENT OVERWRITE DETECTED")
        print("Both workers succeeded, but one update vanished.")
    else:
        print("\nUnexpected result: this repro should show the race condition.")


if __name__ == "__main__":
    main()
