from __future__ import annotations

from pathlib import Path


BASE_DIR = Path(__file__).resolve().parent
WORKSPACE_DIR = BASE_DIR / "workspace"
TARGET_FILE = WORKSPACE_DIR / "src" / "auth.js"

INITIAL_FILE = """// Mock repo workspace: src/auth.js
function requireAuth(req, res, next) {
  if (!req.headers.authorization) {
    return res.status(401).send('Unauthorized');
  }

  next();
}

module.exports = { requireAuth };
"""

FEATURES = {
    "agent_older": (
        "add-rbac",
        """function requireRole(role) {
  return function roleGuard(req, res, next) {
    if (!req.user || req.user.role !== role) {
      return res.status(403).send('Forbidden');
    }

    next();
  };
}""",
    ),
    "agent_younger": (
        "add-rate-limit",
        """function authRateLimit(req, res, next) {
  req.rateLimit = { max: 10, windowMs: 60_000 };
  next();
}""",
    ),
}


def reset_workspace() -> None:
    TARGET_FILE.parent.mkdir(parents=True, exist_ok=True)
    TARGET_FILE.write_text(INITIAL_FILE, encoding="utf-8")


def build_update(source: str, marker: str, code: str) -> str:
    return (
        source.rstrip()
        + f"\n\n// FEATURE: {marker}\n"
        + code.rstrip()
        + "\n"
    )


def feature_count(text: str) -> int:
    return text.count("// FEATURE:")


def load_workspace() -> str:
    return TARGET_FILE.read_text(encoding="utf-8")
