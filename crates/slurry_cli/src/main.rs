use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use clap::Parser;
use slurry::{get_squeue_res_locally, squeue_diff, SqueueMode};

/// Run squeue loop and save delta data
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Folder path where to save the results
    #[arg(short, long)]
    path: PathBuf,

    /// Number of seconds to wait in between calls
    #[arg(short, long, default_value_t = 5)]
    delay: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let mut known_jobs = HashMap::default();
    let mut all_ids = HashSet::default();
    let mut i = 0;
    loop {
        squeue_diff(
            || get_squeue_res_locally(&SqueueMode::ALL),
            &args.path,
            &mut known_jobs,
            &mut all_ids,
        )
        .await
        .unwrap();
        i += 1;
        println!("Ran for {} iterations, sleeping...", i);
        tokio::time::sleep(tokio::time::Duration::from_secs(args.delay)).await;
    }
}
