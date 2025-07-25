#![warn(
    clippy::doc_markdown,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility,
    missing_docs
)]
#![doc = include_str!("../README.md")]

use std::{str::FromStr, time::Duration};

use anyhow::Error;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssh")]
use async_ssh2_tokio::client::{AuthKeyboardInteractive, AuthMethod, ServerCheckMethod};
#[cfg(feature = "ssh")]
const SERVER_CHECK_METHOD: ServerCheckMethod = ServerCheckMethod::NoCheck;
#[cfg(feature = "ssh")]
pub use async_ssh2_tokio::Client;

#[cfg(feature = "ssh")]
/// Module for managing (e.g., creating or cancelling) SLURM jobs
pub mod job_management;

/// Module for extracting data from SLURM systems
/// e.g., about currently running jobs
pub mod data_extraction;

/// Module for miscellaneous features
///
/// e.g., SSH port forwarding
pub mod misc;

#[cfg(feature = "ssh")]
#[doc(inline)]
pub use misc::port_forwarding::ssh_port_forwarding;

#[cfg(feature = "ssh")]
#[doc(inline)]
pub use job_management::submit_job;

#[doc(inline)]
pub use data_extraction::get_squeue_res_locally;

#[cfg(feature = "ssh")]
#[doc(inline)]
pub use data_extraction::get_squeue_res_ssh;

#[doc(inline)]
pub use data_extraction::squeue_diff;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// State of a SLURM job (according to `squeue`)
///
/// Documentation taken from <https://slurm.schedmd.com/squeue.html#SECTION_JOB-STATE-CODES>.
pub enum JobState {
    /// Job currently has an allocation.
    RUNNING,
    /// Job is awaiting resource allocation.
    PENDING,
    /// Job is in the process of completing. Some processes on some nodes may still be active.
    COMPLETING,
    /// Job has terminated all processes on all nodes with an exit code of zero.
    COMPLETED,
    /// Job was explicitly cancelled by the user or system administrator. The job may or may not have been initiated.
    CANCELLED,
    /// Job terminated with non-zero exit code or other failure condition.
    FAILED,
    /// Job terminated upon reaching its time limit.
    TIMEOUT,
    /// Job experienced out of memory error.
    #[allow(non_camel_case_types)]
    OUT_OF_MEMORY,
    /// Job terminated due to failure of one or more allocated nodes.
    #[allow(non_camel_case_types)]
    NODE_FAIL,
    /// Other Job state, specifying the concrete job state as a [`String`]
    OTHER(String),
}
impl FromStr for JobState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
                println!("Unhandled job state: {s} detected!");
                Ok(Self::OTHER(s.to_string()))
            }
        }
    }
}

#[cfg(feature = "ssh")]
#[derive(Debug, Clone, Serialize, Deserialize)]
/// A connection config for logging in using SSH
pub struct ConnectionConfig {
    /// The host (hostname and port) to connect to
    pub host: (String, u16),
    /// The username to use for connecting
    pub username: String,
    /// The authentication configuration
    pub auth: ConnectionAuth,
}

#[cfg(feature = "ssh")]
impl Default for ConnectionConfig {
    fn default() -> Self {
        ConnectionConfig {
            host: (String::new(), 22),
            username: String::new(),
            auth: ConnectionAuth::PasswordMFA {
                password: String::new(),
                mfa_code: String::new(),
            },
        }
    }
}
#[cfg(feature = "ssh")]
impl ConnectionConfig {
    /// Create a new connection configuration using the given parameters
    pub fn new(host: (String, u16), username: String, auth: ConnectionAuth) -> Self {
        ConnectionConfig {
            host,
            username,
            auth,
        }
    }
    /// Assign the passed authentication settings to the connection config
    pub fn with_auth(mut self, auth: ConnectionAuth) -> Self {
        self.auth = auth;
        self
    }

    /// Assign the passed username to the connection config
    pub fn with_username(mut self, username: String) -> Self {
        self.username = username;
        self
    }

    /// Assign the passed host to the connection config
    pub fn with_host(mut self, host: (String, u16)) -> Self {
        self.host = host;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
#[cfg(feature = "ssh")]
/// Authentication Settings for a SHH Connection ([`ConnectionConfig`])
pub enum ConnectionAuth {
    #[serde(rename = "password-mfa")]
    /// Login via password and multi-factor-authentication token (MFA)
    PasswordMFA {
        /// Password
        password: String,
        #[serde(rename = "mfaCode")]
        /// Multi-Factor-Authentication (MFA) token
        mfa_code: String,
    },
    #[serde(rename = "ssh-key")]
    /// Login via an SSH key
    SSHKey {
        /// Path to where the SSH key is stored
        path: String,
        /// Optional passphrase for the SSH key
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
/// Login via SSH using the specified configuration
pub async fn login_with_cfg(cfg: &ConnectionConfig) -> Result<Client, Error> {
    let auth_method = (&cfg.auth).into();
    let client = Client::connect_with_config(
        cfg.host.clone(),
        &cfg.username,
        auth_method,
        SERVER_CHECK_METHOD,
        async_ssh2_tokio::Config {
            ..Default::default()
        },
    )
    .await?;
    Ok(client)
}
