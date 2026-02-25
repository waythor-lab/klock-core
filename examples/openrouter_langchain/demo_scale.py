import os
import time
import json
import threading
from typing import Type
from concurrent.futures import ThreadPoolExecutor

from langchain_core.tools import BaseTool
from pydantic import BaseModel, Field
from langchain_openai import ChatOpenAI
from langchain_core.prompts import ChatPromptTemplate

from klock_langchain import klock_protected
import requests

# ==========================================
# 1. SETUP KLOCK HTTP CLIENT
# ==========================================
class KlockHttpClient:
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

klock = KlockHttpClient()

# ==========================================
# 2. SETUP METRICS & FILES
# ==========================================
NUM_AGENTS = 5
FILES = ["data_users.json", "data_logs.json", "data_transactions.json"]
metrics = {
    "llm_api_errors": 0,
    "attempted_writes": 0,
    "successful_writes": 0,
    "klock_aborts": 0,
    "total_time": 0.0
}
metrics_lock = threading.Lock()

def reset_dbs():
    for f in FILES:
        with open(f, "w") as file:
            json.dump([], file)

# ==========================================
# 3. DEFINE DYNAMIC LANGCHAIN TOOL
# ==========================================
class WriteDataInput(BaseModel):
    file_name: str = Field(description="The exact name of the file to write to (e.g., data_users.json).")
    data_entry: str = Field(description="The data payload to append to the file.")

class ProtectedWriteTool(BaseTool):
    name: str = "write_to_database"
    description: str = "Appends a string entry to a specified JSON database array."
    args_schema: Type[BaseModel] = WriteDataInput

    agent_id: str
    session_id: str

    def _run(self, file_name: str, data_entry: str, run_manager=None) -> str:
        if file_name not in FILES:
            return f"Error: Invalid file {file_name}"

        # Dynamically protect the specific file the LLM chose
        decorator = klock_protected(
            klock_client=klock,
            agent_id=self.agent_id,
            session_id=self.session_id,
            resource_type="FILE",
            resource_path_extractor=lambda kwargs: file_name,
            predicate="MUTATES"
        )
        
        @decorator
        def critical_section(file_name=file_name, data_entry=data_entry):
            # Read
            with open(file_name, "r") as f:
                data = json.load(f)
            
            # Simulate processing time to force collisions
            time.sleep(1.0)
            
            # Write
            data.append(data_entry)
            with open(file_name, "w") as f:
                json.dump(data, f)
            return f"Successfully wrote '{data_entry}' to {file_name}"

        # Only count it as an attempted write if the LLM successfully called the tool
        with metrics_lock:
            metrics["attempted_writes"] += 1
            
        try:
            result = critical_section(file_name=file_name, data_entry=data_entry)
            with metrics_lock:
                metrics["successful_writes"] += 1
            return result
        except Exception as e:
            # This happens if Klock issues a DIE command to prevent deadlock
            with metrics_lock:
                metrics["klock_aborts"] += 1
            return f"Error: {str(e)}"

# ==========================================
# 4. CONFIGURE OPENROUTER AGENT
# ==========================================
def run_agent_workflow(agent_idx: int):
    # Register agent with priority (lower idx = higher priority)
    agent_id = f"Agent_{agent_idx}"
    klock.register_agent(agent_id, priority=agent_idx)
    
    # We will ask this agent to write to TWO different files to create complex cross-file contention
    target_file_1 = FILES[agent_idx % len(FILES)]
    target_file_2 = FILES[(agent_idx + 1) % len(FILES)]
    
    llm = ChatOpenAI(
        base_url="https://openrouter.ai/api/v1",
        api_key=os.environ.get("OPENROUTER_API_KEY"),
        model="openai/gpt-oss-20b:free",
        temperature=0.1
    )
    
    tool = ProtectedWriteTool(agent_id=agent_id, session_id=f"session_{agent_id}")
    llm_with_tools = llm.bind_tools([tool])
    
    tasks = [
        f"Append 'Task 1 from {agent_id}' to {target_file_1}. Use the write_to_database tool. Reply with the tool call only.",
        f"Append 'Task 2 from {agent_id}' to {target_file_2}. Use the write_to_database tool. Reply with the tool call only."
    ]
    
    for task in tasks:
        print(f"[{agent_id}] ⏳ Planning...")
        try:
            response = llm_with_tools.invoke(task)
            if response.tool_calls:
                tc = response.tool_calls[0]
                print(f"[{agent_id}] 🧠 Decided to write to {tc['args']['file_name']}")
                tool.invoke(tc["args"])
            else:
                print(f"[{agent_id}] ❌ Did not return a tool call.")
        except Exception as e:
            print(f"[{agent_id}] 💥 LLM API Failed: {e}")
            with metrics_lock:
                metrics["llm_api_errors"] += 1

# ==========================================
# 5. EXECUTION & METRICS
# ==========================================
def main():
    if "OPENROUTER_API_KEY" not in os.environ:
        print("ERROR: Please set OPENROUTER_API_KEY")
        return

    print(f"\n🚀 === STARTING KLOCK BENCHMARK ({NUM_AGENTS} AGENTS, {len(FILES)} FILES) ===")
    reset_dbs()
    
    start_time = time.time()
    
    # Run all agents concurrently in a thread pool
    with ThreadPoolExecutor(max_workers=NUM_AGENTS) as executor:
        futures = [executor.submit(run_agent_workflow, i) for i in range(NUM_AGENTS)]
        for f in futures:
            f.result() # Wait for all to finish
            
    end_time = time.time()
    metrics["total_time"] = end_time - start_time
    
    print("\n📊 === PERFORMANCE METRICS ===")
    print(f"Total Agents: {NUM_AGENTS}")
    print(f"Total Files Operated On: {len(FILES)}")
    print(f"Total Task Instructions: {NUM_AGENTS * 2}")
    print(f"OpenRouter API Rate Limits/Errors: {metrics['llm_api_errors']}")
    print(f"Actual Attempted Concurrent Writes: {metrics['attempted_writes']}")
    print(f"Successfully Resolved & Written: {metrics['successful_writes']}")
    print(f"Klock 'Wait-Die' Aborts (Deadlock Prevented): {metrics['klock_aborts']}")
    print(f"Total Execution Time: {metrics['total_time']:.2f} seconds")
    
    print("\n📁 === FINAL FILE VERIFICATION ===")
    total_entries_verified = 0
    for file in FILES:
        with open(file, "r") as f:
            data = json.load(f)
            total_entries_verified += len(data)
            print(f"- {file}: {len(data)} entries")
            
    if total_entries_verified == metrics["successful_writes"]:
        print(f"\n✅ DATA INTEGRITY VERIFIED: 100% (Matches successful writes)")
        print(f"✅ ZERO Data Loss from Multi-Agent Race Conditions.")
    else:
        print(f"\n❌ DATA LOSS DETECTED: Files have {total_entries_verified} entries, expected {metrics['successful_writes']}.")

if __name__ == "__main__":
    main()
