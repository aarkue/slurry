use std::{collections::HashMap, time::SystemTime};

use anyhow::Error;
use async_ssh2_tokio::client::{AuthMethod, Client, ServerCheckMethod};
use chrono::{DateTime, Utc};

// https://slurm.schedmd.com/squeue.html
const SQUEUE_FORMAT_STR: &str = "%a|%A|%B|%c|%C|%D|%e|%E|%f|%F|%g|%G|%i|%l|%L|%j|%m|%M|%p|%P|%T|%r|%S|%V|%Z|%o";
const SQUEUE_EXPECTED_COLS: &[&str] = &[
    "ACCOUNT",
    "JOBID",
    "EXEC_HOST",
    "MIN_CPUS",
    "CPUS",
    "NODES",
    "END_TIME",
    "DEPENDENCY",
    "FEATURES",
    "ARRAY_JOB_ID",
    "GROUP",
    "GROUP",
    "JOBID",
    "TIME_LIMIT",
    "TIME_LEFT",
    "NAME",
    "MIN_MEMORY",
    "TIME",
    "PRIORITY",
    "PARTITION",
    "STATE",
    "REASON",
    "START_TIME",
    "SUBMIT_TIME",
    "WORK_DIR",
    "COMMAND",
];
#[tokio::main]
async fn main() -> Result<(), Error> {
    let auth_method = AuthMethod::with_key_file(std::env::var("SSH_KEY_PATH")?, None);
    let client = Client::connect(
        (std::env::var("SSH_HOST")?, 22),
        &std::env::var("SSH_USERNAME")?,
        auth_method,
        ServerCheckMethod::DefaultKnownHostsFile,
    )
    .await?;

    let result = client
        .execute(&format!("squeue --format='{SQUEUE_FORMAT_STR}'"))
        .await?;
    let mut res_lines = result
    .stdout
    .split("\n");
    let column_str = 
        res_lines.next()
        .ok_or(Error::msg("No line breaks in output"))?
        .to_string();
    let columns: Vec<&str> = column_str.split("|").collect();
    println!("{:?}", columns);
    assert_eq!(columns,SQUEUE_EXPECTED_COLS);
    let time: DateTime<Utc> = SystemTime::now().into();
    let d: Vec<HashMap<_,_>> = res_lines.map(|line| columns.iter().zip(line.split("|")).collect()).collect();
    println!("{:#?}",d[0]);
    println!("Got {} results",d.len());
    println!("Time of Querying: {}",time.to_rfc3339());
    Ok(())
}
