from __future__ import annotations

from klock import KlockHttpClient

from common import TARGET_FILE


CLIENT = KlockHttpClient()
RESOURCE_PATH = str(TARGET_FILE)


def show(label: str, result: dict[str, object]) -> None:
    if result.get("success"):
        print(f"{label}: GRANT ({result['lease_id']})")
    else:
        print(f"{label}: {result['reason']} (wait_time={result.get('wait_time')})")


def main() -> None:
    print("=== WAIT-DIE TRACE ===")
    print("This walkthrough shows all three public outcomes on the same file.\n")

    try:
        CLIENT.register_agent("agent_older", 100)
        CLIENT.register_agent("agent_younger", 200)
        CLIENT.register_agent("agent_newest", 300)
    except Exception as exc:  # pragma: no cover - user-facing demo path
        print(f"Failed to reach the local Klock server: {exc}")
        print("Start it from Klock-OpenSource/:")
        print("  cargo run --release -p klock-cli -- serve")
        raise SystemExit(1) from exc

    younger = CLIENT.acquire_lease(
        "agent_younger",
        "session-younger",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    show("1. younger acquires", younger)

    older_wait = CLIENT.acquire_lease(
        "agent_older",
        "session-older",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    show("2. older collides", older_wait)

    newest_die = CLIENT.acquire_lease(
        "agent_newest",
        "session-newest",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    show("3. newest collides", newest_die)

    CLIENT.release_lease(str(younger["lease_id"]))
    print("4. younger releases")

    older_grant = CLIENT.acquire_lease(
        "agent_older",
        "session-older",
        "FILE",
        RESOURCE_PATH,
        "MUTATES",
        5_000,
    )
    show("5. older retries", older_grant)

    if older_grant.get("success"):
        CLIENT.release_lease(str(older_grant["lease_id"]))


if __name__ == "__main__":
    main()
