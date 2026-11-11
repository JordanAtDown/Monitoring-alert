//! `GET /api/update`: is a newer version available, and where to get it.

use crate::application::updates::{UpdateSnapshot, UpdateStatus};

pub fn execute(status: &UpdateStatus) -> UpdateSnapshot {
    status.snapshot()
}
