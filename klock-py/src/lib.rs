use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{json, Value};

use ::klock_core::client::KlockClient as RustClient;
use ::klock_core::types::{LeaseFailureReason, LeaseResult as RustLeaseResult};

const DEFAULT_BASE_URL: &str = "http://localhost:3100";
const DEFAULT_TTL_MS: u64 = 60_000;
const DEFAULT_MAX_RETRIES: u32 = 10;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn default_agent_id() -> String {
    format!("agent-{}", std::process::id())
}

fn default_session_id() -> String {
    format!("session-{}-{}", std::process::id(), now_ms())
}

fn mode_to_predicate(mode: &str) -> PyResult<&'static str> {
    match mode.to_ascii_lowercase().as_str() {
        "read" => Ok("CONSUMES"),
        "mutate" => Ok("MUTATES"),
        "delete" => Ok("DELETES"),
        "rename" => Ok("RENAMES"),
        "provide" => Ok("PROVIDES"),
        "depend" => Ok("DEPENDS_ON"),
        _ => Err(PyRuntimeError::new_err(format!(
            "Unsupported Klock file mode '{}'. Expected one of: read, mutate, delete, rename, provide, depend",
            mode
        ))),
    }
}

#[derive(Debug)]
struct LeaseOutcome {
    success: bool,
    lease_id: Option<String>,
    agent_id: Option<String>,
    resource: Option<String>,
    predicate: Option<String>,
    expires_at: Option<u64>,
    reason: Option<String>,
    wait_time: Option<u64>,
}

enum KlockBackend {
    Embedded(RustClient),
    Http {
        client: KlockHttpClient,
        registered: bool,
    },
}

#[pyclass]
pub struct Klock {
    inner: Arc<Mutex<KlockBackend>>,
    agent_id: String,
    session_id: String,
    priority: u64,
}

#[pyclass]
pub struct KlockFileGuard {
    inner: Arc<Mutex<KlockBackend>>,
    lease_id: Option<String>,
}

/// The Klock coordination client for Python.
/// Manages agent registration, lease acquisition, and conflict resolution.
#[pyclass(unsendable)]
pub struct KlockClient {
    inner: RustClient,
}

/// HTTP client for talking to a local or remote Klock server.
///
/// When `auto_start=True` (default) and the `base_url` is local, the client
/// will spawn a `klock serve` subprocess if no server is reachable. The
/// child process handle is retained so that `shutdown()`, `__exit__`, and
/// `Drop` can terminate it. Servers that were already running when this
/// client was constructed are left untouched.
#[pyclass]
pub struct KlockHttpClient {
    base_url: String,
    api_key: Option<String>,
    timeout_ms: u64,
    auto_start: bool,
    auto_start_disabled_by_env: bool,
    startup_timeout_ms: u64,
    server_command: Vec<String>,
    auto_start_attempted: Mutex<bool>,
    last_started_pid: Mutex<Option<u32>>,
    /// Held only when this client owns the spawned server. Released on
    /// `shutdown()` / context-manager exit / Drop.
    child_handle: Mutex<Option<Child>>,
}

#[pymethods]
impl KlockClient {
    /// Create a new embedded KlockClient.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: RustClient::new(),
        }
    }

    /// Register an agent with a priority (lower = older = higher priority).
    pub fn register_agent(&mut self, agent_id: &str, priority: u64) {
        self.inner.register_agent(agent_id, priority);
    }

    /// Acquire a lease on a resource.
    /// Returns a dict with 'success', 'lease_id', 'reason', and 'wait_time'.
    pub fn acquire_lease<'py>(
        &mut self,
        py: Python<'py>,
        agent_id: &str,
        session_id: &str,
        resource_type: &str,
        resource_path: &str,
        predicate: &str,
        ttl: u64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let result = self.inner.acquire_lease(
            agent_id,
            session_id,
            resource_type,
            resource_path,
            predicate,
            ttl,
        );

        lease_result_to_dict(py, result, resource_type, resource_path)
    }

    /// Release a lease by its ID.
    pub fn release_lease(&mut self, lease_id: &str) -> bool {
        self.inner.release_lease(lease_id)
    }

    /// Get the number of currently active leases.
    pub fn active_lease_count(&self) -> usize {
        self.inner.get_active_leases().len()
    }

    /// Evict expired leases. Returns number evicted.
    pub fn evict_expired(&mut self) -> usize {
        self.inner.evict_expired()
    }
}

#[pymethods]
impl Klock {
    /// Create the default local facade. Uses the local HTTP coordinator with
    /// auto-start so independent processes can coordinate safely.
    #[staticmethod]
    #[pyo3(signature = (
        agent_id = None,
        session_id = None,
        priority = None,
        base_url = DEFAULT_BASE_URL.to_string(),
        api_key = None,
        timeout_ms = 5000,
        auto_start = true,
        startup_timeout_ms = 5000,
        server_command = None
    ))]
    pub fn local(
        agent_id: Option<String>,
        session_id: Option<String>,
        priority: Option<u64>,
        base_url: String,
        api_key: Option<String>,
        timeout_ms: u64,
        auto_start: bool,
        startup_timeout_ms: u64,
        server_command: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let agent_id = agent_id.unwrap_or_else(default_agent_id);
        let session_id = session_id.unwrap_or_else(default_session_id);
        let priority = priority.unwrap_or_else(now_ms);
        let client = KlockHttpClient::new(
            base_url,
            api_key,
            timeout_ms,
            auto_start,
            startup_timeout_ms,
            server_command,
        );
        Ok(Self {
            inner: Arc::new(Mutex::new(KlockBackend::Http {
                client,
                registered: false,
            })),
            agent_id,
            session_id,
            priority,
        })
    }

    /// Create an embedded facade. This is simple and server-free, but only
    /// coordinates work inside this process.
    #[staticmethod]
    #[pyo3(signature = (agent_id = None, session_id = None, priority = None))]
    pub fn embedded(
        agent_id: Option<String>,
        session_id: Option<String>,
        priority: Option<u64>,
    ) -> Self {
        let agent_id = agent_id.unwrap_or_else(default_agent_id);
        let session_id = session_id.unwrap_or_else(default_session_id);
        let priority = priority.unwrap_or_else(now_ms);
        let mut client = RustClient::new();
        client.register_agent(&agent_id, priority);
        Self {
            inner: Arc::new(Mutex::new(KlockBackend::Embedded(client))),
            agent_id,
            session_id,
            priority,
        }
    }

    /// Acquire a raw lease. Exposed so integrations such as klock-langchain
    /// can use the facade anywhere an existing client is accepted.
    #[pyo3(signature = (
        agent_id,
        session_id,
        resource_type,
        resource_path,
        predicate,
        ttl
    ))]
    pub fn acquire_lease<'py>(
        &self,
        py: Python<'py>,
        agent_id: &str,
        session_id: &str,
        resource_type: &str,
        resource_path: &str,
        predicate: &str,
        ttl: u64,
    ) -> PyResult<Bound<'py, PyDict>> {
        ensure_default_registered(&self.inner, &self.agent_id, self.priority)?;
        let outcome = acquire_backend(
            &self.inner,
            agent_id,
            session_id,
            resource_type,
            resource_path,
            predicate,
            ttl,
        )?;
        lease_outcome_to_dict(py, &outcome, resource_type, resource_path)
    }

    /// Register an additional agent on the underlying coordinator.
    pub fn register_agent(&self, agent_id: &str, priority: u64) -> PyResult<()> {
        let mut guard = self.inner.lock().unwrap();
        match &mut *guard {
            KlockBackend::Embedded(client) => {
                client.register_agent(agent_id, priority);
                Ok(())
            }
            KlockBackend::Http { client, registered } => {
                client.register_agent(agent_id, priority)?;
                if agent_id == self.agent_id {
                    *registered = true;
                }
                Ok(())
            }
        }
    }

    /// Release a raw lease. Exposed for integrations and manual use.
    pub fn release_lease(&self, lease_id: &str) -> PyResult<bool> {
        release_backend(&self.inner, lease_id)
    }

    /// Protect a file operation with a lease. The returned object is a
    /// context manager and releases the lease on exit.
    #[pyo3(signature = (
        path,
        mode = "mutate",
        ttl_ms = DEFAULT_TTL_MS,
        max_retries = DEFAULT_MAX_RETRIES
    ))]
    pub fn file(
        &self,
        path: &str,
        mode: &str,
        ttl_ms: u64,
        max_retries: u32,
    ) -> PyResult<KlockFileGuard> {
        let predicate = mode_to_predicate(mode)?;
        ensure_default_registered(&self.inner, &self.agent_id, self.priority)?;
        let lease_id = acquire_with_retries(
            &self.inner,
            &self.agent_id,
            &self.session_id,
            "FILE",
            path,
            predicate,
            ttl_ms,
            max_retries,
        )?;
        Ok(KlockFileGuard {
            inner: Arc::clone(&self.inner),
            lease_id: Some(lease_id),
        })
    }
}

#[pymethods]
impl KlockFileGuard {
    pub fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __exit__(
        &mut self,
        _exc_type: PyObject,
        _exc_val: PyObject,
        _exc_tb: PyObject,
    ) -> PyResult<bool> {
        self.release()?;
        Ok(false)
    }

    pub fn release(&mut self) -> PyResult<bool> {
        if let Some(lease_id) = self.lease_id.take() {
            release_backend(&self.inner, &lease_id)
        } else {
            Ok(false)
        }
    }
}

impl Drop for KlockFileGuard {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[pymethods]
impl KlockHttpClient {
    #[new]
    #[pyo3(signature = (
        base_url = "http://localhost:3100".to_string(),
        api_key = None,
        timeout_ms = 5000,
        auto_start = true,
        startup_timeout_ms = 5000,
        server_command = None
    ))]
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        timeout_ms: u64,
        auto_start: bool,
        startup_timeout_ms: u64,
        server_command: Option<Vec<String>>,
    ) -> Self {
        let auto_start_disabled_by_env = auto_start_disabled_by_env();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            timeout_ms,
            auto_start: auto_start && !auto_start_disabled_by_env,
            auto_start_disabled_by_env,
            startup_timeout_ms,
            server_command: server_command.unwrap_or_else(default_server_command),
            auto_start_attempted: Mutex::new(false),
            last_started_pid: Mutex::new(None),
            child_handle: Mutex::new(None),
        }
    }

    /// Terminate the auto-started server subprocess, if this client owns one.
    /// Servers that were already running when this client was constructed are
    /// left running. Safe to call multiple times.
    pub fn shutdown(&self) {
        let mut guard = self.child_handle.lock().unwrap();
        if let Some(mut child) = guard.take() {
            // Best-effort: ignore the result. The child may have already
            // exited on its own (CTRL-C, timeout, etc.).
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Context-manager entry. Returns self so the bound name in `with` is
    /// the client. Implemented as `__enter__` to match Python convention.
    pub fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context-manager exit: terminates the auto-started server.
    pub fn __exit__(&self, _exc_type: PyObject, _exc_val: PyObject, _exc_tb: PyObject) -> bool {
        self.shutdown();
        // Returning false signals "do not suppress any in-flight exception".
        false
    }

    /// Returns true when localhost auto-start is currently enabled.
    pub fn auto_start_enabled(&self) -> bool {
        self.auto_start
    }

    /// Returns true when auto-start was disabled by KLOCK_DISABLE_AUTOSTART.
    pub fn auto_start_disabled_by_env(&self) -> bool {
        self.auto_start_disabled_by_env
    }

    /// Returns the PID from the last auto-started local server, if any.
    pub fn last_started_pid(&self) -> Option<u32> {
        *self.last_started_pid.lock().unwrap()
    }

    /// Register an agent against the Klock server.
    pub fn register_agent(&self, agent_id: &str, priority: u64) -> PyResult<()> {
        let response = self.request_json(
            "POST",
            "/agents",
            Some(json!({
                "agent_id": agent_id,
                "priority": priority,
            })),
        )?;

        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(PyRuntimeError::new_err(extract_error(&response)))
        }
    }

    /// Acquire a lease from the Klock server.
    pub fn acquire_lease<'py>(
        &self,
        py: Python<'py>,
        agent_id: &str,
        session_id: &str,
        resource_type: &str,
        resource_path: &str,
        predicate: &str,
        ttl: u64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let response = self.request_json(
            "POST",
            "/leases",
            Some(json!({
                "agent_id": agent_id,
                "session_id": session_id,
                "resource_type": resource_type,
                "resource_path": resource_path,
                "predicate": predicate,
                "ttl": ttl,
            })),
        )?;

        if response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let dict = PyDict::new(py);
            let data = response
                .get("data")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    PyRuntimeError::new_err("Klock server returned a malformed lease response")
                })?;

            dict.set_item("success", true)?;
            dict.set_item("lease_id", value_as_str(data.get("lease_id"))?)?;
            dict.set_item("agent_id", value_as_str(data.get("agent_id"))?)?;
            dict.set_item("resource", value_as_str(data.get("resource"))?)?;

            if let Some(predicate_value) = data.get("predicate").and_then(Value::as_str) {
                dict.set_item("predicate", predicate_value)?;
            }

            if let Some(expires_at) = data.get("expires_at").and_then(Value::as_u64) {
                dict.set_item("expires_at", expires_at)?;
            }

            Ok(dict)
        } else {
            let dict = PyDict::new(py);
            dict.set_item("success", false)?;
            dict.set_item(
                "reason",
                response
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("CONFLICT"),
            )?;
            dict.set_item(
                "wait_time",
                response
                    .get("wait_time")
                    .and_then(Value::as_u64)
                    .unwrap_or(1000),
            )?;
            Ok(dict)
        }
    }

    /// Release a lease by its ID.
    pub fn release_lease(&self, lease_id: &str) -> PyResult<bool> {
        let response = self.request_json("DELETE", &format!("/leases/{}", lease_id), None)?;
        Ok(response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Renew a lease heartbeat.
    pub fn heartbeat_lease(&self, lease_id: &str) -> PyResult<bool> {
        let response =
            self.request_json("POST", &format!("/leases/{}/heartbeat", lease_id), None)?;
        Ok(response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// List currently active leases.
    pub fn list_leases<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let response = self.request_json("GET", "/leases", None)?;
        if !response
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(PyRuntimeError::new_err(extract_error(&response)));
        }

        let list = PyList::empty(py);
        let leases = response
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                PyRuntimeError::new_err("Klock server returned a malformed lease list")
            })?;

        for lease in leases {
            let lease_dict = PyDict::new(py);
            let Some(lease_obj) = lease.as_object() else {
                return Err(PyRuntimeError::new_err(
                    "Klock server returned a malformed lease entry",
                ));
            };

            if let Some(id) = lease_obj.get("id").and_then(Value::as_str) {
                lease_dict.set_item("id", id)?;
            }
            if let Some(agent_id) = lease_obj.get("agent_id").and_then(Value::as_str) {
                lease_dict.set_item("agent_id", agent_id)?;
            }
            if let Some(resource) = lease_obj.get("resource").and_then(Value::as_str) {
                lease_dict.set_item("resource", resource)?;
            }
            if let Some(predicate) = lease_obj.get("predicate").and_then(Value::as_str) {
                lease_dict.set_item("predicate", predicate)?;
            }
            if let Some(expires_at) = lease_obj.get("expires_at").and_then(Value::as_u64) {
                lease_dict.set_item("expires_at", expires_at)?;
            }

            list.append(lease_dict)?;
        }

        Ok(list)
    }
}

impl KlockHttpClient {
    fn request_json(&self, method: &str, path: &str, payload: Option<Value>) -> PyResult<Value> {
        if path != "/health" {
            self.ensure_server()?;
        }

        let url = format!("{}{}", self.base_url, path);
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build();

        let request = match method {
            "GET" => agent.get(&url),
            "POST" => agent.post(&url),
            "DELETE" => agent.delete(&url),
            _ => {
                return Err(PyRuntimeError::new_err(format!(
                    "Unsupported Klock HTTP method '{}'",
                    method
                )))
            }
        };

        let request = if let Some(api_key) = &self.api_key {
            request.set("Authorization", &format!("Bearer {}", api_key))
        } else {
            request
        };

        let response = match payload {
            Some(body) => request
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            None => request.call(),
        };

        match response {
            Ok(resp) => read_json_response(resp),
            Err(ureq::Error::Status(_, resp)) => read_json_response(resp),
            Err(ureq::Error::Transport(err)) => Err(PyRuntimeError::new_err(format!(
                "Failed to reach Klock server at {}: {}",
                self.base_url, err
            ))),
        }
    }

    fn ensure_server(&self) -> PyResult<()> {
        if self.health_check().is_ok() {
            return Ok(());
        }

        if !is_local_base_url(&self.base_url) {
            return Ok(());
        }

        if !self.auto_start {
            let disabled_reason = if self.auto_start_disabled_by_env {
                " by KLOCK_DISABLE_AUTOSTART"
            } else {
                ""
            };

            return Err(PyRuntimeError::new_err(format!(
                "No Klock server is reachable at {}. Auto-start is disabled{}. Start it manually with: {}",
                self.base_url,
                disabled_reason,
                format_server_command(&self.server_command),
            )));
        }

        {
            let attempted = self.auto_start_attempted.lock().unwrap();
            if *attempted {
                let pid_note = self
                    .last_started_pid
                    .lock()
                    .unwrap()
                    .map(|pid| format!(" Last attempted PID: {}.", pid))
                    .unwrap_or_default();

                return Err(PyRuntimeError::new_err(format!(
                    "Klock server is still unavailable at {} after an earlier auto-start attempt.{} Start it manually with: {}",
                    self.base_url,
                    pid_note,
                    format_server_command(&self.server_command),
                )));
            }
        }

        let Some((program, args)) = self.server_command.split_first() else {
            return Err(PyRuntimeError::new_err(
                "Klock auto-start is enabled but no server command is configured",
            ));
        };

        eprintln!(
            "Starting local Klock server for {} using: {}",
            self.base_url,
            format_server_command(&self.server_command),
        );

        let child = Command::new(program)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|err| {
                PyRuntimeError::new_err(format!(
                    "Failed to auto-start Klock server with command {:?}: {}",
                    self.server_command, err
                ))
            })?;

        let pid = child.id();
        *self.auto_start_attempted.lock().unwrap() = true;
        *self.last_started_pid.lock().unwrap() = Some(pid);
        // Retain ownership of the spawned process so we can terminate it on
        // shutdown(). Without this, a parent process exit leaves the server
        // orphaned.
        *self.child_handle.lock().unwrap() = Some(child);

        eprintln!(
            "Started local Klock server for {} with PID {}",
            self.base_url, pid
        );

        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(self.startup_timeout_ms) {
            // Briefly check whether the child has already exited.
            // Holding the lock across the entire loop would deadlock with
            // any concurrent shutdown() — instead we lock per-iteration.
            {
                let mut guard = self.child_handle.lock().unwrap();
                if let Some(child) = guard.as_mut() {
                    if let Ok(Some(status)) = child.try_wait() {
                        // Child died early: drop it from the handle so we
                        // don't keep a stale Child around.
                        *guard = None;
                        return Err(PyRuntimeError::new_err(format!(
                            "Klock server process exited before becoming healthy at {}. Exit status: {}. Start it manually with: {}",
                            self.base_url,
                            status,
                            format_server_command(&self.server_command),
                        )));
                    }
                }
            }

            if self.health_check().is_ok() {
                eprintln!("Klock server is healthy at {} (PID {})", self.base_url, pid);
                return Ok(());
            }
            sleep(Duration::from_millis(150));
        }

        Err(PyRuntimeError::new_err(format!(
            "Klock server did not become healthy at {} within {}ms after auto-starting PID {}. Start it manually with: {}",
            self.base_url,
            self.startup_timeout_ms,
            pid,
            format_server_command(&self.server_command),
        )))
    }

    fn health_check(&self) -> Result<(), ureq::Error> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build();

        let request = if let Some(api_key) = &self.api_key {
            agent
                .get(&format!("{}/health", self.base_url))
                .set("Authorization", &format!("Bearer {}", api_key))
        } else {
            agent.get(&format!("{}/health", self.base_url))
        };

        match request.call() {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Drop for KlockHttpClient {
    fn drop(&mut self) {
        // Best-effort: kill any auto-started server we still own. This is the
        // safety net for code paths that don't use `with KlockHttpClient(...)`.
        // Python's GC timing means this is not deterministic; users who need
        // immediate shutdown should call `shutdown()` or use the context
        // manager.
        if let Ok(mut guard) = self.child_handle.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn default_server_command() -> Vec<String> {
    if let Ok(command) = std::env::var("KLOCK_SERVER_COMMAND") {
        let parts: Vec<String> = command
            .split_whitespace()
            .map(|part| part.to_string())
            .collect();
        if !parts.is_empty() {
            return parts;
        }
    }

    if command_exists("klock") {
        return vec!["klock".to_string(), "serve".to_string()];
    }

    if let Some(workspace_root) = find_workspace_root() {
        return vec![
            "cargo".to_string(),
            "run".to_string(),
            "--release".to_string(),
            "--manifest-path".to_string(),
            workspace_root.join("Cargo.toml").display().to_string(),
            "-p".to_string(),
            "klock-cli".to_string(),
            "--".to_string(),
            "serve".to_string(),
        ];
    }

    vec!["klock".to_string(), "serve".to_string()]
}

fn auto_start_disabled_by_env() -> bool {
    matches!(
        std::env::var("KLOCK_DISABLE_AUTOSTART")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .is_ok()
}

fn find_workspace_root() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    for dir in cwd.ancestors() {
        if dir.join("Cargo.toml").exists() && dir.join("klock-cli").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn format_server_command(command: &[String]) -> String {
    if command.is_empty() {
        "<no command configured>".to_string()
    } else {
        command.join(" ")
    }
}

fn is_local_base_url(base_url: &str) -> bool {
    base_url.contains("localhost") || base_url.contains("127.0.0.1")
}

fn read_json_response(response: ureq::Response) -> PyResult<Value> {
    let raw = response.into_string().map_err(|err| {
        PyRuntimeError::new_err(format!("Failed to read Klock response: {}", err))
    })?;

    if raw.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(&raw).map_err(|err| {
            PyRuntimeError::new_err(format!("Failed to parse Klock response JSON: {}", err))
        })
    }
}

fn lease_result_to_dict<'py>(
    py: Python<'py>,
    result: RustLeaseResult,
    resource_type: &str,
    resource_path: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);

    match result {
        RustLeaseResult::Success { lease } => {
            dict.set_item("success", true)?;
            dict.set_item("lease_id", &lease.id)?;
            dict.set_item("agent_id", &lease.agent_id)?;
            dict.set_item("resource", format!("{}:{}", resource_type, resource_path))?;
            dict.set_item("expires_at", lease.expires_at)?;
        }
        RustLeaseResult::Failure {
            reason, wait_time, ..
        } => {
            let reason_str = match reason {
                LeaseFailureReason::Wait => "WAIT",
                LeaseFailureReason::Die => "DIE",
                LeaseFailureReason::Conflict => "CONFLICT",
                LeaseFailureReason::ResourceLocked => "RESOURCE_LOCKED",
                LeaseFailureReason::SessionExpired => "SESSION_EXPIRED",
                LeaseFailureReason::StorageUnavailable => "STORAGE_UNAVAILABLE",
            };
            dict.set_item("success", false)?;
            dict.set_item("reason", reason_str)?;
            dict.set_item("wait_time", wait_time)?;
        }
    }

    Ok(dict)
}

fn lease_result_to_outcome(
    result: RustLeaseResult,
    resource_type: &str,
    resource_path: &str,
) -> LeaseOutcome {
    match result {
        RustLeaseResult::Success { lease } => LeaseOutcome {
            success: true,
            lease_id: Some(lease.id),
            agent_id: Some(lease.agent_id),
            resource: Some(format!("{}:{}", resource_type, resource_path)),
            predicate: None,
            expires_at: Some(lease.expires_at),
            reason: None,
            wait_time: None,
        },
        RustLeaseResult::Failure {
            reason, wait_time, ..
        } => {
            let reason_str = match reason {
                LeaseFailureReason::Wait => "WAIT",
                LeaseFailureReason::Die => "DIE",
                LeaseFailureReason::Conflict => "CONFLICT",
                LeaseFailureReason::ResourceLocked => "RESOURCE_LOCKED",
                LeaseFailureReason::SessionExpired => "SESSION_EXPIRED",
                LeaseFailureReason::StorageUnavailable => "STORAGE_UNAVAILABLE",
            };
            LeaseOutcome {
                success: false,
                lease_id: None,
                agent_id: None,
                resource: None,
                predicate: None,
                expires_at: None,
                reason: Some(reason_str.to_string()),
                wait_time,
            }
        }
    }
}

fn value_to_outcome(value: &Value) -> LeaseOutcome {
    if value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let data = value.get("data").and_then(Value::as_object);
        LeaseOutcome {
            success: true,
            lease_id: data
                .and_then(|d| d.get("lease_id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            agent_id: data
                .and_then(|d| d.get("agent_id"))
                .and_then(Value::as_str)
                .map(str::to_string),
            resource: data
                .and_then(|d| d.get("resource"))
                .and_then(Value::as_str)
                .map(str::to_string),
            predicate: data
                .and_then(|d| d.get("predicate"))
                .and_then(Value::as_str)
                .map(str::to_string),
            expires_at: data
                .and_then(|d| d.get("expires_at"))
                .and_then(Value::as_u64),
            reason: None,
            wait_time: None,
        }
    } else {
        LeaseOutcome {
            success: false,
            lease_id: None,
            agent_id: None,
            resource: None,
            predicate: None,
            expires_at: None,
            reason: Some(
                value
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("CONFLICT")
                    .to_string(),
            ),
            wait_time: value
                .get("wait_time")
                .and_then(Value::as_u64)
                .or(Some(1000)),
        }
    }
}

fn lease_outcome_to_dict<'py>(
    py: Python<'py>,
    outcome: &LeaseOutcome,
    resource_type: &str,
    resource_path: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("success", outcome.success)?;
    if outcome.success {
        dict.set_item(
            "lease_id",
            outcome.lease_id.as_deref().ok_or_else(|| {
                PyRuntimeError::new_err("Klock returned success without lease_id")
            })?,
        )?;
        if let Some(agent_id) = outcome.agent_id.as_deref() {
            dict.set_item("agent_id", agent_id)?;
        }
        dict.set_item(
            "resource",
            outcome
                .resource
                .clone()
                .unwrap_or_else(|| format!("{}:{}", resource_type, resource_path)),
        )?;
        if let Some(predicate) = outcome.predicate.as_deref() {
            dict.set_item("predicate", predicate)?;
        }
        if let Some(expires_at) = outcome.expires_at {
            dict.set_item("expires_at", expires_at)?;
        }
    } else {
        dict.set_item("reason", outcome.reason.as_deref().unwrap_or("CONFLICT"))?;
        dict.set_item("wait_time", outcome.wait_time)?;
    }
    Ok(dict)
}

fn acquire_backend(
    inner: &Arc<Mutex<KlockBackend>>,
    agent_id: &str,
    session_id: &str,
    resource_type: &str,
    resource_path: &str,
    predicate: &str,
    ttl: u64,
) -> PyResult<LeaseOutcome> {
    let mut guard = inner.lock().unwrap();
    match &mut *guard {
        KlockBackend::Embedded(client) => Ok(lease_result_to_outcome(
            client.acquire_lease(
                agent_id,
                session_id,
                resource_type,
                resource_path,
                predicate,
                ttl,
            ),
            resource_type,
            resource_path,
        )),
        KlockBackend::Http { client, .. } => {
            let value = client.request_json(
                "POST",
                "/leases",
                Some(json!({
                    "agent_id": agent_id,
                    "session_id": session_id,
                    "resource_type": resource_type,
                    "resource_path": resource_path,
                    "predicate": predicate,
                    "ttl": ttl,
                })),
            )?;
            Ok(value_to_outcome(&value))
        }
    }
}

fn release_backend(inner: &Arc<Mutex<KlockBackend>>, lease_id: &str) -> PyResult<bool> {
    let mut guard = inner.lock().unwrap();
    match &mut *guard {
        KlockBackend::Embedded(client) => Ok(client.release_lease(lease_id)),
        KlockBackend::Http { client, .. } => {
            let value = client.request_json("DELETE", &format!("/leases/{}", lease_id), None)?;
            Ok(value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false))
        }
    }
}

fn ensure_default_registered(
    inner: &Arc<Mutex<KlockBackend>>,
    agent_id: &str,
    priority: u64,
) -> PyResult<()> {
    let mut guard = inner.lock().unwrap();
    match &mut *guard {
        KlockBackend::Embedded(_) => Ok(()),
        KlockBackend::Http { client, registered } => {
            if *registered {
                return Ok(());
            }
            client.register_agent(agent_id, priority)?;
            *registered = true;
            Ok(())
        }
    }
}

fn acquire_with_retries(
    inner: &Arc<Mutex<KlockBackend>>,
    agent_id: &str,
    session_id: &str,
    resource_type: &str,
    resource_path: &str,
    predicate: &str,
    ttl: u64,
    max_retries: u32,
) -> PyResult<String> {
    let mut retries = 0;
    let mut total_wait_ms = 0;
    loop {
        let outcome = acquire_backend(
            inner,
            agent_id,
            session_id,
            resource_type,
            resource_path,
            predicate,
            ttl,
        )?;
        if outcome.success {
            return outcome
                .lease_id
                .ok_or_else(|| PyRuntimeError::new_err("Klock returned success without lease_id"));
        }

        let reason = outcome.reason.as_deref().unwrap_or("CONFLICT");
        if reason == "WAIT" && retries < max_retries {
            let wait_ms = outcome.wait_time.unwrap_or(1000);
            sleep(Duration::from_millis(wait_ms));
            total_wait_ms += wait_ms;
            retries += 1;
            continue;
        }

        let message = if reason == "WAIT" {
            format!(
                "Klock exceeded max WAIT retries for {} on {} after {}ms",
                agent_id, resource_path, total_wait_ms
            )
        } else {
            format!("Klock denied {} on {}: {}", agent_id, resource_path, reason)
        };
        return Err(PyRuntimeError::new_err(message));
    }
}

fn extract_error(response: &Value) -> String {
    response
        .get("error")
        .and_then(Value::as_str)
        .or_else(|| response.get("reason").and_then(Value::as_str))
        .unwrap_or("Unknown Klock server error")
        .to_string()
}

fn value_as_str(value: Option<&Value>) -> PyResult<&str> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| PyRuntimeError::new_err("Klock server returned a malformed response field"))
}

/// The Klock Python module.
#[pymodule]
fn klock(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Klock>()?;
    m.add_class::<KlockFileGuard>()?;
    m.add_class::<KlockClient>()?;
    m.add_class::<KlockHttpClient>()?;
    Ok(())
}
