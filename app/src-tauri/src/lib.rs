use anyhow::Error;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
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
use rust_slurm::{
    self, get_squeue_res_ssh,
    jobs_management::{
        get_job_status, submit_job, JobFilesToUpload, JobLocalForwarding, JobOptions, JobStatus,
    },
    login_with_cfg, squeue_diff, Client, ConnectionConfig, JobState, SqueueMode, SqueueRow,
};
use serde::Serialize;
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
use tokio::{sync::Mutex, time::Instant};
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
            let mut ocel: OCEL = OCEL {
                event_types: Vec::new(),
                object_types: Vec::new(),
                events: Vec::new(),
                objects: Vec::new(),
            };
            ocel.object_types.push(OCELType {
                name: "Job".to_string(),
                attributes: vec![
                    OCELTypeAttribute::new("state", &OCELAttributeType::String),
                    OCELTypeAttribute::new("command", &OCELAttributeType::String),
                    OCELTypeAttribute::new("work_dir", &OCELAttributeType::String),
                    OCELTypeAttribute::new("cpus", &OCELAttributeType::Integer),
                    OCELTypeAttribute::new("min_memory", &OCELAttributeType::String),
                ],
            });

            ocel.object_types.push(OCELType {
                name: "Account".to_string(),
                attributes: vec![],
            });
            ocel.object_types.push(OCELType {
                name: "Group".to_string(),
                attributes: vec![],
            });
            ocel.object_types.push(OCELType {
                name: "Host".to_string(),
                attributes: vec![],
            });
            ocel.object_types.push(OCELType {
                name: "Partition".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Submit Job".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Started".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Ending".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Completed".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Cancelled".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Failed".to_string(),
                attributes: vec![OCELTypeAttribute::new("reason", &OCELAttributeType::String)],
            });

            ocel.event_types.push(OCELType {
                name: "Job Timeout".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Out Of Memory".to_string(),
                attributes: vec![],
            });

            ocel.event_types.push(OCELType {
                name: "Job Node Fail".to_string(),
                attributes: vec![],
            });
            let src_path = src_path.as_path().unwrap();
            println!("Before gathering jobs...");
            let now: Instant = Instant::now();
            // let jobs_per_time: HashMap<DateTime<Utc>, HashSet<String>> =
            //     glob(&src_path.join("*.json").to_string_lossy())
            //         .expect("Glob failed")
            //         .into_iter().par_bridge()
            //         .flat_map(|entry| match entry {
            //             Ok(j) => {
            //                 let job_ids: HashSet<String> =
            //                     serde_json::from_reader(File::open(&j).unwrap()).unwrap();
            //                 let time = extract_timestamp(
            //                     &j.file_name()
            //                         .unwrap()
            //                         .to_string_lossy()
            //                         .replace(".json", ""),
            //                 );
            //                 Some((time, job_ids))
            //             }
            //             Err(_) => None,
            //         })
            //         .collect();
            //     println!(
            //     "Gathered jobs per time in {:?}",
            //     now.elapsed()
            // );
            let all_jobs_ids: HashSet<String> = glob(&src_path.join("*/").to_string_lossy())
                .expect("Glob failed")
                .into_iter()
                .par_bridge()
                .flat_map(|entry| match entry {
                    Ok(j) => j.file_name().and_then(|n| n.to_str().map(String::from)),
                    Err(_) => None,
                })
                .collect();
            println!("First job ID: {:?}", all_jobs_ids.iter().next());
            // let all_jobs_ids: HashSet<&String> = jobs_per_time.values().flatten().collect();
            println!(
                "Recorded {} jobs overall. Gathered in {:?}",
                all_jobs_ids.len(),
                now.elapsed()
            );

            let accounts: std::sync::RwLock<HashSet<String>> = Default::default();
            let groups: std::sync::RwLock<HashSet<String>> = Default::default();
            let partitions: std::sync::RwLock<HashSet<String>> = Default::default();
            let execution_hosts: std::sync::RwLock<HashSet<String>> = Default::default();
            let r = regex::Regex::new(r"\/rwthfs\/rz\/cluster\/home\/([^\/]*)\/.*").unwrap();
            // Go through all jobs
            // Only consider jobs which start as 'PENDING'
            let (obs, evs): (Vec<_>, Vec<_>) = all_jobs_ids
                .par_iter()
                .flat_map(|job_id| {
                    let mut events: Vec<_> = Vec::new();
                    let mut g = glob(&src_path.join(job_id).join("*.json").to_string_lossy())
                        .expect("Glob failed");
                    let mut start_ev: Option<OCELEvent> = None;
                    if let Some(Ok(d)) = g.next() {
                        let dt = extract_timestamp(
                            &d.file_name()
                                .unwrap()
                                .to_string_lossy()
                                .replace(".json", ""),
                        );
                        // Initial Job Data
                        // This is assumed to then be the first result (i.e., initial job data)
                        let mut row: SqueueRow = serde_json::from_reader(File::open(&d).unwrap())
                            .inspect_err(|e| eprintln!("Failed to deser.: {d:?}"))
                            .unwrap();

                        let account = match row.account.as_str() {
                            "default" => {
                                let work_dir = row.work_dir.to_string_lossy();
                                if let Some(account_captures) = r.captures(&work_dir) {
                                    let account =
                                        account_captures.get(1).map_or("", |m| m.as_str());
                                    if !account.is_empty() {
                                        account.to_string()
                                    } else {
                                        String::from("default")
                                    }
                                } else {
                                    String::from("default")
                                }
                            }
                            s => s.to_string(),
                        };
                        accounts.write().unwrap().insert(account.clone());
                        groups.write().unwrap().insert(row.group.clone());
                        partitions.write().unwrap().insert(row.partition.clone());
                        if let Some(h) = &row.exec_host {
                            execution_hosts.write().unwrap().insert(h.clone());
                        }

                        let mut o = OCELObject {
                            id: row.job_id.clone(),
                            object_type: "Job".to_string(),
                            attributes: vec![
                                OCELObjectAttribute::new(
                                    "command",
                                    row.command.split("/").last().unwrap_or_default(),
                                    DateTime::UNIX_EPOCH,
                                ),
                                OCELObjectAttribute::new(
                                    "work_dir",
                                    row.work_dir.to_string_lossy().to_string(),
                                    DateTime::UNIX_EPOCH,
                                ),
                                OCELObjectAttribute::new("cpus", row.cpus, DateTime::UNIX_EPOCH),
                                OCELObjectAttribute::new(
                                    "min_memory",
                                    &row.min_memory,
                                    DateTime::UNIX_EPOCH,
                                ),
                                OCELObjectAttribute::new("state", format!("{:?}", &row.state), dt),
                            ],
                            relationships: vec![
                                OCELRelationship::new(
                                    format!("acc_{}", &account),
                                    "submitted by",
                                ),
                                OCELRelationship::new(
                                    format!("group_{}", &row.group),
                                    "submitted by group",
                                ),
                                OCELRelationship::new(
                                    format!("part_{}", &row.partition),
                                    "submitted on",
                                ),
                            ],
                        };
                        if let Some(exec_host) = &row.exec_host {
                            o.relationships.push(OCELRelationship::new(
                                format!("host_{exec_host}"),
                                "executed on",
                            ));
                            execution_hosts.write().unwrap().insert(exec_host.clone());
                        }

                        let e = OCELEvent::new(
                            format!("submit-{}-{}", o.id, events.len()),
                            "Submit Job",
                            row.submit_time
                                .and_local_timezone(FixedOffset::east_opt(1 * 3600).unwrap())
                                .single()
                                .unwrap()
                                .to_utc(),
                            Vec::new(),
                            vec![
                                OCELRelationship::new(&o.id, "job"),
                                OCELRelationship::new(format!("acc_{}", &account), "submitter"),
                            ],
                        );
                        events.push(e);

                        if row.state != JobState::PENDING {
                            if let Some(st) = &row.start_time {
                                let mut e = OCELEvent::new(
                                    format!("start-{}-{}", o.id, events.len()),
                                    "Job Started",
                                    st.and_local_timezone(FixedOffset::east_opt(1 * 3600).unwrap())
                                        .single()
                                        .unwrap()
                                        .to_utc(),
                                    Vec::new(),
                                    vec![OCELRelationship::new(&o.id, "job"),
                                    OCELRelationship::new(&format!("group_{}",&row.group), "for"),
                                    ],
                                );
                                
                                if let Some(h) = row.exec_host.as_ref() {
                                    execution_hosts.write().unwrap().insert(h.clone());
                                    e.relationships.push(OCELRelationship::new(&format!("host_{}",row.exec_host.as_ref().unwrap().clone()), "host"));
                                }
                                start_ev = Some(e);
                            }
                        }
                        let mut last_dt = dt;
                        for d in g.flatten() {
                            let file_name = d.file_name().unwrap().to_string_lossy();
                            if !file_name.contains("DELTA") {
                                // eprintln!("JobID: [{}] No DELTA in filename {}", job_id, file_name);
                                continue;
                            }
                            let dt = extract_timestamp(
                                &file_name.replace("DELTA-", "").replace(".json", ""),
                            );
                            if last_dt > dt {
                                eprintln!("Going backwards in time! {} {last_dt} -> {dt}", o.id);
                            }

                            last_dt = dt;
                            type D = <SqueueRow as StructDiff>::Diff;
                            let delta: Vec<D> = serde_json::from_reader(File::open(&d).unwrap())
                                .inspect_err(|e| {
                                    println!("Serde deser. failed for {} in file {:?}", job_id, d)
                                })
                                .unwrap();
                            row.apply_mut(delta.clone());
                            for df in delta {
                                // println!("{:?}", df);
                                match df {
                                    D::command(c) => {
                                        o.attributes.push(OCELObjectAttribute::new(
                                            "command",
                                            c.split("/").last().unwrap_or_default(),
                                            dt,
                                        ));
                                    }
                                    D::work_dir(w) => {
                                        o.attributes.push(OCELObjectAttribute::new(
                                            "work_dir",
                                            w.to_string_lossy().to_string(),
                                            dt,
                                        ));
                                    }
                                    D::min_memory(m) => {
                                        o.attributes.push(OCELObjectAttribute::new(
                                            "min_memory",
                                            m,
                                            dt,
                                        ));
                                    }
                                    D::exec_host(h) => {
                                        if let Some(h) = &h {
                                            execution_hosts.write().unwrap().insert(h.clone());
                                            o.relationships.push(OCELRelationship::new(
                                                format!("host_{h}"),
                                                "executed on",
                                            ));
                                        }
                                    }

                                    D::account(a) => {
                                        println!("Account change not handled!");
                                        // accounts.write().unwrap().insert(a.clone());
                                        // o.relationships.push(OCELRelationship::new(
                                        //     format!("acc_{}", &row.account),
                                        //     "submitted by",
                                        // ))
                                    }
                                    D::state(s) => {
                                        o.attributes.push(OCELObjectAttribute::new(
                                            "state",
                                            format!("{:?}", &row.state),
                                            dt,
                                        ));
                                        // State update => Event!
                                        let mut e = OCELEvent::new(
                                            format!("{}-{}", o.id, ocel.events.len()),
                                            "Submit Job",
                                            dt,
                                            Vec::new(),
                                            vec![OCELRelationship::new(&o.id, "job")],
                                        );
                                        let mut ignore = false;
                                        match s {
                                            rust_slurm::JobState::RUNNING => {
                                                e.id = format!("{}_{}", "start-", e.id);
                                                e.event_type = "Job Started".to_string();
                                                ignore = true;
                                            }
                                            rust_slurm::JobState::COMPLETING => {
                                                e.id = format!("{}_{}", "ending-", e.id);
                                                e.event_type = "Job Ending".to_string()
                                            }
                                            rust_slurm::JobState::COMPLETED => {
                                                e.id = format!("{}_{}", "ended-", e.id);
                                                e.event_type = "Job Completed".to_string()
                                            }
                                            rust_slurm::JobState::CANCELLED => {
                                                e.id = format!("{}_{}", "cancelled-", e.id);
                                                e.event_type = "Job Cancelled".to_string()
                                            }
                                            rust_slurm::JobState::FAILED => {
                                                e.id = format!("{}_{}", "failed-", e.id);
                                                e.event_type = "Job Failed".to_string()
                                            }
                                            rust_slurm::JobState::TIMEOUT => {
                                                e.id = format!("{}_{}", "timeout-", e.id);
                                                e.event_type = "Job Timeout".to_string()
                                            }
                                            rust_slurm::JobState::OUT_OF_MEMORY => {
                                                e.id = format!("{}_{}", "oom-", e.id);
                                                e.event_type = "Job Out Of Memory".to_string()
                                            }
                                            rust_slurm::JobState::NODE_FAIL => {
                                                e.id = format!("{}_{}", "node-fail-", e.id);
                                                e.event_type = "Job Node Fail".to_string()
                                            }
                                            rust_slurm::JobState::PENDING => {
                                                // Status change TO pending?
                                                // Hmm..
                                                //             eprintln!(
                                                //     "Unexpected job ID {} state change to pending. Attrs: {:?}",
                                                //     o.id, o.attributes
                                                // );
                                                ignore = true;
                                            }
                                            rust_slurm::JobState::OTHER(other) => {
                                                // eprintln!(
                                                //     "Unexpected job state change to other: {}",
                                                //     other
                                                // );
                                                ignore = true;
                                            }
                                        }
                                        if !ignore {
                                            events.push(e);
                                        }
                                    }
                                    D::group(g) => {
                                        groups.write().unwrap().insert(g.clone());
                                    }
                                    D::partition(p) => {
                                        partitions.write().unwrap().insert(p.clone());
                                    }
                                    //   _ => {}
                                    D::job_id(_) => {}
                                    D::min_cpus(_) => {}
                                    D::cpus(_) => {}
                                    D::nodes(_) => {}
                                    D::end_time(_) => {}
                                    D::dependency(_) => {}
                                    D::features(_) => {}
                                    D::array_job_id(_) => {}
                                    D::step_job_id(_) => {}
                                    D::time_limit(_) => {}
                                    D::name(_) => {}
                                    D::priority(p) => {
                                        o.attributes
                                            .push(OCELObjectAttribute::new("priority", p, dt));
                                    }
                                    D::reason(_) => {}
                                    D::start_time(st) => {
                                        if row.state != JobState::PENDING {
                                            if let Some(st) = st {
                                                if let Some(e) = start_ev.as_mut() {
                                                    e.time = st
                                                        .and_local_timezone(
                                                            FixedOffset::east_opt(1 * 3600)
                                                                .unwrap(),
                                                        )
                                                        .single()
                                                        .unwrap()
                                                        .into();
                                                } else {
                                                    let e = OCELEvent::new(
                                                        format!(
                                                            "start-{}-{}",
                                                            o.id,
                                                            ocel.events.len()
                                                        ),
                                                        "Job Started",
                                                        st.and_local_timezone(
                                                            FixedOffset::east_opt(1 * 3600)
                                                                .unwrap(),
                                                        )
                                                        .single()
                                                        .unwrap()
                                                        .to_utc(),
                                                        Vec::new(),
                                                        vec![OCELRelationship::new(&o.id, "job")],
                                                    );
                                                    start_ev = Some(e);
                                                }
                                            }
                                        }
                                    }
                                    D::submit_time(_) => {}
                                };
                            }
                        }
                        if let Some(start_event) = start_ev {
                            events.push(start_event);
                        }

                        return Some((o, events));
                    }
                    None
                })
                .unzip();

            ocel.objects.extend(obs);

            ocel.events.extend(evs.into_iter().flatten());

            ocel.objects
                .extend(accounts.into_inner().unwrap().iter().map(|a| OCELObject {
                    id: format!("acc_{}", a),
                    object_type: "Account".to_string(),
                    attributes: Vec::default(),
                    relationships: Vec::default(),
                }));

            ocel.objects
                .extend(groups.into_inner().unwrap().iter().map(|a| OCELObject {
                    id: format!("group_{}", a),
                    object_type: "Group".to_string(),
                    attributes: Vec::default(),
                    relationships: Vec::default(),
                }));

            ocel.objects
                .extend(partitions.into_inner().unwrap().iter().map(|a| OCELObject {
                    id: format!("part_{}", a),
                    object_type: "Partition".to_string(),
                    attributes: Vec::default(),
                    relationships: Vec::default(),
                }));

            ocel.objects.extend(
                execution_hosts
                    .into_inner()
                    .unwrap()
                    .iter()
                    .map(|a| OCELObject {
                        id: format!("host_{}", a),
                        object_type: "Host".to_string(),
                        attributes: Vec::default(),
                        relationships: Vec::default(),
                    }),
            );
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
            Ok((folder_id, job_id)) => Ok(job_id),
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
