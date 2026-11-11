//! Everything technical: the code that talks to the outside world.
//!
//! Each adapter implements a port of `application::ports` or drives a use case
//! of `application`. They are the outermost layer: nothing inside the domain or
//! the application knows they exist.

pub mod config;
pub mod http;
pub mod lhm;
pub mod logging;
pub mod sqlite;
pub mod windows_service;
