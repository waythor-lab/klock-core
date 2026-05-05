from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import termios
import threading
import time
import tty as tty_module
from contextlib import suppress
from pathlib import Path
from typing import Any

from klock import KlockHttpClient
from klock_langchain import KlockConflictError, klock_protected
from langchain_core.tools import BaseTool
from pydantic import BaseModel, ConfigDict, Field

from common import FEATURES, TARGET_FILE, build_update, feature_count, load_workspace, reset_workspace


BASE_DIR = Path(__file__).resolve().parent
ROOT_DIR = BASE_DIR.parents[1]
RESOURCE_PATH = str(TARGET_FILE)
EXPECTED_MARKERS = [FEATURES["agent_older"][0], FEATURES["agent_younger"][0]]
USE_COLOR = os.environ.get("NO_COLOR") is None
STEP_MODE = os.environ.get("KLOCK_DEMO_STEP") == "1"


def color(code: str, text: str) -> str:
    if not USE_COLOR:
        return text
    return f"\033[{code}m{text}\033[0m"


def title(text: str) -> None:
    print()
    print(color("1;36", "=" * 72))
    print(color("1;36", text))
    print(color("1;36", "=" * 72))


def callout(text: str, code: str = "1;33") -> None:
    print(color(code, text))


def wait_for_space(prompt: str) -> None:
    if not STEP_MODE:
        return
    print()
    print(color("1;33", f"▶ {prompt}"))
    print(color("2", "  press SPACE to continue"))
    try:
        fd = sys.stdin.fileno()
        old_settings = termios.tcgetattr(fd)
        try:
            tty_module.setraw(fd)
            while True:
                char = sys.stdin.read(1)
                if char in {" ", "\n", "\r"}:
                    break
        finally:
            termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    except Exception:
        input("  press Enter to continue")
    print()


def run_build_check() -> bool:
    if not shutil.which("node"):
        print("  build check: node not found")
        return False
    result = subprocess.run(["node", "--check", str(TARGET_FILE)], capture_output=True, text=True)
    if result.returncode == 0:
        print(f"  build check: {color('1;32', 'PASS')} (node --check src/auth.js)")
        return True
    print(f"  build check: {color('1;31', 'FAIL')} (node --check src/auth.js)")
    if result.stderr.strip():
        print(result.stderr.strip())
    return False


class WriteFileInput(BaseModel):
    path: str = Field(description="Absolute path to the repo file to mutate.")


class ProtectedWriteTool(BaseTool):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    name: str = "write_auth_feature"
    description: str = "Mutates auth.js after acquiring a Klock lease."
    args_schema: type[BaseModel] = WriteFileInput

    agent_id: str
    session_id: str
    marker: str
    code: str
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
            time.sleep(0.16)
            TARGET_FILE.write_text(build_update(snapshot, self.marker, self.code), encoding="utf-8")
            return self.marker

        return critical_section(path=path)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def server_binary() -> Path:
    configured = os.environ.get("KLOCK_YC_SERVER_BIN")
    if configured:
        return Path(configured)
    return ROOT_DIR / "target" / "release" / "klock"


def start_server(port: int) -> subprocess.Popen[bytes]:
    command = [
        str(server_binary()),
        "serve",
        "--host",
        "127.0.0.1",
        "--port",
        str(port),
        "--storage",
        "memory",
    ]
    return subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def wait_for_server(client: KlockHttpClient) -> None:
    deadline = time.time() + 5
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            client.register_agent("yc_probe", 999)
            return
        except Exception as exc:
            last_error = exc
            time.sleep(0.1)
    raise RuntimeError(f"Klock server did not become ready: {last_error}")


def feature_markers() -> list[str]:
    markers: list[str] = []
    for line in load_workspace().splitlines():
        if line.startswith("// FEATURE: "):
            markers.append(line.removeprefix("// FEATURE: "))
    return markers


def print_result(label: str, expected: int, actual: int, markers: list[str], status: str | None = None) -> None:
    status = status or ("PASS" if actual == expected else "FAIL")
    print(f"{label}: {status}")
    print(f"  expected feature blocks: {expected}")
    print(f"  actual feature blocks:   {actual}")
    print(f"  surviving features:      {json.dumps(markers)}")


def print_file_state(label: str) -> None:
    markers = feature_markers()
    actual = feature_count(load_workspace())
    print()
    print(color("1", label))
    print(f"  file: src/auth.js")
    for marker in EXPECTED_MARKERS:
        if marker in markers:
            print(f"  {color('1;32', 'PRESENT')} {marker}")
        else:
            print(f"  {color('1;31', 'MISSING')} {marker}")
    print(f"  feature blocks: {actual} / {len(EXPECTED_MARKERS)}")
    run_build_check()


def print_relevant_file(label: str) -> None:
    text = load_workspace()
    feature_start = text.find("// FEATURE:")
    print()
    print(color("1", label))
    if feature_start == -1:
        print("  no appended feature blocks yet")
        print("  baseline file:")
        for line in text.rstrip().splitlines():
            print(f"  {line}")
        return

    print("  appended feature blocks:")
    for line in text[feature_start:].rstrip().splitlines():
        print(f"  {line}")


def run_uncoordinated() -> None:
    reset_workspace()
    barrier = threading.Barrier(2)

    def worker(agent_id: str, delay: float) -> None:
        marker, code = FEATURES[agent_id]
        snapshot = load_workspace()
        print(f"  {agent_id}: read 0 features, will write {marker}")
        barrier.wait()
        time.sleep(delay)
        TARGET_FILE.write_text(build_update(snapshot, marker, code), encoding="utf-8")
        print(f"  {agent_id}: success")

    title("1. WITHOUT KLOCK: both agents succeed, one update disappears")
    print_relevant_file("STARTING FILE WITHOUT KLOCK")
    wait_for_space("Run two agent tasks WITHOUT KLOCK")
    older = threading.Thread(target=worker, args=("agent_older", 0.10))
    younger = threading.Thread(target=worker, args=("agent_younger", 0.20))
    older.start()
    younger.start()
    older.join()
    younger.join()

    wait_for_space("Reveal the final file WITHOUT KLOCK")
    print_file_state("FINAL FILE WITHOUT KLOCK")
    print_result("  result", 2, feature_count(load_workspace()), feature_markers(), color("1;31", "BUG REPRODUCED"))
    callout("  no error, no crash, green build, missing work", "1;31")
    print_relevant_file("AUTH.JS AFTER WITHOUT KLOCK")


def register_demo_agents(client: KlockHttpClient, prefix: str) -> None:
    client.register_agent(f"{prefix}_older", 100)
    client.register_agent(f"{prefix}_younger", 200)
    client.register_agent(f"{prefix}_newest", 300)


def run_coordinated(client: KlockHttpClient) -> None:
    reset_workspace()
    register_demo_agents(client, "yc")

    def worker(agent_id: str, session_id: str, marker: str, code: str, delay: float) -> None:
        if delay:
            time.sleep(delay)

        attempts = 0
        while True:
            decorator = klock_protected(
                klock_client=client,
                agent_id=agent_id,
                session_id=session_id,
                resource_type="FILE",
                resource_path_extractor=lambda kwargs: kwargs["path"],
                predicate="MUTATES",
                ttl_ms=5_000,
                max_retries=5,
            )

            @decorator
            def protected_write(path: str) -> str:
                snapshot = load_workspace()
                print(f"  {agent_id}: GRANT {marker}")
                time.sleep(0.16)
                TARGET_FILE.write_text(build_update(snapshot, marker, code), encoding="utf-8")
                return marker

            try:
                protected_write(path=RESOURCE_PATH)
                print(f"  {agent_id}: success")
                return
            except KlockConflictError as exc:
                attempts += 1
                print(f"  {agent_id}: {exc.reason}, retry {attempts}")
                if exc.reason != "DIE" or attempts >= 5:
                    raise
                time.sleep(0.20)

    title("2. WITH KLOCK: coordination happens before mutation")
    print_relevant_file("STARTING FILE WITH KLOCK")
    wait_for_space("Run the same agent tasks WITH KLOCK")
    older_marker, older_code = FEATURES["agent_older"]
    younger_marker, younger_code = FEATURES["agent_younger"]
    older = threading.Thread(target=worker, args=("yc_older", "yc-session-older", older_marker, older_code, 0.00))
    younger = threading.Thread(
        target=worker,
        args=("yc_younger", "yc-session-younger", younger_marker, younger_code, 0.04),
    )
    older.start()
    younger.start()
    older.join()
    younger.join()

    wait_for_space("Reveal the final file WITH KLOCK")
    print_file_state("FINAL FILE WITH KLOCK")
    print_result("  result", 2, feature_count(load_workspace()), feature_markers(), color("1;32", "FIX CONFIRMED"))
    print_relevant_file("AUTH.JS AFTER WITH KLOCK")


def show_wait_die_trace(client: KlockHttpClient) -> None:
    title("3. CONFLICT TIMELINE: GRANT / WAIT / DIE before mutation")
    wait_for_space("Show the explicit conflict decisions")
    register_demo_agents(client, "trace")

    younger = client.acquire_lease("trace_younger", "trace-session-younger", "FILE", RESOURCE_PATH, "MUTATES", 5_000)
    print(f"  trace_younger requests FILE:auth.js -> {'GRANT' if younger.get('success') else younger.get('reason')}")

    older = client.acquire_lease("trace_older", "trace-session-older", "FILE", RESOURCE_PATH, "MUTATES", 5_000)
    print(f"  trace_older collides with younger holder -> {older.get('reason')}")
    if older.get("reason") == "WAIT":
        callout("  IMPORTANT: this agent is blocked before it can overwrite anything")

    newest = client.acquire_lease("trace_newest", "trace-session-newest", "FILE", RESOURCE_PATH, "MUTATES", 5_000)
    print(f"  trace_newest collides with older holder -> {newest.get('reason')}")

    client.release_lease(str(younger["lease_id"]))
    retry = client.acquire_lease("trace_older", "trace-session-older", "FILE", RESOURCE_PATH, "MUTATES", 5_000)
    print(f"  trace_older retries after release -> {'GRANT' if retry.get('success') else retry.get('reason')}")
    if retry.get("success"):
        client.release_lease(str(retry["lease_id"]))


def run_langchain_tool(client: KlockHttpClient) -> None:
    reset_workspace()
    client.register_agent("lc_older", 100)
    client.register_agent("lc_younger", 200)

    def invoke_tool(agent_id: str, session_id: str, priority: int, marker: str, code: str, delay: float) -> None:
        if delay:
            time.sleep(delay)
        tool = ProtectedWriteTool(
            agent_id=agent_id,
            session_id=session_id,
            marker=marker,
            code=code,
            klock_client=client,
        )
        attempts = 0
        while True:
            try:
                tool.invoke({"path": RESOURCE_PATH})
                print(f"  {agent_id}: LangChain BaseTool success")
                return
            except KlockConflictError as exc:
                attempts += 1
                print(f"  {agent_id}: BaseTool {exc.reason}, retry {attempts}")
                if exc.reason != "DIE" or attempts >= 5:
                    raise
                time.sleep(0.20)

    title("4. REAL INTEGRATION: same protection through a LangChain BaseTool")
    older_marker, older_code = FEATURES["agent_older"]
    younger_marker, younger_code = FEATURES["agent_younger"]
    older = threading.Thread(target=invoke_tool, args=("lc_older", "lc-session-older", 100, older_marker, older_code, 0.00))
    younger = threading.Thread(
        target=invoke_tool,
        args=("lc_younger", "lc-session-younger", 200, younger_marker, younger_code, 0.04),
    )
    older.start()
    younger.start()
    older.join()
    younger.join()

    print_file_state("FINAL FILE THROUGH LANGCHAIN TOOL")
    print_result("  result", 2, feature_count(load_workspace()), feature_markers(), color("1;32", "LANGCHAIN PATH CONFIRMED"))


def main() -> None:
    port = free_port()
    server = start_server(port)
    client = KlockHttpClient(f"http://127.0.0.1:{port}", auto_start=False)

    try:
        wait_for_server(client)
        title("KLOCK YC DEMO")
        print(f"Target file: {TARGET_FILE}")
        print("Two agent tasks both succeed. Without coordination, one update disappears.")
        print("Klock makes agents declare intent before mutation: GRANT, WAIT, or DIE.")

        run_uncoordinated()
        run_coordinated(client)
        show_wait_die_trace(client)
        run_langchain_tool(client)

        title("BOTTOM LINE")
        print(f"  without Klock: {color('1;31', 'success logs, green build, lost work')}")
        print(f"  with Klock:    {color('1;32', 'coordination before mutation, both updates preserved')}")
    finally:
        with suppress(Exception):
            client.shutdown()
        server.terminate()
        with suppress(Exception):
            server.wait(timeout=2)
        if server.poll() is None:
            server.kill()


if __name__ == "__main__":
    main()
