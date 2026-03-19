# Agent Support

Klock OSS v1 works today with **real agents that can call the SDK or wrap their file-mutating tools**.

That means OSS v1 is coordination for cooperative agents. It is not yet filesystem-level enforcement of the workspace itself.

## Supported today

### LangChain

Supported through `klock-langchain`.

This is the canonical OSS v1 path:

- local Klock server
- `KlockHttpClient`
- `klock_protected(...)` on file tools

### Custom Python agents

Supported through:

- `KlockClient` for embedded coordination
- `KlockHttpClient` for local-server coordination

If your agent can call Python functions before it reads and writes a repo file, it can use Klock today.

### Custom JavaScript / TypeScript agents

Supported through:

- `KlockClient`
- `KlockHttpClient`

If your agent runtime can call the JS SDK before mutating a file, it can use Klock today.

## Partially supported

### Other frameworks with tool wrappers

Frameworks like LangGraph or CrewAI are not the polished v1 path, but they can still use Klock at the tool layer if you wrap file-mutating operations yourself.

## Not transparent yet

These are **not** plug-and-play today unless you add a wrapper or integration:

- Claude Code
- Cursor agents
- Cline
- Roo Code
- OpenHands

The reason is simple: OSS v1 is not shipping a transparent agent wrapper yet. It ships an explicit integration surface first.

## Precise claim for launch

Use this wording:

> Klock OSS v1 works with LangChain today and with any custom Python or JavaScript agent that can call the SDK before mutating shared repo files.

Avoid this wording:

> works with any agent automatically
