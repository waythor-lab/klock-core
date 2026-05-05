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
from concurrent.futures import ThreadPoolExecutor
from contextlib import suppress
from pathlib import Path
from typing import Any

from klock import KlockHttpClient
from klock_langchain import KlockConflictError, klock_protected
from langchain_core.tools import BaseTool
from langchain_openai import ChatOpenAI
from pydantic import BaseModel, ConfigDict, Field

from common import TARGET_FILE, build_update, feature_count, load_workspace, reset_workspace


BASE_DIR = Path(__file__).resolve().parent
ROOT_DIR = BASE_DIR.parents[1]
RESOURCE_PATH = str(TARGET_FILE)
MODEL = os.environ.get("KLOCK_REAL_AGENT_MODEL", "openai/gpt-oss-20b:free")
BASE_URL = os.environ.get("KLOCK_REAL_AGENT_BASE_URL", "https://openrouter.ai/api/v1")
EXPECTED_MARKERS = ["add-rbac", "add-rate-limit"]
USE_COLOR = os.environ.get("NO_COLOR") is None
STEP_MODE = os.environ.get("KLOCK_DEMO_STEP") == "1"
SAFE_FEATURE_CODE = {
    "add-rbac": """function requireRole(role) {
  return function roleGuard(req, res, next) {
    if (!req.user || req.user.role !== role) {
      return res.status(403).send('Forbidden');
    }

    next();
  };
}""",
    "add-rate-limit": """function authRateLimit(req, res, next) {
  req.rateLimit = { max: 10, windowMs: 60000 };
  next();
}""",
}


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


class FeatureInput(BaseModel):
    feature_marker: str = Field(description="Short feature marker, for example add-rbac.")
    feature_code: str = Field(description="JavaScript code to append to auth.js.")


class UnprotectedWriteTool(BaseTool):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    name: str = "write_auth_feature"
    description: str = "Append a feature block to auth.js without coordination."
    args_schema: type[BaseModel] = FeatureInput

    agent_id: str
    start_barrier: Any

    def _run(self, feature_marker: str, feature_code: str) -> str:
        feature_code = SAFE_FEATURE_CODE.get(feature_marker, feature_code)
        snapshot = load_workspace()
        print(f"  {self.agent_id}: tool read {feature_count(snapshot)} feature blocks")
        self.start_barrier.wait(timeout=20)
        time.sleep(0.25)
        TARGET_FILE.write_text(build_update(snapshot, feature_marker, feature_code), encoding="utf-8")
        return f"wrote {feature_marker}"


class KlockWriteTool(BaseTool):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    name: str = "write_auth_feature"
    description: str = "Append a feature block to auth.js after acquiring a Klock lease."
    args_schema: type[BaseModel] = FeatureInput

    agent_id: str
    session_id: str
    klock_client: Any

    def _run(self, feature_marker: str, feature_code: str) -> str:
        feature_code = SAFE_FEATURE_CODE.get(feature_marker, feature_code)
        decorator = klock_protected(
            klock_client=self.klock_client,
            agent_id=self.agent_id,
            session_id=self.session_id,
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: RESOURCE_PATH,
            predicate="MUTATES",
            ttl_ms=5_000,
            max_retries=5,
        )

        @decorator
        def critical_section(feature_marker: str, feature_code: str) -> str:
            snapshot = load_workspace()
            print(f"  {self.agent_id}: GRANT, tool read {feature_count(snapshot)} feature blocks")
            time.sleep(0.25)
            TARGET_FILE.write_text(build_update(snapshot, feature_marker, feature_code), encoding="utf-8")
            return f"wrote {feature_marker}"

        return critical_section(feature_marker=feature_marker, feature_code=feature_code)


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
    return subprocess.Popen(
        [
            str(server_binary()),
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--storage",
            "memory",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def wait_for_server(client: KlockHttpClient) -> None:
    deadline = time.time() + 5
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            client.register_agent("real_probe", 999)
            return
        except Exception as exc:
            last_error = exc
            time.sleep(0.1)
    raise RuntimeError(f"Klock server did not become ready: {last_error}")


def api_key() -> str:
    api_key = os.environ.get("OPENROUTER_API_KEY") or os.environ.get("OPENAI_API_KEY")
    if not api_key:
        raise SystemExit(
            "Missing API key. Set OPENROUTER_API_KEY, or set OPENAI_API_KEY and "
            "KLOCK_REAL_AGENT_BASE_URL for your provider."
        )
    return api_key


def llm() -> ChatOpenAI:
    return ChatOpenAI(
        base_url=BASE_URL,
        api_key=api_key(),
        model=MODEL,
        temperature=0,
        timeout=30,
        max_retries=1,
    )


def feature_markers() -> list[str]:
    markers: list[str] = []
    for line in load_workspace().splitlines():
        if line.startswith("// FEATURE: "):
            markers.append(line.removeprefix("// FEATURE: "))
    return markers


def print_result(expected: int) -> int:
    actual = feature_count(load_workspace())
    print(f"  expected feature blocks: {expected}")
    print(f"  actual feature blocks:   {actual}")
    print(f"  surviving features:      {json.dumps(feature_markers())}")
    return actual


def print_file_state(label: str) -> int:
    markers = feature_markers()
    actual = feature_count(load_workspace())
    print()
    print(color("1", label))
    print("  file: src/auth.js")
    for marker in EXPECTED_MARKERS:
        if marker in markers:
            print(f"  {color('1;32', 'PRESENT')} {marker}")
        else:
            print(f"  {color('1;31', 'MISSING')} {marker}")
    print(f"  feature blocks: {actual} / {len(EXPECTED_MARKERS)}")
    run_build_check()
    return actual


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


def run_model_agent(agent_id: str, task: str, tool: BaseTool) -> None:
    model = llm().bind_tools([tool], tool_choice=tool.name)
    print(f"  {agent_id}: asking {MODEL} to choose a tool call")
    response = model.invoke(task)
    if not response.tool_calls:
        raise RuntimeError(f"{agent_id} did not return a tool call: {response.content!r}")

    call = response.tool_calls[0]
    print(f"  {agent_id}: model called {call['name']}({call['args'].get('feature_marker')})")

    attempts = 0
    while True:
        try:
            result = tool.invoke(call["args"])
            print(f"  {agent_id}: {result}")
            return
        except KlockConflictError as exc:
            attempts += 1
            print(f"  {agent_id}: {exc.reason}, retry {attempts}")
            if exc.reason != "DIE" or attempts >= 5:
                raise
            time.sleep(0.25)


def rbac_task(agent_id: str) -> str:
    return f"""
You are {agent_id}, a coding agent editing src/auth.js.
Call write_auth_feature exactly once.
Use feature_marker="add-rbac".
Use feature_code with a small JavaScript function named requireRole.
Do not answer in text; return only the tool call.
""".strip()


def rate_limit_task(agent_id: str) -> str:
    return f"""
You are {agent_id}, a coding agent editing src/auth.js.
Call write_auth_feature exactly once.
Use feature_marker="add-rate-limit".
Use feature_code with a small JavaScript function named authRateLimit.
Do not answer in text; return only the tool call.
""".strip()


def run_unprotected_real_agents() -> None:
    reset_workspace()
    barrier = threading.Barrier(2)
    title("1. REAL LLM AGENTS WITHOUT KLOCK")
    print_relevant_file("STARTING FILE WITHOUT KLOCK")
    wait_for_space("Run two real agents WITHOUT KLOCK")
    print("  two model-backed agents both choose the write_auth_feature tool")

    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [
            executor.submit(
                run_model_agent,
                "real_older",
                rbac_task("real_older"),
                UnprotectedWriteTool(agent_id="real_older", start_barrier=barrier),
            ),
            executor.submit(
                run_model_agent,
                "real_younger",
                rate_limit_task("real_younger"),
                UnprotectedWriteTool(agent_id="real_younger", start_barrier=barrier),
            ),
        ]
        for future in futures:
            future.result()
    wait_for_space("Reveal the final file WITHOUT KLOCK")
    actual = print_file_state("FINAL FILE WITHOUT KLOCK")
    print_result(expected=2)
    if actual == 1:
        callout("  result: BUG REPRODUCED - no error, no crash, green build, missing work", "1;31")
    else:
        callout("  result: unexpected result", "1;33")
    print_relevant_file("AUTH.JS AFTER WITHOUT KLOCK")


def run_klock_real_agents(client: KlockHttpClient) -> None:
    reset_workspace()
    client.register_agent("real_older", 100)
    client.register_agent("real_younger", 200)
    title("2. REAL LLM AGENTS WITH KLOCK")
    print_relevant_file("STARTING FILE WITH KLOCK")
    wait_for_space("Run the same real agents WITH KLOCK")
    print("  same model-backed agents, same tool call, protected before mutation")

    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [
            executor.submit(
                run_model_agent,
                "real_older",
                rbac_task("real_older"),
                KlockWriteTool(agent_id="real_older", session_id="real-session-older", klock_client=client),
            ),
            executor.submit(
                run_model_agent,
                "real_younger",
                rate_limit_task("real_younger"),
                KlockWriteTool(agent_id="real_younger", session_id="real-session-younger", klock_client=client),
            ),
        ]
        for future in futures:
            future.result()
    wait_for_space("Reveal the final file WITH KLOCK")
    actual = print_file_state("FINAL FILE WITH KLOCK")
    print_result(expected=2)
    if actual == 2:
        callout("  result: FIX CONFIRMED - both model-authored updates preserved", "1;32")
    else:
        callout("  result: unexpected result", "1;33")
    print_relevant_file("AUTH.JS AFTER WITH KLOCK")


def show_wait_die_trace(client: KlockHttpClient) -> None:
    title("3. CONFLICT TIMELINE: GRANT / WAIT / DIE before mutation")
    wait_for_space("Show the explicit conflict decisions")
    client.register_agent("trace_younger", 200)
    client.register_agent("trace_older", 100)
    client.register_agent("trace_newest", 300)

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


def main() -> None:
    api_key()
    port = free_port()
    server = start_server(port)
    client = KlockHttpClient(f"http://127.0.0.1:{port}", auto_start=False)

    try:
        wait_for_server(client)
        title("KLOCK YC REAL AGENTS DEMO")
        print(f"Model: {MODEL}")
        print(f"Provider URL: {BASE_URL}")
        print(f"Target file: {TARGET_FILE}")
        print("These are real LLM-backed LangChain tool-calling agents.")
        print("They both call valid write tools. Without coordination, one update disappears.")

        run_unprotected_real_agents()
        run_klock_real_agents(client)
        show_wait_die_trace(client)

        title("BOTTOM LINE")
        print(f"  without Klock: {color('1;31', 'real agents call valid tools, green build, lost work')}")
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
