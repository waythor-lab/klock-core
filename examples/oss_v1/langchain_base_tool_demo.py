from __future__ import annotations

import threading
import time
from typing import Any

from klock import KlockHttpClient
from klock_langchain import KlockConflictError, klock_protected
from langchain_core.tools import BaseTool
from pydantic import BaseModel, ConfigDict, Field

from common import FEATURES, TARGET_FILE, build_update, feature_count, load_workspace, reset_workspace


RESOURCE_PATH = str(TARGET_FILE)


class WriteFileInput(BaseModel):
    path: str = Field(description="Absolute path to the repo file to mutate.")


class ProtectedWriteTool(BaseTool):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    name: str = "write_auth_feature"
    description: str = "Mutates auth.js after acquiring a Klock lease."
    args_schema: type[BaseModel] = WriteFileInput

    agent_id: str
    session_id: str
    feature_marker: str
    feature_code: str
    klock_client: Any

    def _run(self, path: str) -> str:
        decorator = klock_protected(
            klock_client=self.klock_client,
            agent_id=self.agent_id,
            session_id=self.session_id,
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: kwargs["path"],
            predicate="MUTATES",
            ttl_ms=5_000,
            max_retries=5,
        )

        @decorator
        def critical_section(path: str) -> str:
            snapshot = load_workspace()
            print(f"[{self.agent_id}] LangChain tool GRANT {self.feature_marker}")
            time.sleep(0.2)
            TARGET_FILE.write_text(build_update(snapshot, self.feature_marker, self.feature_code), encoding="utf-8")
            return self.feature_marker

        return critical_section(path=path)


def ensure_server() -> None:
    client = KlockHttpClient()
    try:
        client.register_agent("langchain_older", 100)
        client.register_agent("langchain_younger", 200)
    except Exception as exc:  # pragma: no cover - user-facing demo path
        print(f"Failed to reach the local Klock server: {exc}")
        print("Start it from Klock-OpenSource/:")
        print("  cargo run --release -p klock-cli -- serve")
        raise SystemExit(1) from exc


def run_tool(agent_id: str, session_id: str, marker: str, code: str, start_delay: float = 0.0) -> None:
    if start_delay:
        time.sleep(start_delay)

    tool = ProtectedWriteTool(
        agent_id=agent_id,
        session_id=session_id,
        feature_marker=marker,
        feature_code=code,
        klock_client=KlockHttpClient(),
    )

    attempts = 0
    while True:
        try:
            tool.invoke({"path": RESOURCE_PATH})
            print(f"[{agent_id}] LangChain tool finished safely")
            return
        except KlockConflictError as exc:
            attempts += 1
            print(f"[{agent_id}] LangChain tool {exc.reason}; retry {attempts}")
            if exc.reason != "DIE" or attempts >= 5:
                raise
            time.sleep(0.25)


def main() -> None:
    reset_workspace()
    ensure_server()

    print("=== LANGCHAIN BASETOOL DEMO ===")
    print("This uses a real LangChain BaseTool surface backed by Klock.\n")

    older_marker, older_code = FEATURES["agent_older"]
    younger_marker, younger_code = FEATURES["agent_younger"]

    older = threading.Thread(
        target=run_tool,
        args=("langchain_older", "langchain-session-older", older_marker, older_code, 0.0),
    )
    younger = threading.Thread(
        target=run_tool,
        args=("langchain_younger", "langchain-session-younger", younger_marker, younger_code, 0.05),
    )

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
        print("\nLANGCHAIN TOOL PROTECTION CONFIRMED")
    else:
        print("\nUnexpected result: the LangChain tool demo should preserve both updates.")


if __name__ == "__main__":
    main()
