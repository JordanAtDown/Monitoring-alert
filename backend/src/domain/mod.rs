//! The business core: what a temperature snapshot is, how a load level is
//! classified, how a drift is computed.
//!
//! Pure code: no I/O (no database, network, file system or process), so it is
//! trivial to test. It depends on nothing else of the crate; everything else
//! depends on it.

pub mod load;
pub mod model;
pub mod trends;
