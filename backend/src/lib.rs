//! Monitoring — long-term temperature drift monitoring library (crate `monitoring-alert`).
//!
//! A simplified hexagonal layout, inside one crate:
//!
//! * [`domain`]      the business core, pure (load levels, drift computation)
//! * [`application`] the use cases and the ports (traits) they need
//! * [`adapters`]    the technical side: SQLite, LibreHardwareMonitor over HTTP,
//!   the web server, the config file, logging, Windows
//! * [`daemon`]      the composition root that plugs the adapters in
//!
//! Dependencies point inwards only: adapters → application → domain.
//! `tests/architecture.rs` enforces it.

pub mod adapters;
pub mod application;
pub mod daemon;
pub mod domain;

#[cfg(test)]
mod tests;
