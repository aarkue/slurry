use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use anyhow::{Error, Ok};
use async_ssh2_tokio::Client;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::{join, sync::futures, task::JoinSet};

use crate::{get_squeue_res_ssh, JobState};

type JobID = String;
type FolderID = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOptions {
    pub root_dir: String,
    pub files_to_upload: HashSet<JobFilesToUpload>,
    pub num_cpus: usize,
    pub time: String,
    pub command: String,
    pub local_forwarding: Option<JobLocalForwarding>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct JobFilesToUpload {
    pub local_path: PathBuf,
    pub remote_subpath: String,
    pub remote_file_name: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct JobLocalForwarding {
    pub local_port: u16,
    pub relay_port: u16,
    pub relay_addr: String,
}
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
                    .expect(&format!(
                        "Could not create directory for file {}",
                        file_to_upload.remote_subpath
                    ));
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
        return Ok((folder_id.clone(), job_id.to_string()));
    } else {
        return Err(Error::msg("No JOB ID returned by sbatch."));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum JobStatus {
    PENDING {
        start_time: Option<NaiveDateTime>,
    },
    RUNNING {
        start_time: Option<NaiveDateTime>,
        end_time: Option<NaiveDateTime>,
    },
    ENDED {
        state: JobState,
    },
    NotFound,
}

pub async fn get_job_status(client: &Client, job_id: &str) -> Result<JobStatus, Error> {
    let (_time, res) =
        get_squeue_res_ssh(client, &crate::SqueueMode::JOBIDS(vec![job_id.to_string()])).await?;
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
