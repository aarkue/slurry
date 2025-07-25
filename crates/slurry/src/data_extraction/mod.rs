
/// Module for extracting data using the `squeue` command
pub mod squeue;

pub use squeue::{get_squeue_res, get_squeue_res_locally, squeue_diff, SqueueMode};

#[cfg(feature = "ssh")]
pub use squeue::{get_squeue_res_ssh};