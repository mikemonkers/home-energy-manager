//! Shared test utilities.
//!
//! Currently only contains [`with_isolated_config_dir`], used by tests in
//! multiple modules to run against an ephemeral `~/.givenergy-local/`-shaped
//! config directory without polluting the user's real settings file.

#![cfg(test)]

use std::sync::Mutex;

/// Global lock that serializes all tests touching `GIVENERGY_LOCAL_CONFIG_DIR`.
///
/// `std::env::set_var` is process-global, so any test that flips the env var
/// must hold this mutex for the duration of its body — otherwise a parallel
/// sibling test could read the wrong config dir or have its dir torn down
/// mid-flight. Each module's local mutex (in `poll::tests`,
/// `server::api::tests`, etc.) is replaced by this single shared one so that
/// tests across the crate serialize against each other.
pub static CONFIG_DIR_MUTEX: Mutex<()> = Mutex::new(());

/// Run `body` against an isolated `GIVENERGY_LOCAL_CONFIG_DIR` pointing at a
/// fresh temp directory. Holds [`CONFIG_DIR_MUTEX`] for the duration, creates
/// the dir, sets the env var, runs `body`, then removes the env var and
/// deletes the dir on the way out.
///
/// Use this for any test that calls `Settings::load()` or `Settings::save()`
/// (directly or indirectly through `update_settings`, `AppState::new`,
/// `persist_cosy_active`, etc.) so it doesn't read or write the user's real
/// `~/.givenergy-local/settings.json`.
pub fn with_isolated_config_dir<T>(body: impl FnOnce() -> T) -> T {
    let _guard = CONFIG_DIR_MUTEX.lock().unwrap();

    let tmp = std::env::temp_dir().join(format!(
        "givenergy-local-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::create_dir_all(&tmp);
    // SAFETY: tests are single-binary; the mutex serializes all tests that
    // touch this env var.
    std::env::set_var("GIVENERGY_LOCAL_CONFIG_DIR", &tmp);

    let result = body();

    std::env::remove_var("GIVENERGY_LOCAL_CONFIG_DIR");
    let _ = std::fs::remove_dir_all(&tmp);

    result
}
