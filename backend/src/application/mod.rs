//! Use cases and the ports they need.
//!
//! Depends on the domain only. The outside world (sensors, storage, web, files)
//! is reached through the traits of [`ports`], which the adapters implement and
//! the composition root (`daemon`, the binaries) plugs in.

pub mod clock;
pub mod collect;
pub mod health;
pub mod install;
pub mod monitor;
pub mod overview;
pub mod ports;
pub mod queries;
pub mod status;
pub mod updates;
