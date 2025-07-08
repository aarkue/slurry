use std::{collections::HashSet, path::PathBuf, sync::Arc, time::SystemTime};

use anyhow::{Error, Ok};
use async_ssh2_tokio::Client;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{JobState};

type JobID = String;
type FolderID = String;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Options for creating new SLURM jobs
pub struct JobOptions {
    /// The root directory (i.e., where the job should be started)
    pub root_dir: String,
    /// Files to upload before starting the job (e.g., the binary that should be started or required data files)
    pub files_to_upload: HashSet<JobFilesToUpload>,
    /// How many CPUs to request per task (`--cpus-per-task`)
    pub num_cpus: usize,
    /// How long the job should be executed (`--time`)
    pub time: String,
    /// The bash command to execute
    pub command: String,
    /// Port forwarding configuartion, if local port on HPC node executing the job should be forwarded
    pub local_forwarding: Option<JobLocalForwarding>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// Files to upload before starting a SLURM job
pub struct JobFilesToUpload {
    /// Local path to file
    pub local_path: PathBuf,
    /// Subpath (i.e., in which directory to save the file on the HPC cluster, directories will be recursively created)
    pub remote_subpath: String,
    /// Filename (i.e., how the file should be names on the HPC cluster)
    pub remote_file_name: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// Port forwarding options
/// 
/// Can be used to forward a port of the executing HPC cluster node to the user's local machine.
///
/// Forwarding is done over a relay node directly accessible over SSH (e.g., the login node of the SLURM system)
pub struct JobLocalForwarding {
    /// The port where the forwarding should be available locally
    pub local_port: u16,
    /// The port to use for the relay 
    pub relay_port: u16,
    /// The address of the relay (e.g., hostname)
    pub relay_addr: String,
}
/// Submit a job to SLURM over SSH
pub async fn submit_job(
    client: Arc<Client>,
    job_options: JobOptions,
) -> Result<(FolderID, JobID), Error> {
    // Create job folder
    let folder_id = DateTime::<Utc>::from(SystemTime::now()).to_rfc3339();
    client
        .execute(&format!(
            "mkdir -p '{}/{}'",
            job_options.root_dir, folder_id
        ))
        .await?;

    let mut set = JoinSet::new();
    let root_dir = job_options.root_dir.clone();

    // Upload all files
    job_options
        .files_to_upload
        .into_iter()
        .for_each(|file_to_upload| {
            let root_dir = root_dir.clone();
            let client_arc = Arc::clone(&client);
            let file_to_upload = file_to_upload.clone();
            let folder_id = folder_id.clone();
            set.spawn(async move {
                client_arc
                    .execute(&format!(
                        "mkdir -p '{}/{}/{}'",
                        root_dir, folder_id, file_to_upload.remote_subpath
                    ))
                    .await
                    .unwrap_or_else(|_| {
                        panic!(
                            "Could not create directory for file {}",
                            file_to_upload.remote_subpath
                        )
                    });
                client_arc
                    .upload_file(
                        &file_to_upload.local_path,
                        format!(
                            "{}/{}/{}/{}",
                            root_dir,
                            folder_id,
                            file_to_upload.remote_subpath,
                            file_to_upload.remote_file_name
                        ),
                    )
                    .await
            });
        });
    set.join_all()
        .await
        .into_iter()
        .collect::<Result<(), _>>()?;

    // Create Job Script

    // Add local port forwarding (if necessary)
    let forwaring_str = match job_options.local_forwarding {
        Some(forwarding_options) => format!(
            "ssh -N -f -R {}:localhost:{} {}",
            forwarding_options.relay_port,
            forwarding_options.local_port,
            forwarding_options.relay_addr
        ),
        None => String::default(),
    };
    // Create script on system
    client
        .execute(&format!(
            "cd {}/{} &&
    echo '#!/usr/bin/zsh
### Job Parameters
#SBATCH --ntasks=1
#SBATCH --cpus-per-task={}
#SBATCH --time={}
#SBATCH --job-name={}  # Sets the job name
#SBATCH --output=stdout.txt     # redirects stdout and stderr to stdout.txt

### Program Code
{}
{}' > start.sh && chmod +x start.sh",
            root_dir,
            folder_id,
            job_options.num_cpus,
            job_options.time,
            folder_id,
            forwaring_str,
            job_options.command
        ))
        .await?;

    // Schedule job & get job id
    let sbatch_out = client
        .execute(&format!("cd {}/{} && sbatch start.sh", root_dir, folder_id))
        .await?;
    let job_id = sbatch_out.stdout.split(" ").last();
    if let Some(job_id) = job_id {
        Ok((folder_id.clone(), job_id.to_string()))
    } else {
        Err(Error::msg("No JOB ID returned by sbatch."))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
/// Status of a scheduled SLURM job
pub enum JobStatus {
    /// Job is pending
    PENDING {
        /// Estimated start time of job (if available)
        start_time: Option<NaiveDateTime>,
    },
    /// Job is running
    RUNNING {
        /// Start time of job (if available)
        start_time: Option<NaiveDateTime>,
        /// (Estimated) end time of job (if available)
        end_time: Option<NaiveDateTime>,
    },
    /// Job has ended
    ENDED {
        /// End state of Job
        state: JobState,
    },
    /// Job was not found
    NotFound,
}

/// Get the status of a SLURM job, given its ID and a SSH client
pub async fn get_job_status(client: &Client, job_id: &str) -> Result<JobStatus, Error> {
    let (_time, res) =
        crate::data_extraction::get_squeue_res_ssh(client, &crate::data_extraction::SqueueMode::JOBIDS(vec![job_id.to_string()])).await?;
    if res.is_empty() {
        return Ok(JobStatus::NotFound);
        // return Err(Error::msg("Could not find job."))
    }
    let j = &res[0];
    Ok(match &j.state {
        JobState::PENDING => JobStatus::PENDING {
            start_time: j.start_time,
        },
        JobState::RUNNING => JobStatus::RUNNING {
            start_time: j.start_time,
            end_time: j.end_time,
        },
        c => JobStatus::ENDED { state: c.clone() },
    })
}
