use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, File},
    future::Future,
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime},
};

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use rayon::iter::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use structdiff::{Difference, StructDiff};

#[cfg(feature = "ssh")]
use async_ssh2_tokio::client::{AuthKeyboardInteractive, AuthMethod, ServerCheckMethod};
#[cfg(feature = "ssh")]
const SERVER_CHECK_METHOD: ServerCheckMethod = ServerCheckMethod::NoCheck;
#[cfg(feature = "ssh")]
pub use async_ssh2_tokio::Client;

// https://slurm.schedmd.com/squeue.html
const SQUEUE_FORMAT_STR: &str =
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
pub struct SqueueRow {
    // "ACCOUNT",
    pub account: String,
    // "JOBID",
    pub job_id: String,
    // "EXEC_HOST",
    pub exec_host: Option<String>,
    // "MIN_CPUS",
    pub min_cpus: usize,
    // "CPUS",
    pub cpus: usize,
    // "NODES",
    pub nodes: usize,
    // "END_TIME",
    pub end_time: Option<NaiveDateTime>,
    // "DEPENDENCY",
    pub dependency: Option<String>,
    // "FEATURES",
    pub features: String,
    // "ARRAY_JOB_ID",
    pub array_job_id: String,
    // "GROUP",
    pub group: String,
    // "STEPJOBID",
    // 49848561 or 49869434_2 or 49616001_[3-10%1]
    pub step_job_id: (String, Option<String>),
    // "TIME_LIMIT",
    pub time_limit: Option<Duration>,
    // "TIME_LEFT",
    #[difference(skip)]
    pub time_left: Option<Duration>,
    // "NAME",
    pub name: String,
    // "MIN_MEMORY",
    pub min_memory: String,
    // "TIME",
    #[difference(skip)]
    pub time: Option<Duration>,
    // "PRIORITY",
    pub priority: f64,
    // "PARTITION",
    pub partition: String,
    // "STATE",
    pub state: JobState,
    // "REASON",
    pub reason: String,
    // "START_TIME",
    pub start_time: Option<NaiveDateTime>,
    // "SUBMIT_TIME",
    pub submit_time: NaiveDateTime,
    // "WORK_DIR",
    pub work_dir: PathBuf,
    // "COMMAND",
    pub command: String,
}

// days-hours:minutes:seconds
fn parse_slurm_duration(s: &str) -> Result<Duration, Error> {
    let mut dur = Duration::default();

    let v: Vec<_> = s.split("-").collect();
    let mut hms_part = v[0];
    let has_days_part: bool = v.len() > 1;
    if has_days_part {
        // days part exists
        let days: u64 = v[0].parse()?;
        dur += Duration::from_secs(days * 60 * 60 * 24);
        hms_part = v[1];
    }
    let hms = hms_part.split(":").collect::<Vec<_>>();

    if hms.len() == 3 {
        let hours: u64 = hms[0].parse()?;
        let mins: u64 = hms[1].parse()?;
        let secs: u64 = hms[1].parse()?;
        dur += Duration::from_secs(secs + 60 * mins + 60 * 60 * hours);
    } else if hms.len() == 2 {
        let mins: u64 = hms[0].parse()?;
        let secs: u64 = hms[1].parse()?;
        dur += Duration::from_secs(secs + 60 * mins);
    } else if hms.len() == 1 {
        if has_days_part {
            // then: hours
            let hours: u64 = hms[0].parse()?;
            dur += Duration::from_secs(60 * 60 * hours);
        } else {
            // otherwise: minutes
            let mins: u64 = hms[0].parse()?;
            dur += Duration::from_secs(60 * mins);
        }
    } else {
        println!("Parse Error: Got {} splits for duration {}.", hms.len(), s);
        return Err(Error::msg("Invalid duration format."));
    }

    Ok(dur)
}

impl SqueueRow {
    pub fn parse_from_strs(vals: &[&str]) -> Result<Self, Error> {
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
            state: JobState::from_str(vals[19])?,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum JobState {
    RUNNING,
    PENDING,
    COMPLETING,
    COMPLETED,
    CANCELLED,
    FAILED,
    TIMEOUT,
    #[allow(non_camel_case_types)]
    OUT_OF_MEMORY,
    #[allow(non_camel_case_types)]
    NODE_FAIL,
    OTHER(String),
}

impl JobState {
    pub fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "RUNNING" => Ok(Self::RUNNING),
            "PENDING" => Ok(Self::PENDING),
            "COMPLETING" => Ok(Self::COMPLETING),
            "COMPLETED" => Ok(Self::COMPLETED),
            "CANCELLED" => Ok(Self::CANCELLED),
            "FAILED" => Ok(Self::FAILED),
            "TIMEOUT" => Ok(Self::TIMEOUT),
            "OUT_OF_MEMORY" => Ok(Self::OUT_OF_MEMORY),
            "NODE_FAIL" => Ok(Self::NODE_FAIL),
            s => {
                println!("Unhandled job state: {} detected!", s);
                Ok(Self::OTHER(s.to_string()))
            }
        }
    }
}

#[cfg(feature = "ssh")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: (String, u16),
    pub username: String,
    pub auth: ConnectionAuth,
}

#[cfg(feature = "ssh")]
impl ConnectionConfig {
    pub fn default() -> Self {
        ConnectionConfig {
            host: (String::new(), 22),
            username: String::new(),
            auth: ConnectionAuth::PasswordMFA {
                password: String::new(),
                mfa_code: String::new(),
            },
        }
    }
    pub fn new(host: (String, u16), username: String, auth: ConnectionAuth) -> Self {
        ConnectionConfig {
            host,
            username,
            auth,
        }
    }

    pub fn with_auth(mut self, auth: ConnectionAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_username(mut self, username: String) -> Self {
        self.username = username;
        self
    }

    pub fn with_host(mut self, host: (String, u16)) -> Self {
        self.host = host;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
#[cfg(feature = "ssh")]
pub enum ConnectionAuth {
    #[serde(rename = "password-mfa")]
    PasswordMFA {
        password: String,
        #[serde(rename = "mfaCode")]
        mfa_code: String,
    },
    #[serde(rename = "ssh-key")]
    SSHKey {
        path: String,
        passphrase: Option<String>,
    },
}

#[cfg(feature = "ssh")]
impl From<ConnectionAuth> for AuthMethod {
    fn from(val: ConnectionAuth) -> Self {
        match val {
            ConnectionAuth::PasswordMFA { password, mfa_code } => {
                AuthMethod::with_keyboard_interactive(
                    AuthKeyboardInteractive::new()
                        .with_response("Password", password)
                        .with_response("Two-factor code", mfa_code),
                )
            }
            ConnectionAuth::SSHKey { path, passphrase } => {
                AuthMethod::with_key_file(path, passphrase.as_deref())
            }
        }
    }
}

#[cfg(feature = "ssh")]
impl From<&ConnectionAuth> for AuthMethod {
    fn from(val: &ConnectionAuth) -> Self {
        match val {
            ConnectionAuth::PasswordMFA { password, mfa_code } => {
                AuthMethod::with_keyboard_interactive(
                    AuthKeyboardInteractive::new()
                        .with_response("Password", password.clone())
                        .with_response("Two-factor code", mfa_code.clone()),
                )
            }
            ConnectionAuth::SSHKey { path, passphrase } => {
                AuthMethod::with_key_file(path, passphrase.as_deref())
            }
        }
    }
}

#[cfg(feature = "ssh")]
pub async fn login_with_cfg(cfg: &ConnectionConfig) -> Result<Client, Error> {
    let auth_method = (&cfg.auth).into();
    let client = Client::connect(
        cfg.host.clone(),
        &cfg.username,
        auth_method,
        SERVER_CHECK_METHOD,
    )
    .await?;
    Ok(client)
}
use std::io::Write;
pub async fn get_squeue_res<F, Fut>(
    execute_cmd: F,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<String, Error>>,
{
    let result = execute_cmd(format!(
        "squeue -h -a -M all -t all --format='{SQUEUE_FORMAT_STR}'"
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
        .filter_map(move |line| {
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

pub async fn get_squeue_res_locally<'a>() -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error> {
    get_squeue_res(|cmd_s| async move {
        // let splits: Vec<&str> = cmd.split(" ").collect();
        // println!("{:#?}",splits);
        // cmd.args(splits.iter().skip(1));
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(format!("{cmd_s}"));
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
pub async fn get_squeue_res_ssh<'a>(
    client: &'a Client,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error> {
    get_squeue_res(|cmd| async move {
        let r = client.execute(&cmd).await?;
        Ok(r.stdout)
    })
    .await
}
use rayon::prelude::*;

pub async fn squeue_diff<'a, 'b, F, Fut>(
    get_squeue: F,
    // client: &'a Client,
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
    let id_save_path = path.join(format!("{}.json", cleaned_time));
    if let Err(e) = serde_json::to_writer(
        BufWriter::new(File::create(id_save_path).unwrap()),
        &row_ids,
    ) {
        eprintln!("Failed to create file for all jobs ids: {:?}", e);
    }
    *known_jobs = rows.par_iter().map(|row| {
        if let Some(prev_row) = known_jobs.get(&row.job_id) {
            // Job is known!
            // Compute delta
            let diff = prev_row.diff(row);
            if !diff.is_empty() {
                // Save job delta (e.g., as JSON)
                let save_path = path
                    .join(&row.job_id)
                    .join(format!("DELTA-{}.json", cleaned_time));
                if let Err(e) =
                    serde_json::to_writer(BufWriter::new(File::create(save_path).unwrap()), &diff)
                {
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
            let save_path = folder_path.join(format!("{}.json", cleaned_time));
            if let Err(e) =
                serde_json::to_writer(BufWriter::new(File::create(save_path).unwrap()), &row)
            {
                eprintln!("Failed to create file for {}: {:?}", row.job_id, e);
            }
            // rw.write().unwrap().insert(row.job_id.clone(), row.clone());
            (row.job_id.clone(), row.clone())
        }
    }).collect();
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

    use crate::get_squeue_res_locally;
    #[cfg(feature = "ssh")]
    use crate::{login_with_cfg, squeue_diff, ConnectionAuth, ConnectionConfig};

    #[cfg(feature = "ssh")]
    #[tokio::test]
    async fn test_squeue_loop() {
        let login_cfg = ConnectionConfig::new(
            ("login23-1.hpc.itc.rwth-aachen.de".to_string(), 22),
            "at325350".to_string(),
            ConnectionAuth::SSHKey {
                path: "/home/aarkue/.ssh/id_ed25519".to_string(),
                passphrase: None,
            },
        );
        let client = login_with_cfg(&login_cfg).await.unwrap();
        let mut known_jobs = HashMap::default();
        let mut all_ids = HashSet::default();
        let path = PathBuf::new().join("test_squeue_loop-14-01-2025");
        let mut i = 0;
        loop {
            squeue_diff(
                || crate::get_squeue_res_ssh(&client),
                &path,
                &mut known_jobs,
                &mut all_ids,
            )
            .await
            .unwrap();
            i += 1;
            println!("Ran for {} iterations, sleeping...", i);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    #[tokio::test]
    async fn test_local() {
        let res = get_squeue_res_locally().await.unwrap();
        println!("Got {} results", res.1.len())
    }
}
