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

    /// Remembers the given variables and puts them back on drop, even if the
    /// test panics, so one test cannot leak env changes into the next.
    pub struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvRestore {
        pub fn new(keys: &[&'static str]) -> Self {
            Self(keys.iter().map(|k| (*k, std::env::var_os(k))).collect())
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }
}
