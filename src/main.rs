use std::{collections::{HashMap, HashSet}, time::{Duration, SystemTime}};

use anyhow::Error;
use async_ssh2_tokio::client::{AuthMethod, Client, ServerCheckMethod};
use chrono::{DateTime, Utc};
use tokio::time::sleep;

// https://slurm.schedmd.com/squeue.html
const SQUEUE_FORMAT_STR: &str =
    "%a|%A|%B|%c|%C|%D|%e|%E|%f|%F|%g|%G|%i|%l|%L|%j|%m|%M|%p|%P|%T|%r|%S|%V|%Z|%o";
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
    let (time,d) = get_squeue_res(&client).await?;
    println!("{:#?}", d[0]);
    println!("Got {} results", d.len());
    println!("Time of Querying: {}", time.to_rfc3339());

    let job_ids: HashSet<String> = d.iter().map(|c| c.get("JOBID").cloned().unwrap_or_default()).collect();
    println!("Got {} jobs",job_ids.len());
    sleep(Duration::from_secs(20)).await;


    let (time,d) = get_squeue_res(&client).await?;
    let job_ids_2: HashSet<String> = d.iter().map(|c| c.get("JOBID").cloned().unwrap_or_default()).collect();
    println!("Got {} NEW jobs",job_ids_2.difference(&job_ids).count());

    Ok(())
}

async fn get_squeue_res<'a>(client: &'a Client) -> Result<(DateTime<Utc>, Vec<HashMap<String,String>>), Error> {
    let result = client
        .execute(&format!("squeue --format='{SQUEUE_FORMAT_STR}'"))
        .await?;
    let mut res_lines = result.stdout.split("\n");
    let column_str = res_lines
        .next()
        .ok_or(Error::msg("No line breaks in output"))?
        .to_string();
    let columns: Vec<&str> = column_str.split("|").collect();
    assert_eq!(columns, SQUEUE_EXPECTED_COLS);
    let time: DateTime<Utc> = SystemTime::now().into();
    let d: Vec<HashMap<_, _>> = res_lines
        .map(move |line| columns.iter().zip(line.split("|")).map(|(col_name,val)| (col_name.to_string(),val.to_string())).collect())
        .collect();
    Ok((time, d))
}
