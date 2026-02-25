import os
import time
import json
import threading
from typing import Optional, Type, Dict, Any

from langchain_core.tools import BaseTool
from pydantic import BaseModel, Field


from klock_langchain import klock_protected

# ==========================================
# 1. SETUP KLOCK CLIENT & AGENTS
# ==========================================
import requests

class KlockHttpClient:
    """A minimal Python client that talks to the Klock CLI server for distributed coordination."""
    def __init__(self, base_url="http://localhost:3100"):
        self.base_url = base_url
        
    def register_agent(self, agent_id, priority):
        requests.post(f"{self.base_url}/agents", json={"agent_id": agent_id, "priority": priority})
        
    def acquire_lease(self, agent_id, session_id, resource_type, resource_path, predicate, ttl):
        res = requests.post(f"{self.base_url}/leases", json={
            "agent_id": agent_id, "session_id": session_id,
            "resource_type": resource_type, "resource_path": resource_path,
            "predicate": predicate, "ttl": ttl
        }).json()
        
        if res.get("success"):
            return {"success": True, "lease_id": res["data"]["lease_id"]}
        else:
            return {"success": False, "reason": res.get("reason", "CONFLICT"), "wait_time": res.get("wait_time", 1000)}
            
    def release_lease(self, lease_id):
        requests.delete(f"{self.base_url}/leases/{lease_id}")

# Connect to the local Klock daemon (klock-cli serve)
klock = KlockHttpClient()
# Agent A is Senior (lower number = higher priority = WAITs for juniors)
klock.register_agent("agent_A", 100)
# Agent B is Junior (higher number = lower priority = DIEs to prevent deadlock)
klock.register_agent("agent_B", 200)

DB_FILE = "database.json"

def reset_db():
    with open(DB_FILE, "w") as f:
        json.dump([], f)

# ==========================================
# 2. DEFINE LANGCHAIN TOOLS
# ==========================================
class AppendAuthorInput(BaseModel):
    author_name: str = Field(description="The name of the author to append to the database.")

class ProtectedAppendAuthorTool(BaseTool):
    name: str = "append_author"
    description: str = "Appends an author name to the JSON database."
    args_schema: Type[BaseModel] = AppendAuthorInput

    # Inject the Agent ID and Session dynamically
    agent_id: str
    session_id: str

    def _run(self, author_name: str, run_manager=None) -> str:
        # Dynamically apply the klock decorator just for this invocation 
        # (This is simulating our @klock_protected decorator)
        decorator = klock_protected(
            klock_client=klock,
            agent_id=self.agent_id,
            session_id=self.session_id,
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: DB_FILE,
            predicate="MUTATES"
        )
        
        @decorator
        def critical_section(author_name=author_name):
            print(f"[{self.agent_id}] 🔒 Lease Acquired. Reading {DB_FILE}...")
            # Read
            with open(DB_FILE, "r") as f:
                data = json.load(f)
            
            # Simulate processing time (creates the race condition)
            print(f"[{self.agent_id}] Processing...")
            time.sleep(2)
            
            # Write
            data.append(author_name)
            with open(DB_FILE, "w") as f:
                json.dump(data, f)
            print(f"[{self.agent_id}] 🔓 Saved. Lease Released.")
            return f"Successfully appended {author_name}"

        try:
            return critical_section(author_name=author_name)
        except Exception as e:
            return f"Error: Tool execution halted. {str(e)}"

# ==========================================
# 3. CONFIGURE OPENROUTER AGENTS
# ==========================================
from langchain_openai import ChatOpenAI

# Use OpenRouter via the OpenAI SDK wrapper
llm = ChatOpenAI(
    base_url="https://openrouter.ai/api/v1",
    api_key=os.environ.get("OPENROUTER_API_KEY"),
    model="openai/gpt-oss-20b:free",
    temperature=0.1
)

def run_agent(agent_name: str, author_name: str):
    """Run a single LangChain agent explicitly attempting to write to the DB."""
    tool = ProtectedAppendAuthorTool(agent_id=agent_name, session_id=f"session_{agent_name}")
    
    # Explicitly bind the tool to force the model to use it
    llm_with_tools = llm.bind_tools([tool])
    
    print(f"\n🚀 [{agent_name}] Planning task...")
    try:
        # 1. Ask the LLM to plan the action (it should return a tool_call)
        response = llm_with_tools.invoke(f"Append the name '{author_name}' to the database. Use the append_author tool. Do nothing else. Just return the tool call.")
        
        if not response.tool_calls:
            print(f"[{agent_name}] ❌ LLM failed to invoke tool. It replied: {response.content}")
            return
            
        print(f"[{agent_name}] 🧠 LLM decided to invoke: {response.tool_calls[0]['name']}")
        
        # 2. Execute the protected tool with the LLM's arguments
        tc = response.tool_calls[0]
        tool_result = tool.invoke(tc["args"])
        print(f"[{agent_name}] ✅ Tool finished. Result: {tool_result}")
        
    except Exception as e:
        print(f"\n💥 [{agent_name}] Agent stopped: {e}\n")

# ==========================================
# 4. EXECUTE THE DEMO (THE RACE)
# ==========================================
def main():
    if "OPENROUTER_API_KEY" not in os.environ:
        print("ERROR: Please set your OPENROUTER_API_KEY in the environment before running.")
        return

    print("=== STARTING KLOCK LANGCHAIN DEMO ===")
    reset_db()
    
    # We spawn two threads to simulate two AI agents making network requests simultaneously
    # Agent A (Senior)
    thread_a = threading.Thread(target=run_agent, args=("agent_A", "Alice AI"))
    # Agent B (Junior)
    thread_b = threading.Thread(target=run_agent, args=("agent_B", "Bob Bot"))
    
    thread_a.start()
    thread_b.start()
    
    thread_a.join()
    thread_b.join()
    
    print("\n=== FINAL DATABASE STATE ===")
    with open(DB_FILE, "r") as f:
        data = json.load(f)
        print(json.dumps(data, indent=2))
        
    if len(data) == 2:
        print("✅ SUCCESS: Zero Data Loss. Klock prevented the Multi-Agent Race Condition (MARC).")
    else:
        print("💥 FAILED: Data Loss Occurred.")

if __name__ == "__main__":
    main()
