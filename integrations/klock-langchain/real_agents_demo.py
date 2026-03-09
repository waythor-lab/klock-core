import json
import time
import os
import requests
from typing import TypedDict
from concurrent.futures import ThreadPoolExecutor
from langgraph.graph import StateGraph, START, END
from langchain_core.tools import tool

# ==============================================================================
# SETUP: Shared Express.js API Files
# ==============================================================================
REPO_DIR = "mock_express_api"
AUTH_FILE = os.path.join(REPO_DIR, "auth.js")

def setup_repo():
    """Create a mock repository with an existing auth.js file."""
    os.makedirs(REPO_DIR, exist_ok=True)
    initial_auth_code = """// Auth Middleware
function requireAuth(req, res, next) {
    if (!req.headers.authorization) return res.status(401).send('Unauthorized');
    next();
}
module.exports = { requireAuth };
"""
    with open(AUTH_FILE, "w") as f:
        f.write(initial_auth_code)

# ==============================================================================
# LLM CONFIGURATION (OpenRouter)
# ==============================================================================
# Set your key in the environment: export OPENROUTER_API_KEY="sk-or-v1-..."
OPENROUTER_API_KEY = os.environ.get("OPENROUTER_API_KEY", "dummy_key")

def call_llm(prompt: str) -> str:
    """Calls an actual LLM (e.g., Llama 3 or Haiku) via OpenRouter."""
    if OPENROUTER_API_KEY == "dummy_key":
        # Fallback if no key is provided, so the script doesn't crash immediately
        time.sleep(2) # simulate network
        return f"// Added by LLM: {prompt}\\n"

    headers = {
        "Authorization": f"Bearer {OPENROUTER_API_KEY}",
        "Content-Type": "application/json"
    }
    
    # We use a more impressive model for the swarm
    payload = {
        "model": "openai/gpt-oss-20b:free", 
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.1
    }
    
    response = requests.post("https://openrouter.ai/api/v1/chat/completions", headers=headers, json=payload)
    if response.status_code == 200:
        content = response.json()['choices'][0]['message']['content'].strip()
        # Clean up any markdown code blocks if the LLM includes them
        if "```" in content:
            content = content.split("```")[1]
            if content.startswith("javascript"): content = content[10:]
            if content.startswith("js"): content = content[2:]
        return content.strip()
    return f"// LLM ERROR: {response.status_code}"

# ==============================================================================
# LANGCHAIN TOOL (UNSAFE)
# ==============================================================================
@tool
def refactor_auth_file_unsafe(feature_request: str) -> str:
    """Reads auth.js, asks the LLM to add a new feature, and writes it back."""
    
    # 1. Read current state of the file
    with open(AUTH_FILE, "r") as f:
        current_code = f.read()
        
    # 2. Call REAL LLM (Network latency creates the massive race condition window)
    prompt = f"I have this code:\n\n{current_code}\n\nPlease implement this feature: {feature_request}. Return ONLY the valid javascript code, no explanation, no markdown blocks."
    new_code = call_llm(prompt)
    
    # 3. Write new state back to the file
    updated_code = current_code + "\n\n// --- FEATURE: " + feature_request + " ---\n" + new_code + "\n"
    with open(AUTH_FILE, "w") as f:
        f.write(updated_code)
        
    return f"Successfully added {feature_request}"

# ==============================================================================
# LANGCHAIN TOOL (SAFE / KLOCK PROTECTED)
# ==============================================================================
from klock_langchain import klock_protected

# Mock Klock kernel for standalone demo
class MockKlockClient:
    def __init__(self): self.locked = False
    def acquire_lease(self, agent_id, session_id, resource_type, resource_path, predicate, ttl_ms):
        if self.locked:
            print(f"🚦 [Klock] Wait-Die Triggered: {resource_path} is locked. Agent {agent_id} is WAITING...")
            return {"success": False, "reason": "WAIT", "wait_time": 2000}
        self.locked = True
        print(f"🔒 [Klock] Agent {agent_id} ACQUIRED write lease on {resource_path}.")
        return {"success": True, "lease_id": f"lease_{agent_id}"}
    def release_lease(self, lease_id):
        print(f"🔓 [Klock] RELEASED {lease_id}.\n")
        self.locked = False

klock_client = MockKlockClient()

@tool
@klock_protected(
    klock_client=klock_client,
    agent_id="real_llm_agent",
    session_id="swarm_001",
    resource_type="FILE",
    resource_path_extractor=lambda kwargs: AUTH_FILE,
    predicate="MUTATES"
)
def refactor_auth_file_safe(feature_request: str) -> str:
    """Exact same logic, but protected by Klock's Wait-Die scheduling."""
    with open(AUTH_FILE, "r") as f:
        current_code = f.read()
        
    prompt = f"I have this code:\n\n{current_code}\n\nPlease implement this feature: {feature_request}. Return ONLY the valid javascript code, no explanation, no markdown blocks."
    new_code = call_llm(prompt)
    
    updated_code = current_code + "\n\n// --- FEATURE: " + feature_request + " ---\n" + new_code + "\n"
    with open(AUTH_FILE, "w") as f:
        f.write(updated_code)
        
    return f"Successfully added {feature_request}"

# ==============================================================================
# LANGGRAPH SWARM EXECUTION
# ==============================================================================
class State(TypedDict): pass

def run_real_swarm(tool_to_use):
    setup_repo()
    builder = StateGraph(State)
    
    # 5 Agents editing the exact same auth.js file simultaneously
    features = [
        "Add a Passport.js Google Strategy for OAuth2 login.",
        "Implement a JSON Web Token (JWT) refresh token rotation logic.",
        "Add an express-rate-limit middleware to the auth routes.",
        "Create a Role-Based Access Control (RBAC) middleware for 'admin' roles.",
        "Implement a SAML 2.0 Service Provider (SP) configuration."
    ]
    
    for i, feature in enumerate(features):
        node_name = f"agent_{i+1}"
        
        def make_node(req):
            def node_func(state):
                tool_to_use.invoke({"feature_request": req})
                return {}
            return node_func
            
        builder.add_node(node_name, make_node(feature))
        builder.add_edge(START, node_name)
        builder.add_edge(node_name, END)
        
    graph = builder.compile()
    
    print(f"🚀 Launching 5 LLM agents concurrently targeting {AUTH_FILE}...")
    graph.invoke({})
    
    with open(AUTH_FILE, "r") as f:
        final_code = f.read()
        
    print("\\n" + "="*50)
    print("FINAL auth.js CONTENT:")
    print("="*50)
    print(final_code)
    
    # Count how many features actually survived
    if "LLM ERROR" in final_code:
        survival_count = final_code.count("LLM ERROR")
    elif "Added by LLM" in final_code:
        survival_count = final_code.count("Added by LLM")
    else:
        survival_count = sum(1 for f in features if f.split()[0] in final_code)
        
    print(f"\\n📊 DATA SURVIVAL RATE: {survival_count} / 5 features survived.")

if __name__ == "__main__":
    print("\\n" + "#"*60)
    print("PART 1: REAL AGENTS (WITHOUT KLOCK)")
    print("#"*60)
    run_real_swarm(refactor_auth_file_unsafe)
    
    time.sleep(2)
    
    print("\\n" + "#"*60)
    print("PART 2: REAL AGENTS (WITH KLOCK)")
    print("#"*60)
    run_real_swarm(refactor_auth_file_safe)
