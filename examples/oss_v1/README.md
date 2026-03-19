# Klock OSS v1 Proof

These scripts are the public proof set for **Klock OSS v1: local multi-agent repo coordination**.

They all target the same repo-like workspace file: `workspace/src/auth.js`.

## Install

```bash
pip install klock klock-langchain
```

Or run the fully scripted version from `Klock-OpenSource/`:

```bash
./scripts/run_oss_v1_demo.sh
```

## 1. Reproduce the failure

```bash
cd Klock-OpenSource/examples/oss_v1
python3 without_klock.py
```

Expected result:

- both agents report success
- only **1** feature block survives
- terminal ends with `SILENT OVERWRITE DETECTED`

## 2. Use localhost auto-start by default

The coordinated scripts now auto-start `klock serve` when they target `http://localhost:3100` and can find a launch command.

Priority order:

1. `KLOCK_SERVER_COMMAND`
2. installed `klock` binary
3. source-tree `cargo run --release -p klock-cli -- serve`

When auto-start happens, the SDK logs the base URL, launch command, and PID.

Disable auto-start with:

- `KLOCK_DISABLE_AUTOSTART=1`
- `auto_start=False` in Python

## 3. Run the coordinated version

```bash
cd Klock-OpenSource/examples/oss_v1
python3 with_klock.py
```

Expected result:

- both feature blocks survive
- the younger agent may print `DIE` and retry
- terminal ends with `WAIT-DIE COORDINATION CONFIRMED`

This proves coordination for cooperative agents that use the SDK. It does not prove filesystem-level enforcement against arbitrary processes.

## 4. Show the protocol outcomes directly

```bash
cd Klock-OpenSource/examples/oss_v1
python3 wait_die_trace.py
```

Expected result:

- first request: `GRANT`
- older agent colliding with younger: `WAIT`
- newest agent colliding with older holder: `DIE`
- retry after release: `GRANT`

## 5. Validate the real LangChain tool surface

```bash
cd Klock-OpenSource/examples/oss_v1
python3 langchain_base_tool_demo.py
```

Expected result:

- the script uses a real `langchain_core.tools.BaseTool`
- both feature blocks survive
- terminal ends with `LANGCHAIN TOOL PROTECTION CONFIRMED`
