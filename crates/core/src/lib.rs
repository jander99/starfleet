//! Starfleet core: everything that is not UI.
//!
//! The Tauri shell is a thin layer over [`app::Starfleet`]; keeping the logic
//! here lets it be tested without a window.

pub mod app;
pub mod env;
pub mod herdr;
pub mod log;
pub mod paths;
pub mod pr;
pub mod preflight;
pub mod project;
pub mod stations;
pub mod store;
pub mod view;

pub use app::{DispatchRequest, DispatchResult, Options, Starfleet};

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard};

    /// Process env is global, so every test that sets or clears a variable
    /// serializes on this one lock — a per-module lock is not enough.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Take the env lock, recovering it if an earlier test panicked while
    /// holding it (poisoning would otherwise cascade into unrelated failures).
    pub fn env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}
