//! Application settings and configuration.
//!
//! Manages persistent user preferences such as the inverter IP
//! address, poll interval, and other runtime configuration,
//! stored via Tauri's app data directory.
//!
//! For the MVP, settings are simply re-exported from the poll module.

pub use crate::inverter::poll::PollSettings as AppSettings;
