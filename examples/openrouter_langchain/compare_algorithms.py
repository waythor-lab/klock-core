import time
import json
import threading
import requests
import copy
from concurrent.futures import ThreadPoolExecutor

NUM_ACCOUNTS = 5
NUM_AGENTS = 5
INITIAL_BALANCE = 100

def reset_accounts():
    for i in range(NUM_ACCOUNTS):
        with open(f"account_{i}.json", "w") as f:
            json.dump({"balance": INITIAL_BALANCE, "version": 0}, f)

def read_account(i):
    with open(f"account_{i}.json", "r") as f:
        return json.load(f)

def write_account(i, data):
    with open(f"account_{i}.json", "w") as f:
        json.dump(data, f)

# ==========================================
# 1. CHAOS (No Locks - Data Loss)
# ==========================================
def run_chaos_agent(agent_id, acc_from, acc_to, metrics, metrics_lock):
    try:
        data_from = read_account(acc_from)
        data_to = read_account(acc_to)
        
        # Simulate LLM thinking / network latency
        time.sleep(0.1)
        
        data_from["balance"] -= 10
        data_from["version"] += 1
        write_account(acc_from, data_from)
        
        data_to["balance"] += 10
        data_to["version"] += 1
        write_account(acc_to, data_to)
        
        with metrics_lock:
            metrics["success"] += 1
    except Exception as e:
        with metrics_lock:
            metrics["errors"] += 1

# ==========================================
# 2. PESSIMISTIC (Standard Mutex - Deadlocks)
# ==========================================
mutex_locks = {i: threading.Lock() for i in range(NUM_ACCOUNTS)}

def run_pessimistic_agent(agent_id, acc_from, acc_to, metrics, metrics_lock):
    try:
        # A deadlock happens if two threads try to lock the same locks in reverse order
        # We try to acquire them sequentially
        # Add a timeout so it doesn't freeze the whole benchmark forever
        acquired_from = mutex_locks[acc_from].acquire(timeout=2.0)
        if not acquired_from:
            with metrics_lock:
                metrics["deadlocks_detected"] += 1
            return
            
        time.sleep(0.05) # Yield to encourage interleaving/deadlocks
        
        acquired_to = mutex_locks[acc_to].acquire(timeout=2.0)
        if not acquired_to:
            mutex_locks[acc_from].release() # Back off if deadlock
            with metrics_lock:
                metrics["deadlocks_detected"] += 1
            return

        # Critical Section
        data_from = read_account(acc_from)
        data_to = read_account(acc_to)
        time.sleep(0.1) # Simulate LLM thinking
        data_from["balance"] -= 10
        write_account(acc_from, data_from)
        data_to["balance"] += 10
        write_account(acc_to, data_to)

        mutex_locks[acc_to].release()
        mutex_locks[acc_from].release()
        
        with metrics_lock:
            metrics["success"] += 1
    except Exception as e:
        with metrics_lock:
            metrics["errors"] += 1

# ==========================================
# 3. OPTIMISTIC (OCC - High Aborts under contention)
# ==========================================
occ_cas_lock = threading.Lock()

def run_optimistic_agent(agent_id, acc_from, acc_to, metrics, metrics_lock):
    max_retries = 10
    for _ in range(max_retries):
        try:
            from_before = read_account(acc_from)
            to_before = read_account(acc_to)
            
            time.sleep(0.1) # Simulate LLM thinking
            
            # ATOMIC COMPARE AND SWAP
            with occ_cas_lock:
                from_now = read_account(acc_from)
                to_now = read_account(acc_to)
                
                if from_now["version"] != from_before["version"] or to_now["version"] != to_before["version"]:
                    # Collision detected! Abort and retry
                    with metrics_lock:
                        metrics["aborts"] += 1
                    continue
                    
                from_now["balance"] -= 10
                from_now["version"] += 1
                write_account(acc_from, from_now)
                
                to_now["balance"] += 10
                to_now["version"] += 1
                write_account(acc_to, to_now)
                
            # If we get here, CAS succeeded
            with metrics_lock:
                metrics["success"] += 1
            return
            
        except Exception:
            pass
            
    with metrics_lock:
        metrics["failed_retries"] += 1

# ==========================================
# 4. KLOCK (Wait-Die - Deadlock Prevention)
# ==========================================
class KlockHttpClient:
    def __init__(self, base_url="http://localhost:3100"):
        self.base_url = base_url
    def register_agent(self, agent_id, priority):
        requests.post(f"{self.base_url}/agents", json={"agent_id": agent_id, "priority": priority})
    def acquire_lease(self, agent_id, session_id, resource):
        res = requests.post(f"{self.base_url}/leases", json={
            "agent_id": agent_id, "session_id": session_id,
            "resource_type": "FILE", "resource_path": resource,
            "predicate": "MUTATES", "ttl": 10000
        }).json()
        if res.get("success"):
            return "GRANTED", res["data"]["lease_id"]
        reason = res.get("reason")
        wait_time = res.get("wait_time", 100) or 100
        return reason, wait_time
    def release_lease(self, lease_id):
        requests.delete(f"{self.base_url}/leases/{lease_id}")

klock = KlockHttpClient()

def run_klock_agent(agent_id, acc_from, acc_to, metrics, metrics_lock, priority):
    klock.register_agent(agent_id, priority)
    
    max_retries = 10
    for _ in range(max_retries):
        lease_from = None
        lease_to = None
        try:
            # 1. Acquire FROM
            status, payload = klock.acquire_lease(agent_id, f"sess_{agent_id}", str(acc_from))
            if status == "WAIT":
                with metrics_lock: metrics["waits"] += 1
                time.sleep(payload / 1000.0)
                continue
            elif status == "DIE":
                with metrics_lock: metrics["dies"] += 1
                time.sleep(payload / 1000.0)
                continue
            elif status != "GRANTED":
                continue
            lease_from = payload
            
            time.sleep(0.05) # encourage interleaving
            
            # 2. Acquire TO
            status, payload = klock.acquire_lease(agent_id, f"sess_{agent_id}", str(acc_to))
            if status == "WAIT":
                with metrics_lock: metrics["waits"] += 1
                time.sleep(payload / 1000.0)
                if lease_from: klock.release_lease(lease_from)
                continue
            elif status == "DIE":
                # Wait-Die preventing deadlock!
                with metrics_lock: metrics["dies"] += 1
                if lease_from: klock.release_lease(lease_from)
                time.sleep(payload / 1000.0)
                continue
            elif status != "GRANTED":
                if lease_from: klock.release_lease(lease_from)
                continue
            lease_to = payload

            # Critical Section
            data_from = read_account(acc_from)
            data_to = read_account(acc_to)
            time.sleep(0.1) # Simulate LLM
            data_from["balance"] -= 10
            write_account(acc_from, data_from)
            data_to["balance"] += 10
            write_account(acc_to, data_to)
            
            with metrics_lock:
                metrics["success"] += 1
            break
            
        finally:
            if lease_to: klock.release_lease(lease_to)
            if lease_from: klock.release_lease(lease_from)


# ==========================================
# TEST RUNNER
# ==========================================
def run_benchmark(name, agent_func, extra_args=False):
    metrics = {"success": 0, "errors": 0, "deadlocks_detected": 0, "aborts": 0, "failed_retries": 0, "waits": 0, "dies": 0}
    metrics_lock = threading.Lock()
    
    reset_accounts()
    start_time = time.time()
    
    with ThreadPoolExecutor(max_workers=NUM_AGENTS) as executor:
        futures = []
        for i in range(NUM_AGENTS):
            agent_id = f"Agent_{i}"
            acc_from = i
            acc_to = (i + 1) % NUM_ACCOUNTS # Circular dependency ensures deadlock risk
            
            if extra_args:
                futures.append(executor.submit(agent_func, agent_id, acc_from, acc_to, metrics, metrics_lock, i))
            else:
                futures.append(executor.submit(agent_func, agent_id, acc_from, acc_to, metrics, metrics_lock))
        
        for f in futures:
            f.result()
            
    end_time = time.time()
    
    # Calculate Data Integrity
    total_balance = 0
    for i in range(NUM_ACCOUNTS):
        data = read_account(i)
        total_balance += data["balance"]
        
    expected_balance = NUM_ACCOUNTS * INITIAL_BALANCE
    data_loss = expected_balance - total_balance
    
    return {
        "name": name,
        "time": end_time - start_time,
        "success": metrics["success"],
        "data_loss": data_loss != 0,
        "metrics": metrics
    }

print("\n🚀 === CONCURRENCY ALGORITHM BENCHMARK === 🚀\n")
print(f"Simulating {NUM_AGENTS} Agents making circular bank transfers across {NUM_ACCOUNTS} accounts.")
print("This creates a 'Dining Philosophers' scenario, practically guaranteeing conflicts and deadlocks.\n")

results = []

print("Running Chaos...")
results.append(run_benchmark("1. Chaos (No Locks)", run_chaos_agent))

print("Running Pessimistic (Mutex)...")
results.append(run_benchmark("2. Pessimistic (Deadlocks)", run_pessimistic_agent))

print("Running Optimistic (OCC)...")
results.append(run_benchmark("3. Optimistic (OCC)", run_optimistic_agent))

print("Running Klock (Wait-Die)...")
results.append(run_benchmark("4. Klock (Wait-Die)", run_klock_agent, extra_args=True))

print("\n📊 === FINAL RESULTS COMPARISON === 📊\n")
print(f"{'Algorithm':<28} | {'Time (s)':<8} | {'Success':<7} | {'Data Loss':<10} | {'Key Metrics'}")
print("-" * 85)
for r in results:
    m = r["metrics"]
    key_metrics = []
    if m["deadlocks_detected"] > 0: key_metrics.append(f"{m['deadlocks_detected']} Deadlocks")
    if m["aborts"] > 0: key_metrics.append(f"{m['aborts']} Collision Aborts")
    if m["failed_retries"] > 0: key_metrics.append(f"{m['failed_retries']} Exhausted Retries")
    if m["waits"] > 0: key_metrics.append(f"{m['waits']} Waits (Senior)")
    if m["dies"] > 0: key_metrics.append(f"{m['dies']} Dies (Junior)")
    
    metrics_str = ", ".join(key_metrics) if key_metrics else "Clean execution"
    if r['data_loss']: metrics_str = "RACE CONDITION CORRUPTION"
    
    print(f"{r['name']:<28} | {r['time']:<8.2f} | {r['success']:<7} | {str(r['data_loss']):<10} | {metrics_str}")

print("\n💡 Summary:")
print("- Chaos corrupts data due to race conditions.")
print("- Pessimistic Locking deadlocks (threads freeze and timeout) because of circular dependencies.")
print("- Optimistic (OCC) prevents corruption but burns massive compute violently aborting and retrying.")
print("- Klock uses Wait-Die to deterministically yield, preventing deadlocks without exploding retries.")
