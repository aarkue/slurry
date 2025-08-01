use anyhow::Error;
use chrono::{DateTime, FixedOffset, Utc};
use glob::glob;
use process_mining::{
    export_ocel_json_path,
    ocel::ocel_struct::{
        OCELAttributeType, OCELEvent, OCELObject, OCELObjectAttribute, OCELRelationship, OCELType,
        OCELTypeAttribute,
    },
    OCEL,
};
use rayon::prelude::*;
use serde::Serialize;
use slurry::{
    self,
    data_extraction::{get_squeue_res_ssh, squeue::SqueueRow, squeue_diff, SqueueMode},
    event_data_extraction::extract_ocel_from_slurm_diffs,
    job_management::{
        get_job_status, submit_job, JobFilesToUpload, JobLocalForwarding, JobOptions, JobStatus,
    },
    login_with_cfg, Client, ConnectionConfig, JobState,
};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};
use structdiff::StructDiff;
use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tauri::{async_runtime::RwLock, State};

#[tauri::command]
async fn run_squeue<'a>(state: State<'a, Arc<RwLock<AppState>>>) -> Result<String, CmdError> {
    if let Some(client) = &state.read().await.client {
        let (time, jobs) = get_squeue_res_ssh(client, &SqueueMode::ALL).await?;
        serde_json::to_writer_pretty(
            BufWriter::new(
                File::create(format!("{}.json", time.to_rfc3339().replace(":", "_"))).unwrap(),
            ),
            &jobs,
        )
        .unwrap();
        Ok(format!("Got {} jobs at {}.", jobs.len(), time.to_rfc3339()))
    } else {
        Err(Error::msg("No logged-in client available.").into())
    }
}
use tauri_plugin_dialog::DialogExt;
use tokio::time::Instant;
#[tauri::command]
async fn start_squeue_loop<'a>(
    app: AppHandle,
    state: State<'a, Arc<RwLock<AppState>>>,
    looping_interval: u64,
) -> Result<String, CmdError> {
    let path = app
        .dialog()
        .file()
        .set_directory(app.path().download_dir().unwrap())
        .blocking_pick_folder();
    if let Some(path) = path {
        let state = Arc::clone(&state);
        let path = path
            .into_path()
            .map_err(|e| Error::msg(format!("Could not handle this folder path: {:?}", e)))?
            .join(format!(
                "squeue_results_{}",
                DateTime::<Utc>::from(SystemTime::now())
                    .to_rfc3339()
                    .replace(":", "_")
            ));
        state.write().await.looping_info = Some(LoopingInfo {
            second_interval: looping_interval,
            running_since: std::time::SystemTime::now().into(),
            path: path.clone(),
        });
        async_runtime::spawn(async move {
            let mut known_jobs = HashMap::default();
            let mut all_ids = HashSet::default();
            let mut i = 0;
            'inf_loop: loop {
                // if let Some(LoopingInfo {
                //     second_interval, ..
                // }) = &state.read().await.looping_info.clone()
                // {
                let l = state.read().await;
                if let Some(client) = &l.client {
                    let res = squeue_diff(
                        || get_squeue_res_ssh(client, &SqueueMode::ALL),
                        &path,
                        &mut known_jobs,
                        &mut all_ids,
                    )
                    .await
                    .unwrap();
                    app.emit("squeue-rows", &res).unwrap();
                    i += 1;
                    drop(l);
                    println!("Ran for {} iterations, sleeping...", i);
                    for _ in 1..looping_interval {
                        if state.read().await.looping_info.is_none() {
                            println!("Stopping loop after {} iterations!", i);
                            break 'inf_loop;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                } else {
                    drop(l);
                    eprintln!("No logged-in client available.");
                    state.write().await.looping_info = None;
                    break 'inf_loop;
                }
            }
        });
        Ok("Loop running in background".to_string())
    } else {
        Err(Error::msg("No folder path selected.").into())
    }
}

#[tauri::command]
async fn stop_squeue_loop<'a>(state: State<'a, Arc<RwLock<AppState>>>) -> Result<String, CmdError> {
    if let Some(looping_info) = state.write().await.looping_info.take() {
        Ok(format!(
            "Stopped Loop running since {}",
            looping_info.running_since
        ))
    } else {
        Err(Error::msg("No loop currently running").into())
    }
}

#[tauri::command]
async fn get_loop_info<'a>(
    state: State<'a, Arc<RwLock<AppState>>>,
) -> Result<LoopingInfo, CmdError> {
    if let Some(looping_info) = &state.read().await.looping_info {
        Ok(looping_info.clone())
    } else {
        Err(Error::msg("No loop currently running").into())
    }
}

#[tauri::command]
async fn get_squeue<'a>(
    state: State<'a, Arc<RwLock<AppState>>>,
) -> Result<(DateTime<Utc>, Vec<SqueueRow>), CmdError> {
    if let Some(client) = &state.read().await.client {
        let (time, jobs) = get_squeue_res_ssh(client, &SqueueMode::ALL).await?;
        Ok((time, jobs))
    } else {
        Err(Error::msg("No logged-in client available.").into())
    }
}

#[tauri::command]
async fn login<'a>(
    state: State<'a, Arc<RwLock<AppState>>>,
    cfg: ConnectionConfig,
) -> Result<String, CmdError> {
    let client = login_with_cfg(&cfg).await?;
    state.write().await.client = Some(client);
    Ok(String::from("OK"))
}

#[tauri::command]
async fn is_logged_in<'a>(state: State<'a, Arc<RwLock<AppState>>>) -> Result<bool, CmdError> {
    Ok(state.read().await.client.is_some())
}

#[tauri::command]
async fn logout<'a>(state: State<'a, Arc<RwLock<AppState>>>) -> Result<String, CmdError> {
    if let Some(client) = state.write().await.client.take() {
        if let Err(e) = client.disconnect().await {
            return Err(Error::from(e).into());
        }
    }
    Ok(String::from("OK"))
}

// #[tauri::command]
// async fn extract_ocel(
//     data: Vec<(DateTime<FixedOffset>, Vec<SqueueRow>)>,
// ) -> Result<String, CmdError> {
//     let count: usize = data.iter().map(|(_, rows)| rows.len()).sum();
//     let mut ocel: OCEL = OCEL {
//         event_types: Vec::new(),
//         object_types: Vec::new(),
//         events: Vec::new(),
//         objects: Vec::new(),
//     };
//     #[derive(Debug, Hash, PartialEq, Eq)]
//     struct JobInfo<'a> {
//         pub id: &'a String,
//         pub command: &'a str,
//         pub work_dir: String,
//         pub cpus: usize,
//         pub min_memory: &'a String,
//         pub submit_time: &'a NaiveDateTime,
//         pub start_time: &'a Option<NaiveDateTime>,
//     }
//     impl<'a> From<&'a SqueueRow> for JobInfo<'a> {
//         fn from(r: &'a SqueueRow) -> Self {
//             Self {
//                 id: &r.job_id,
//                 command: r.command.split("/").last().unwrap_or_default(),
//                 work_dir: r.work_dir.to_string_lossy().to_string(),
//                 cpus: r.cpus,
//                 min_memory: &r.min_memory,
//                 submit_time: &r.submit_time,
//                 start_time: &r.start_time,
//             }
//         }
//     }
//     ocel.object_types.push(OCELType {
//         name: "Job".to_string(),
//         attributes: vec![
//             OCELTypeAttribute::new("command", &OCELAttributeType::String),
//             OCELTypeAttribute::new("work_dir", &OCELAttributeType::String),
//             OCELTypeAttribute::new("cpus", &OCELAttributeType::Integer),
//             OCELTypeAttribute::new("min_memory", &OCELAttributeType::String),
//         ],
//     });
//     ocel.object_types.push(OCELType {
//         name: "Account".to_string(),
//         attributes: vec![],
//     });
//     ocel.object_types.push(OCELType {
//         name: "Group".to_string(),
//         attributes: vec![],
//     });
//     ocel.object_types.push(OCELType {
//         name: "Host".to_string(),
//         attributes: vec![],
//     });
//     ocel.object_types.push(OCELType {
//         name: "Partition".to_string(),
//         attributes: vec![],
//     });

//     ocel.event_types.push(OCELType {
//         name: "Start Job".to_string(),
//         attributes: vec![],
//     });

//     ocel.event_types.push(OCELType {
//         name: "Submit Job".to_string(),
//         attributes: vec![],
//     });

//     let job_ids: HashSet<_> = data
//         .iter()
//         .flat_map(|(_, rs)| rs)
//         .map(|r| &r.job_id)
//         .collect();
//     let rows_per_job: HashMap<_, _> = job_ids
//         .into_iter()
//         .map(|j_id| {
//             let mut rows = data
//                 .iter()
//                 .filter_map(|(t, rs)| {
//                     rs.iter()
//                         .find(|r| r.job_id == *j_id)
//                         .and_then(|r| Some((t, r.clone())))
//                 })
//                 .collect::<Vec<_>>();
//             rows.sort_by_key(|(t, _)| **t);

//             (j_id.clone(), rows)
//         })
//         .collect();
//     let mut jobs: HashMap<String, OCELObject> = rows_per_job
//         .iter()
//         .map(|(j_id, rows)| {
//             let (_last_t, last_r) = rows.last().unwrap();
//             ocel.events.push(OCELEvent::new(
//                 format!("submit_job_{}", j_id),
//                 "Submit Job",
//                 last_r.submit_time.and_utc(),
//                 Vec::new(),
//                 vec![OCELRelationship::new(j_id, "job")],
//             ));
//             if let Some(x) = last_r.start_time {
//                 ocel.events.push(OCELEvent::new(
//                     format!("start_job_{}", j_id),
//                     "Start Job",
//                     x.and_utc(),
//                     Vec::new(),
//                     vec![OCELRelationship::new(j_id, "job")],
//                 ));
//             }

//             let mut o = OCELObject {
//                 id: j_id.clone(),
//                 object_type: "Job".to_string(),
//                 attributes: vec![
//                     OCELObjectAttribute::new(
//                         "command",
//                         last_r.command.split("/").last().unwrap_or_default(),
//                         DateTime::UNIX_EPOCH,
//                     ),
//                     OCELObjectAttribute::new(
//                         "work_dir",
//                         last_r.work_dir.to_string_lossy().to_string(),
//                         DateTime::UNIX_EPOCH,
//                     ),
//                     OCELObjectAttribute::new("cpus", last_r.cpus, DateTime::UNIX_EPOCH),
//                     OCELObjectAttribute::new(
//                         "min_memory",
//                         &last_r.min_memory,
//                         DateTime::UNIX_EPOCH,
//                     ),
//                 ],
//                 relationships: vec![
//                     OCELRelationship::new(&last_r.account, "submitted by"),
//                     OCELRelationship::new(&last_r.group, "submitted by group"),
//                     OCELRelationship::new(&last_r.partition, "submitted on"),
//                 ],
//             };

//             if let Some(exec_host) = &last_r.exec_host {
//                 o.relationships
//                     .push(OCELRelationship::new(exec_host, "runs on"))
//             }
//             (j_id.clone(), o)
//         })
//         .collect();

//     let account_ids: HashSet<_> = data
//         .iter()
//         .flat_map(|(_, rs)| rs)
//         .map(|r| &r.account)
//         .collect();
//     let accounts: HashMap<String, OCELObject> = account_ids
//         .into_iter()
//         .map(|a| {
//             (
//                 a.clone(),
//                 OCELObject {
//                     id: a.clone(),
//                     object_type: "Account".to_string(),
//                     attributes: Vec::default(),
//                     relationships: Vec::default(),
//                 },
//             )
//         })
//         .collect();

//     let group_ids: HashSet<_> = data
//         .iter()
//         .flat_map(|(_, rs)| rs)
//         .map(|r| &r.group)
//         .collect();
//     let groups: HashMap<String, OCELObject> = group_ids
//         .into_iter()
//         .map(|a| {
//             (
//                 a.clone(),
//                 OCELObject {
//                     id: a.clone(),
//                     object_type: "Group".to_string(),
//                     attributes: Vec::default(),
//                     relationships: Vec::default(),
//                 },
//             )
//         })
//         .collect();

//     let exec_hosts_ids: HashSet<_> = data
//         .iter()
//         .flat_map(|(_, rs)| rs)
//         .filter_map(|r| r.exec_host.as_ref())
//         .collect();
//     let exec_hosts: HashMap<String, OCELObject> = exec_hosts_ids
//         .into_iter()
//         .map(|a| {
//             (
//                 a.clone(),
//                 OCELObject {
//                     id: a.clone(),
//                     object_type: "Host".to_string(),
//                     attributes: Vec::default(),
//                     relationships: Vec::default(),
//                 },
//             )
//         })
//         .collect();

//     let partition_ids: HashSet<_> = data
//         .iter()
//         .flat_map(|(_, rs)| rs)
//         .map(|r| &r.partition)
//         .collect();
//     let partitions: HashMap<String, OCELObject> = partition_ids
//         .into_iter()
//         .map(|a| {
//             (
//                 a.clone(),
//                 OCELObject {
//                     id: a.clone(),
//                     object_type: "Partition".to_string(),
//                     attributes: Vec::default(),
//                     relationships: Vec::default(),
//                 },
//             )
//         })
//         .collect();

//     ocel.objects.extend(jobs.into_values());
//     ocel.objects.extend(accounts.into_values());
//     ocel.objects.extend(exec_hosts.into_values());
//     ocel.objects.extend(groups.into_values());
//     ocel.objects.extend(partitions.into_values());

//     // Check that all IDs are unique
//     let obj_ids: HashSet<_> = ocel.objects.iter().map(|o| &o.id).collect();
//     let ev_ids: HashSet<_> = ocel.events.iter().map(|e| &e.id).collect();
//     assert_eq!(obj_ids.len(), ocel.objects.len());
//     assert_eq!(ev_ids.len(), ocel.events.len());

//     export_ocel_json_path(&ocel, "ocel-export.json").unwrap();
//     Ok(format!("Got {} rows.", count))
// }

#[tauri::command(async)]
async fn extract_ocel(app: AppHandle) -> Result<String, CmdError> {
    let src_path = app
        .dialog()
        .file()
        .set_directory(app.path().download_dir().unwrap())
        .blocking_pick_folder();
    if let Some(src_path) = src_path {
        let dest_path = app
            .dialog()
            .file()
            .set_directory(app.path().download_dir().unwrap())
            .set_file_name("hpc-ocel-complete.json")
            .blocking_save_file();
        if let Some(dest_path) = dest_path {
            // TODO: Call ocel extraction function
            let ocel = extract_ocel_from_slurm_diffs(src_path.as_path().unwrap())?;
            export_ocel_json_path(&ocel, dest_path.as_path().unwrap()).unwrap();
            return Ok(format!(
                "Extracted OCEL with {} objects and {} events",
                ocel.objects.len(),
                ocel.events.len()
            ));
        }
    }
    Err(Error::msg("No source or destination selected.").into())
}

#[tauri::command]
async fn start_test_job<'a>(state: State<'a, Arc<RwLock<AppState>>>) -> Result<String, CmdError> {
    let mut x = state.write().await;
    if let Some(client) = x.client.take() {
        let arc = Arc::new(client);
        let res = submit_job(
            arc.clone(),
            JobOptions {
                root_dir: "hpc_experiments".to_string(),
                num_cpus: 12,
                time: "0-00:01:00".to_string(),
                local_forwarding: Some(JobLocalForwarding { local_port: 3000, relay_port: 3000, relay_addr: "login23-1".to_string() }),
                command: "./ocpq-server".to_string(),
                files_to_upload: vec![
                    JobFilesToUpload {
                    local_path: PathBuf::from("/home/aarkue/doc/projects/OCPQ/backend/target/x86_64-unknown-linux-gnu/release/ocedeclare-web-server"),
                    remote_subpath: "".to_string(),
                    remote_file_name: "ocpq-server".to_string(),
                },
            //     JobFilesToUpload {
            //     local_path: PathBuf::from("/home/aarkue/dow/ocel/bpic2017-o2o-workflow-qualifier.json"),
            //     remote_subpath: "../data".to_string(),
            //     remote_file_name: "bpic2017-o2o-workflow-qualifier.json".to_string(),
            // }
                ].into_iter().collect(),
            },
        )
        .await;
        // Get our client back
        x.client = Some(Arc::into_inner(arc).unwrap());
        return match res {
            Ok((_folder_id, job_id)) => Ok(job_id),
            Err(e) => Err(e.into()),
        };
    }
    Err(Error::msg("Did not do it :(").into())
}

#[tauri::command]
async fn check_job_status<'a>(
    state: State<'a, Arc<RwLock<AppState>>>,
    job_id: String,
) -> Result<JobStatus, CmdError> {
    match &state.read().await.client {
        Some(client) => {
            let status = get_job_status(client, &job_id).await?;
            Ok(status)
        }
        None => Err(Error::msg("No client available.").into()),
    }
}
pub fn extract_timestamp(s: &str) -> DateTime<Utc> {
    // 2025-01-04T00-55-04.789009695+00-00
    // let (date, time) = s.split_once("T").unwrap();
    // let dt_rfc = format!("{}T{}", date, time.replace("-", ":"));
    // DateTime::parse_from_rfc3339(&dt_rfc).unwrap().to_utc()
    DateTime::parse_from_rfc3339(&s.replace("_", ":"))
        .unwrap()
        .to_utc()
}

struct CmdError {
    pub error: Error,
}

impl From<Error> for CmdError {
    fn from(error: Error) -> Self {
        Self { error }
    }
}

impl Serialize for CmdError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.error.to_string().as_ref())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(RwLock::new(AppState::default())))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            run_squeue,
            start_squeue_loop,
            stop_squeue_loop,
            get_loop_info,
            extract_ocel,
            login,
            logout,
            is_logged_in,
            get_squeue,
            start_test_job,
            check_job_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Debug, Default)]
struct AppState {
    pub client: Option<Client>,
    pub looping_info: Option<LoopingInfo>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LoopingInfo {
    second_interval: u64,
    running_since: DateTime<Utc>,
    path: PathBuf,
}
