use std::{
    collections::{HashMap, HashSet},
    future::IntoFuture,
    path::Path,
    time::{Duration, SystemTime},
};

use anyhow::Error;
use async_ssh2_tokio::client::{AuthKeyboardInteractive, AuthMethod, Client, ServerCheckMethod};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
const SERVER_CHECK_METHOD: ServerCheckMethod = ServerCheckMethod::NoCheck;

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
    let username = std::env::var("SSH_USERNAME")?;
    let host = std::env::var("SSH_HOST")?;
    let cfg: ConnectionConfig<'_, String> = ConnectionConfig::default()
        .with_host((&host, 22))
        .with_username(&username)
        .with_auth(ConnectionAuth::SSHKey {
            path: std::env::var("SSH_KEY_PATH").unwrap_or_default(),
            passphrase: None,
        });

    let client = match login_with_cfg(&cfg).await {
        Ok(client) => Ok::<Client, Error>(client),
        Err(err) => {
            println!("SSH Key Login failed. {err}");

            let password = std::env::var("SSH_PASSWORD")?;
            let mfa = std::env::var("SSH_MFA")?;
            let client = login_with_cfg(&cfg.clone().with_auth(ConnectionAuth::PasswordMFA {
                password: &password,
                mfa_code: &mfa,
            }))
            .await?;
            Ok(client)
        }
    }?;
    let (time, d) = get_squeue_res(&client).await?;
    println!("{:#?}", d[0]);
    println!("Got {} results", d.len());
    println!("Time of Querying: {}", time.to_rfc3339());

    let job_ids: HashSet<String> = d
        .iter()
        .map(|c| c.get("JOBID").cloned().unwrap_or_default())
        .collect();
    println!("Got {} jobs", job_ids.len());
    sleep(Duration::from_secs(20)).await;

    let (time, d) = get_squeue_res(&client).await?;
    let job_ids_2: HashSet<String> = d
        .iter()
        .map(|c| c.get("JOBID").cloned().unwrap_or_default())
        .collect();
    println!("Got {} NEW jobs", job_ids_2.difference(&job_ids).count());

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig<'a, T: AsRef<Path>> {
    pub host: (&'a str, u16),
    pub username: &'a str,
    pub auth: ConnectionAuth<'a, T>,
}
impl<'a, T: AsRef<Path>> ConnectionConfig<'a, T> {
    pub fn default() -> Self {
        ConnectionConfig {
            host: ("", 22),
            username: "",
            auth: ConnectionAuth::PasswordMFA {
                password: "",
                mfa_code: "",
            },
        }
    }
    pub fn new(host: (&'a str, u16), username: &'a str, auth: ConnectionAuth<'a, T>) -> Self {
        ConnectionConfig {
            host,
            username,
            auth,
        }
    }

    pub fn with_auth(mut self, auth: ConnectionAuth<'a, T>) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_username(mut self, username: &'a str) -> Self {
        self.username = username;
        self
    }

    pub fn with_host(mut self, host: (&'a str, u16)) -> Self {
        self.host = host;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum ConnectionAuth<'a, T: AsRef<Path>> {
    #[serde(rename = "password-mfa")]
    PasswordMFA {
        password: &'a str,
        #[serde(rename = "mfaCode")]
        mfa_code: &'a str,
    },
    #[serde(rename = "ssh-key")]
    SSHKey {
        path: T,
        passphrase: Option<&'a str>,
    },
}

impl<'a, T: AsRef<Path>> Into<AuthMethod> for ConnectionAuth<'a, T> {
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
                AuthMethod::with_key_file(path, passphrase)
            }
        }
    }
}

impl<'a, T: AsRef<Path>> Into<AuthMethod> for &ConnectionAuth<'a, T> {
    fn into(self) -> AuthMethod {
        match self {
            ConnectionAuth::PasswordMFA { password, mfa_code } => {
                AuthMethod::with_keyboard_interactive(
                    AuthKeyboardInteractive::new()
                        .with_response("Password", *password)
                        .with_response("Two-factor code", *mfa_code),
                )
            }
            ConnectionAuth::SSHKey { path, passphrase } => {
                AuthMethod::with_key_file(path, *passphrase)
            }
        }
    }
}

async fn login_with_cfg<T: AsRef<Path>>(cfg: &ConnectionConfig<'_, T>) -> Result<Client, Error> {
    let auth_method = (&cfg.auth).into();
    let client = Client::connect(cfg.host, cfg.username, auth_method, SERVER_CHECK_METHOD).await?;
    Ok(client)
}

async fn get_squeue_res<'a>(
    client: &'a Client,
) -> Result<(DateTime<Utc>, Vec<HashMap<String, String>>), Error> {
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
        .map(move |line| {
            columns
                .iter()
                .zip(line.split("|"))
                .map(|(col_name, val)| (col_name.to_string(), val.to_string()))
                .collect()
        })
        .collect();
    Ok((time, d))
}
