
/// Module for extracting data using the `squeue` command
pub mod squeue;

pub use squeue::{get_squeue_res, get_squeue_res_locally, get_squeue_res_ssh, squeue_diff, SqueueMode};