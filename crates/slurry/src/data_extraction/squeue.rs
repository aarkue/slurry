use std::{path::PathBuf, time::Duration};

use anyhow::Error;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use structdiff::{Difference, StructDiff};

use crate::{parse_slurm_duration, JobState};
use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, File},
    future::Future,
    io::BufWriter,
    path::Path,
    process::Command,
    time::{Instant, SystemTime},
};

#[cfg(feature = "ssh")]
use async_ssh2_tokio::Client;
use chrono::{DateTime, Utc};
use rayon::iter::IntoParallelRefIterator;

// https://slurm.schedmd.com/squeue.html
pub(crate) const SQUEUE_FORMAT_STR: &str =
    "%a|%A|%B|%c|%C|%D|%e|%E|%f|%F|%G|%i|%l|%L|%j|%m|%M|%p|%P|%T|%r|%S|%V|%Z|%o";
// const SQUEUE_EXPECTED_COLS: &[&str] = &[
//     "ACCOUNT",
//     "JOBID",
//     "EXEC_HOST",
//     "MIN_CPUS",
//     "CPUS",
//     "NODES",
//     "END_TIME",
//     "DEPENDENCY",
//     "FEATURES",
//     "ARRAY_JOB_ID",
//     "GROUP",
//     "STEPJOBID",
//     "TIME_LIMIT",
//     "TIME_LEFT",
//     "NAME",
//     "MIN_MEMORY",
//     "TIME",
//     "PRIORITY",
//     "PARTITION",
//     "STATE",
//     "REASON",
//     "START_TIME",
//     "SUBMIT_TIME",
//     "WORK_DIR",
//     "COMMAND",
// ];

#[derive(Debug, Clone, Serialize, Deserialize, Difference)]
/// Struct for parsed output row of `squeue` command
///
/// Containg information about a scheduled, running, and completed SLURM job
pub struct SqueueRow {
    /// "ACCOUNT",
    pub account: String,
    /// "JOBID",
    pub job_id: String,
    /// "`EXEC_HOST`",
    pub exec_host: Option<String>,
    /// "`MIN_CPUS`",
    pub min_cpus: usize,
    /// "CPUS",
    pub cpus: usize,
    /// "NODES",
    pub nodes: usize,
    /// "`END_TIME`",
    pub end_time: Option<NaiveDateTime>,
    /// "DEPENDENCY",
    pub dependency: Option<String>,
    /// "FEATURES",
    pub features: String,
    /// "`ARRAY_JOB_ID`",
    pub array_job_id: String,
    /// "GROUP",
    pub group: String,
    /// "STEPJOBID",
    /// 49848561 or `49869434_2` or 49616001_[3-10%1]
    pub step_job_id: (String, Option<String>),
    /// "`TIME_LIMIT`",
    pub time_limit: Option<Duration>,
    /// "`TIME_LEFT`",
    #[difference(skip)]
    pub time_left: Option<Duration>,
    /// "NAME",
    pub name: String,
    /// "`MIN_MEMORY`",
    pub min_memory: String,
    /// "TIME",
    #[difference(skip)]
    pub time: Option<Duration>,
    /// "PRIORITY",
    pub priority: f64,
    /// "PARTITION",
    pub partition: String,
    /// "STATE",
    pub state: JobState,
    /// "REASON",
    pub reason: String,
    /// "`START_TIME`",
    pub start_time: Option<NaiveDateTime>,
    /// "`SUBMIT_TIME`",
    pub submit_time: NaiveDateTime,
    /// "`WORK_DIR`",
    pub work_dir: PathBuf,
    /// "COMMAND",
    pub command: String,
}

impl SqueueRow {
    fn parse_from_strs(vals: &[&str]) -> Result<Self, Error> {
        if vals.len() != 25 {
            return Err(Error::msg("Invalid length of values."));
        }
        let mut step_job_id = vals[11].split("_");
        Ok(Self {
            account: vals[0].to_string(),
            job_id: vals[1].to_string(),
            exec_host: match vals[2] {
                "n/a" => None,
                s => Some(s.to_string()),
            },
            min_cpus: vals[3].parse()?,
            cpus: vals[4].parse()?,
            nodes: vals[5].parse()?,
            end_time: match vals[6] {
                "N/A" => None,
                s => Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")?),
            },
            dependency: match vals[7] {
                "(null)" => None,
                s => Some(s.to_string()),
            },
            features: vals[8].to_string(),
            array_job_id: vals[9].to_string(),
            group: vals[10].to_string(),
            step_job_id: (
                step_job_id.next().unwrap().to_string(),
                step_job_id.next().map(|s| s.to_string()),
            ), // todo!(), // 11
            time_limit: match vals[12] {
                "INVALID" => None,
                s => parse_slurm_duration(s).map(Some).unwrap_or_default(),
            }, // 12
            time_left: match vals[13] {
                "INVALID" => None,
                s => parse_slurm_duration(s).map(Some).unwrap_or_default(),
            }, // 13
            name: vals[14].to_string(),       // 14
            min_memory: vals[15].to_string(), // 15
            time: match vals[16] {
                "INVALID" => None,
                s => parse_slurm_duration(s).map(Some).unwrap_or_default(),
            },
            priority: vals[17]
                .parse()
                .inspect_err(|err| eprintln!("Priority failed to parse! {err:?}"))?, // 17
            partition: vals[18].to_string(),
            state: vals[19].parse()?,
            reason: vals[20].to_string(),
            start_time: match vals[21] {
                "N/A" => None,
                s => Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")?),
            },
            submit_time: NaiveDateTime::parse_from_str(vals[22], "%Y-%m-%dT%H:%M:%S")?,
            work_dir: vals[23].parse()?,
            command: vals[24].to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Parameter for `squeue` extraction, specifying what SLURM jobs to include
pub enum SqueueMode {
    #[default]
    /// Include all SLURM jobs
    ALL,
    /// Include only SLURM jobs of active user
    MINE,
    /// Include only the specified SLURM jobs (given by their IDs)
    JOBIDS(Vec<String>),
}
/// Get squeue results using the provided `execute_cmd` function
pub async fn get_squeue_res<F, Fut>(
    mode: &SqueueMode,
    execute_cmd: F,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<String, Error>>,
{
    let extra_arg = match mode {
        SqueueMode::ALL => String::default(),
        SqueueMode::MINE => String::from("--me"),
        SqueueMode::JOBIDS(vec) => format!("-j {}", vec.join(",")),
    };
    let result = execute_cmd(format!(
        "squeue -h -a -M all -t all --format='{SQUEUE_FORMAT_STR}' {extra_arg}"
    ))
    .await?;
    let res_lines = result.split("\n");

    // For checking columns:
    // let _column_str = res_lines
    //     .next()
    //     .ok_or(Error::msg("No line breaks in output"))?
    //     .to_string();

    // let columns: Vec<&str> = _column_str.split("|").collect();
    // if columns != SQUEUE_EXPECTED_COLS {
    //     eprintln!("Warning! Columns are not identical!");
    //     eprintln!("{:?} != {:?}", columns, SQUEUE_EXPECTED_COLS);
    // }

    let time: DateTime<Utc> = SystemTime::now().into();
    let d: Vec<SqueueRow> = res_lines
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }
            let res = SqueueRow::parse_from_strs(&line.split("|").collect::<Vec<_>>());
            match res {
                Ok(row) => Some(row),
                Err(err) => {
                    println!("[!] {:?} for {:?}", err, &line);
                    None
                }
            }
        })
        .collect();
    Ok((time, d))
}

/// Run and parse `squeue` result locally (i.e., not via SSH)
pub async fn get_squeue_res_locally(
    mode: &SqueueMode,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error> {
    get_squeue_res(mode, |cmd_s| async move {
        // let splits: Vec<&str> = cmd.split(" ").collect();
        // println!("{:#?}",splits);
        // cmd.args(splits.iter().skip(1));
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&cmd_s);
        let d = Instant::now();
        let out = cmd.output()?;
        let s = String::from_utf8(out.stdout)?;
        // println!("{:?}",out);
        println!("Running squeue took {:?}", d.elapsed());
        Ok(s)
    })
    .await
}

#[cfg(feature = "ssh")]
/// Run and parse `squeue` result over SSH
pub async fn get_squeue_res_ssh(
    client: &Client,
    mode: &SqueueMode,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error> {
    get_squeue_res(mode, |cmd| async move {
        let r = client.execute(&cmd).await?;
        Ok(r.stdout)
    })
    .await
}
use rayon::prelude::*;

/// Execute `squeue` and compare the output with (optional) data from previous executions
pub async fn squeue_diff<'b, F, Fut>(
    get_squeue: F,
    path: &Path,
    known_jobs: &'b mut HashMap<String, SqueueRow>,
    all_ids: &'b mut HashSet<String>,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(DateTime<Utc>, Vec<SqueueRow>), Error>>,
{
    let (time, rows) = get_squeue().await?;
    // let (time, rows) = get_squeue_res(client).await?;
    let cleaned_time = time.to_rfc3339().replace(":", "_");
    let row_ids = rows
        .iter()
        .map(|r| r.job_id.clone())
        .collect::<HashSet<_>>();
    // Sanity check
    if rows.len() != row_ids.len() {
        eprintln!("Count mismatch: {} != {}", rows.len(), row_ids.len());
    }
    create_dir_all(path)?;
    let id_save_path = path.join(format!("{cleaned_time}.json"));
    if let Err(e) = serde_json::to_writer(
        BufWriter::new(File::create(id_save_path).unwrap()),
        &row_ids,
    ) {
        eprintln!("Failed to create file for all jobs ids: {e:?}");
    }
    *known_jobs = rows
        .par_iter()
        .map(|row| {
            if let Some(prev_row) = known_jobs.get(&row.job_id) {
                // Job is known!
                // Compute delta
                let diff = prev_row.diff(row);
                if !diff.is_empty() {
                    // Save job delta (e.g., as JSON)
                    let save_path = path
                        .join(&row.job_id)
                        .join(format!("DELTA-{cleaned_time}.json"));
                    if let Err(e) = serde_json::to_writer(
                        BufWriter::new(File::create(save_path).unwrap()),
                        &diff,
                    ) {
                        eprintln!("Failed to create file for {}: {:?}", row.job_id, e);
                    }
                }
                // Update prev_row in known_jobs
                (row.job_id.clone(), row.clone())
                // rw.write().unwrap().insert(row.job_id.clone(), row.clone());
                // *prev_row = row.clone();
            } else {
                // Job is new!
                // Double check with all_ids:
                if all_ids.contains(&row.job_id) {
                    eprintln!("Job re-appeared! Maybe IDs get reused?");
                }
                let folder_path = path.join(&row.job_id);
                create_dir_all(&folder_path).unwrap();
                // Save job (e.g., as JSON)
                let save_path = folder_path.join(format!("{cleaned_time}.json"));
                if let Err(e) =
                    serde_json::to_writer(BufWriter::new(File::create(save_path).unwrap()), &row)
                {
                    eprintln!("Failed to create file for {}: {:?}", row.job_id, e);
                }
                // rw.write().unwrap().insert(row.job_id.clone(), row.clone());
                (row.job_id.clone(), row.clone())
            }
        })
        .collect();
    // let known_jobs = rw.into_inner().unwrap();
    // Remove all known jobs which
    // known_jobs.retain(|j_id, _| row_ids.contains(j_id));
    all_ids.extend(row_ids);
    Ok((time, rows))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };


    use crate::data_extraction::{get_squeue_res_locally, SqueueMode};
    #[cfg(feature = "ssh")]
    use crate::login_with_cfg;

    #[cfg(feature = "ssh")]
    #[tokio::test]
    async fn test_squeue_loop() {
        let login_cfg = crate::misc::get_config_from_env();
        let client = login_with_cfg(&login_cfg).await.unwrap();
        let mut known_jobs = HashMap::default();
        let mut all_ids = HashSet::default();
        let path = PathBuf::new().join("test_squeue_loop-14-01-2025");
        let mut i = 0;
        loop {
            use crate::data_extraction::{get_squeue_res_ssh, squeue_diff};

            squeue_diff(
                || get_squeue_res_ssh(&client, &SqueueMode::ALL),
                &path,
                &mut known_jobs,
                &mut all_ids,
            )
            .await
            .unwrap();
            i += 1;
            println!("Ran for {i} iterations, sleeping...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    #[tokio::test]
    async fn test_local() {
        let res = get_squeue_res_locally(&SqueueMode::ALL).await.unwrap();
        println!("Got {} results", res.1.len())
    }
}
