//! One file per endpoint: it reads the request, calls its use case, and serializes the answer.
//! What an endpoint concretely does lives in the use case it calls, never here.

pub mod cooling;
pub mod curves;
pub mod health;
pub mod overview;
pub mod sensor;
pub mod series;
pub mod status;
pub mod summary;
pub mod trend;
pub mod update;
