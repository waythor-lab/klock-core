from __future__ import annotations

from datetime import datetime

from klock import Klock

from common import TARGET_FILE


CLIENT = Klock.local(agent_id="agent_older", session_id="session-older", priority=100)
RESOURCE_PATH = str(TARGET_FILE)


def now() -> str:
    return datetime.now().strftime("%H:%M:%S.%f")[:-3]


def event(agent_id: str, action: str, detail: str) -> None:
    print(f"[{now()}] {agent_id:<13} {action:<7} {detail}")


def show(label: str, result: dict[str, object]) -> None:
    if result.get("success"):
        print(f"{label}: GRANT ({result['lease_id']})")
    else:
        print(f"{label}: {result['reason']} (wait_time={result.get('wait_time')})")


def main() -> None:
    print("=== WAIT-DIE TRACE ===")
    print("This walkthrough shows the public conflict timeline on one repo file.")
    print("Outcomes are explicit before mutation: GRANT, WAIT, or DIE.\n")

    try:
        CLIENT.register_agent("agent_older", 100)
        CLIENT.register_agent("agent_younger", 200)
        CLIENT.register_agent("agent_newest", 300)
    except Exception as exc:  # pragma: no cover - user-facing demo path
        print(f"Failed to reach the local Klock server: {exc}")
        print("Start it from Klock-OpenSource/:")
        print("  cargo run --release -p klock-cli -- serve")
        raise SystemExit(1) from exc

    event("agent_younger", "REQUEST", f"MUTATES FILE:{RESOURCE_PATH}")
    younger = CLIENT.acquire_lease(
        "agent_younger",
        "session-younger",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    if younger.get("success"):
        event("agent_younger", "GRANT", str(younger["lease_id"]))
    else:
        event("agent_younger", str(younger.get("reason")), f"wait_time={younger.get('wait_time')}")
    show("1. younger acquires", younger)

    event("agent_older", "REQUEST", f"MUTATES FILE:{RESOURCE_PATH}")
    older_wait = CLIENT.acquire_lease(
        "agent_older",
        "session-older",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    event("agent_older", str(older_wait.get("reason")), f"conflict=agent_younger wait_time={older_wait.get('wait_time')}")
    show("2. older collides", older_wait)

    event("agent_newest", "REQUEST", f"MUTATES FILE:{RESOURCE_PATH}")
    newest_die = CLIENT.acquire_lease(
        "agent_newest",
        "session-newest",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    event("agent_newest", str(newest_die.get("reason")), "younger requester aborts and retries later")
    show("3. newest collides", newest_die)

    CLIENT.release_lease(str(younger["lease_id"]))
    event("agent_younger", "RELEASE", str(younger["lease_id"]))
    print("4. younger releases")

    event("agent_older", "RETRY", f"MUTATES FILE:{RESOURCE_PATH}")
    older_grant = CLIENT.acquire_lease(
        "agent_older",
        "session-older",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    if older_grant.get("success"):
        event("agent_older", "GRANT", str(older_grant["lease_id"]))
    else:
        event("agent_older", str(older_grant.get("reason")), f"wait_time={older_grant.get('wait_time')}")
    show("5. older retries", older_grant)

    if older_grant.get("success"):
        CLIENT.release_lease(str(older_grant["lease_id"]))
        event("agent_older", "RELEASE", str(older_grant["lease_id"]))


if __name__ == "__main__":
    main()
