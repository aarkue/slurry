use std::{
    collections::{HashMap, HashSet},
    fs::{create_dir_all, File},
    io::BufWriter,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Error;
use async_ssh2_tokio::client::{AuthKeyboardInteractive, AuthMethod, ServerCheckMethod};
pub use async_ssh2_tokio::Client;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use structdiff::{Difference, StructDiff};
const SERVER_CHECK_METHOD: ServerCheckMethod = ServerCheckMethod::NoCheck;

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
                s => parse_slurm_duration(s).map(|d| Some(d)).unwrap_or_default(),
            }, // 12
            time_left: match vals[13] {
                "INVALID" => None,
                s => parse_slurm_duration(s).map(|d| Some(d)).unwrap_or_default(),
            }, // 13
            name: vals[14].to_string(),       // 14
            min_memory: vals[15].to_string(), // 15
            time: match vals[16] {
                "INVALID" => None,
                s => parse_slurm_duration(s).map(|d| Some(d)).unwrap_or_default(),
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
            s => {
                println!("Unhandled job state: {} detected!", s);
                Ok(Self::OTHER(s.to_string()))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: (String, u16),
    pub username: String,
    pub auth: ConnectionAuth,
}
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

impl Into<AuthMethod> for ConnectionAuth {
    fn into(self) -> AuthMethod {
        match self {
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

impl Into<AuthMethod> for &ConnectionAuth {
    fn into(self) -> AuthMethod {
        match self {
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

pub async fn get_squeue_res<'a>(
    client: &'a Client,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), Error> {
    let result = client
        .execute(&format!(
            "squeue -h -a -M all -t all --format='{SQUEUE_FORMAT_STR}'"
        ))
        .await?;
    let mut res_lines = result.stdout.split("\n");
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

pub async fn squeue_diff<'a, 'b>(
    client: &'a Client,
    path: &Path,
    known_jobs: &'b mut HashMap<String, SqueueRow>,
    all_ids: &'b mut HashSet<String>,
) -> Result<(), Error> {
    let (time, rows) = get_squeue_res(client).await?;
    let cleaned_time = time.to_rfc3339().replace(":", "-");
    let row_ids = rows
        .iter()
        .map(|r| r.job_id.clone())
        .collect::<HashSet<_>>();
    // Sanity check
    if rows.len() != row_ids.len() {
        eprintln!("Count mismatch: {} != {}",rows.len(),row_ids.len());

    }
    create_dir_all(&path)?;
    let id_save_path = path.join(format!("{}.json", cleaned_time));
    if let Err(e) = serde_json::to_writer(
        BufWriter::new(File::create(id_save_path).unwrap()),
        &row_ids,
    ) {
        eprintln!("Failed to create file for all jobs ids: {:?}", e);
    }
    for row in rows {
        if let Some(prev_row) = known_jobs.get_mut(&row.job_id) {
            // Job is known!
            // Compute delta
            let diff = prev_row.diff(&row);
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
            *prev_row = row;
        } else {
            // Job is new!
            // Double check with all_ids:
            if all_ids.contains(&row.job_id) {
                eprintln!("Job re-appeared! Maybe IDs get reused?");
            }
            let folder_path = path.join(&row.job_id);
            create_dir_all(&folder_path)?;
            // Save job (e.g., as JSON)
            let save_path = folder_path.join(format!("{}.json", cleaned_time));
            if let Err(e) =
                serde_json::to_writer(BufWriter::new(File::create(save_path).unwrap()), &row)
            {
                eprintln!("Failed to create file for {}: {:?}", row.job_id, e);
            }
            known_jobs.insert(row.job_id.clone(), row);
        }
    }
    // Remove all known jobs which
    known_jobs.retain(|j_id, _| row_ids.contains(j_id));
    all_ids.extend(row_ids);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs::File,
        io::BufReader,
        path::{Path, PathBuf},
    };

    use anyhow::Error;
    use async_ssh2_tokio::AuthMethod;
    use chrono::Duration;
    use glob::glob;

    use crate::{
        login_with_cfg, squeue_diff, ConnectionAuth, ConnectionConfig, JobState, SqueueRow,
    };

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
        let path = PathBuf::new().join("test_squeue_loop");
        let mut i = 0;
        loop {
            squeue_diff(&client, &path, &mut known_jobs, &mut all_ids)
            .await
            .unwrap();
        i += 1;
        println!("Ran for {} iterations, sleeping...",i);
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
    }

    // #[test]
    // fn test_json() -> Result<(), Error> {
    //     let json_str = include_str!("../test_data/2024-10-12T11:24:16.744882594+00:00.json");
    //     let data: Vec<SqueueRow> = serde_json::from_str(&json_str)?;
    //     for d in &data {
    //         if d.step_job_id.1.is_some() {
    //             println!("{:?} != {:?}", d.job_id, d.step_job_id);
    //         }
    //     }
    //     Ok(())
    // }

    // #[test]
    // fn test_states() -> Result<(), Error> {
    //     let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    //         .join("test_data")
    //         .join("*.json");

    //     for entry in glob(path.to_str().unwrap()).unwrap() {
    //         match entry {
    //             Ok(path) => {
    //                 println!("{:?}", path);
    //                 let buf_reader = BufReader::new(File::open(path)?);
    //                 let data: Vec<SqueueRow> = serde_json::from_reader(buf_reader)?;
    //                 let states: HashSet<JobState> =
    //                     data.iter().enumerate().map(|(_i, d)| d.state.clone()).collect();
    //                 println!("{:?}", states);
    //             }
    //             Err(err) => eprintln!("Err {:?}", err),
    //         }
    //     }
    //     Ok(())
    // }

    // #[test]
    // fn test_with_json_files() {
    //     let json1 = include_str!("../test_data/2024-10-12T11:24:16.744882594+00:00.json");
    //     let json2 = include_str!("../test_data/2024-10-12T11:25:44.866855169+00:00.json");

    //     let data1: Vec<SqueueRow> = serde_json::from_str(&json1).unwrap();
    //     let data2: Vec<SqueueRow> = serde_json::from_str(&json2).unwrap();

    //     let job_map1: HashMap<String, SqueueRow> =
    //         data1.into_iter().map(|d| (d.job_id.clone(), d)).collect();

    //     let job_map2: HashMap<String, SqueueRow> =
    //         data2.into_iter().map(|d| (d.job_id.clone(), d)).collect();

    //     println!("|jobs1| = {}", job_map1.len());
    //     println!("|jobs2| = {}", job_map2.len());

    //     let x: HashSet<&JobState> = job_map1.iter().map(|(_, j)| &j.state).collect();
    //     println!("States 1: {:?}", x);

    //     let y: HashSet<&JobState> = job_map2.iter().map(|(_, j)| &j.state).collect();
    //     println!("States 2: {:?}", y);

    //     for (job1_id, job1) in &job_map1 {
    //         if job1.exec_host.is_some() && job1.state != JobState::RUNNING {
    //             println!("Not running but has exec host: {:?}", job1);
    //         }
    //         if job1.exec_host.is_none() && job1.state == JobState::RUNNING {
    //             println!("Running but has no exec host: {:?}", job1);
    //         }
    //         if let Some(job2) = job_map2.get(job1_id) {
    //             if job2.state != job1.state {
    //                 println!(
    //                     "Job {} State change: {:?} -> {:?}",
    //                     job1.job_id, job1.state, job2.state
    //                 );
    //             } else {
    //                 // println!(
    //                 //     "Job {} State same: {} -> {}",
    //                 //     job1.id, job1.state, job2.state
    //                 // );
    //             }
    //         } else {
    //             println!("Job {:?} not found in second set", job1);
    //         }
    //     }
    //     println!("All jobs finished!");
    // }
}
