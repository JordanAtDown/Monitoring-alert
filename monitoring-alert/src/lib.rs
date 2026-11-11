//! monitoring-alert — long-term temperature monitoring library.
//!
//! The `monitoring-alert` binary is the command-line interface; everything it
//! does lives in this library so that other binaries can reuse it.

pub mod collector;
pub mod config;
pub mod db;
pub mod logger;
pub mod sensors;
pub mod service;
pub mod store;

#[cfg(test)]
mod tests;
