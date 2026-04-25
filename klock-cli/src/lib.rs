//! Library facade for the klock-cli binary.
//!
//! Exposes the HTTP `Router` and `AppState` so integration tests can
//! exercise the server end-to-end via `tower::ServiceExt::oneshot` without
//! binding a real network socket.

pub mod handlers;
pub mod server;

pub use server::{build_router, create_client, AppState};
