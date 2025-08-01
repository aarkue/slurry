use std::{collections::HashSet, fs::File, path::Path, time::Instant};

use chrono::{DateTime, FixedOffset};
use glob::glob;
use process_mining::{
    self,
    ocel::ocel_struct::{
        OCELAttributeType, OCELEvent, OCELObject, OCELObjectAttribute, OCELRelationship, OCELType,
        OCELTypeAttribute,
    },
    OCEL,
};
use rayon::prelude::*;
use structdiff::StructDiff;

use crate::{data_extraction::squeue::SqueueRow, misc::extract_timestamp, JobState};

/// Extract an object-centric event dataset ([`OCEL`]) from diffs recorded using SLURM commands
///
/// Requires a folder path as parameter, containing the recorded SLURM diffs as files
pub fn extract_ocel_from_slurm_diffs(path: impl AsRef<Path>) -> Result<OCEL, anyhow::Error> {
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
    let src_path = path.as_ref();
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
            let mut g =
                glob(&src_path.join(job_id).join("*.json").to_string_lossy()).expect("Glob failed");
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
                    .inspect_err(|e| eprintln!("Failed to deser.: {d:?}, {e:?}"))
                    .unwrap();

                let account = match row.account.as_str() {
                    "default" => {
                        let work_dir = row.work_dir.to_string_lossy();
                        if let Some(account_captures) = r.captures(&work_dir) {
                            let account = account_captures.get(1).map_or("", |m| m.as_str());
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
                        OCELRelationship::new(format!("acc_{}", &account), "submitted by"),
                        OCELRelationship::new(
                            format!("group_{}", &row.group),
                            "submitted by group",
                        ),
                        OCELRelationship::new(format!("part_{}", &row.partition), "submitted on"),
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
                        .and_local_timezone(FixedOffset::east_opt(3600).unwrap())
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
                            st.and_local_timezone(FixedOffset::east_opt(3600).unwrap())
                                .single()
                                .unwrap()
                                .to_utc(),
                            Vec::new(),
                            vec![
                                OCELRelationship::new(&o.id, "job"),
                                OCELRelationship::new(format!("group_{}", &row.group), "for"),
                            ],
                        );

                        if let Some(h) = row.exec_host.as_ref() {
                            execution_hosts.write().unwrap().insert(h.clone());
                            e.relationships.push(OCELRelationship::new(
                                format!("host_{}", row.exec_host.as_ref().unwrap().clone()),
                                "host",
                            ));
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
                    let dt =
                        extract_timestamp(&file_name.replace("DELTA-", "").replace(".json", ""));
                    if last_dt > dt {
                        eprintln!("Going backwards in time! {} {last_dt} -> {dt}", o.id);
                    }

                    last_dt = dt;
                    type D = <SqueueRow as StructDiff>::Diff;
                    let delta: Vec<D> = serde_json::from_reader(File::open(&d).unwrap())
                        .inspect_err(|e| {
                            println!("Serde deser. failed for {job_id} in file {d:?}; {e:?}")
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
                                o.attributes
                                    .push(OCELObjectAttribute::new("min_memory", m, dt));
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
                                println!("Account change for {a} not handled!");
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
                                    crate::JobState::RUNNING => {
                                        e.id = format!("{}_{}", "start-", e.id);
                                        e.event_type = "Job Started".to_string();
                                        ignore = true;
                                    }
                                    crate::JobState::COMPLETING => {
                                        e.id = format!("{}_{}", "ending-", e.id);
                                        e.event_type = "Job Ending".to_string()
                                    }
                                    crate::JobState::COMPLETED => {
                                        e.id = format!("{}_{}", "ended-", e.id);
                                        e.event_type = "Job Completed".to_string()
                                    }
                                    crate::JobState::CANCELLED => {
                                        e.id = format!("{}_{}", "cancelled-", e.id);
                                        e.event_type = "Job Cancelled".to_string()
                                    }
                                    crate::JobState::FAILED => {
                                        e.id = format!("{}_{}", "failed-", e.id);
                                        e.event_type = "Job Failed".to_string()
                                    }
                                    crate::JobState::TIMEOUT => {
                                        e.id = format!("{}_{}", "timeout-", e.id);
                                        e.event_type = "Job Timeout".to_string()
                                    }
                                    crate::JobState::OUT_OF_MEMORY => {
                                        e.id = format!("{}_{}", "oom-", e.id);
                                        e.event_type = "Job Out Of Memory".to_string()
                                    }
                                    crate::JobState::NODE_FAIL => {
                                        e.id = format!("{}_{}", "node-fail-", e.id);
                                        e.event_type = "Job Node Fail".to_string()
                                    }
                                    crate::JobState::PENDING => {
                                        // Status change TO pending?
                                        // Hmm..
                                        //             eprintln!(
                                        //     "Unexpected job ID {} state change to pending. Attrs: {:?}",
                                        //     o.id, o.attributes
                                        // );
                                        ignore = true;
                                    }
                                    crate::JobState::OTHER(other) => {
                                        eprintln!(
                                            "Unexpected job state change to other: {other}"
                                        );
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
                                                    FixedOffset::east_opt(3600).unwrap(),
                                                )
                                                .single()
                                                .unwrap();
                                        } else {
                                            let e = OCELEvent::new(
                                                format!("start-{}-{}", o.id, ocel.events.len()),
                                                "Job Started",
                                                st.and_local_timezone(
                                                    FixedOffset::east_opt(3600).unwrap(),
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
            id: format!("acc_{a}"),
            object_type: "Account".to_string(),
            attributes: Vec::default(),
            relationships: Vec::default(),
        }));

    ocel.objects
        .extend(groups.into_inner().unwrap().iter().map(|a| OCELObject {
            id: format!("group_{a}"),
            object_type: "Group".to_string(),
            attributes: Vec::default(),
            relationships: Vec::default(),
        }));

    ocel.objects
        .extend(partitions.into_inner().unwrap().iter().map(|a| OCELObject {
            id: format!("part_{a}"),
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
                id: format!("host_{a}"),
                object_type: "Host".to_string(),
                attributes: Vec::default(),
                relationships: Vec::default(),
            }),
    );

    Ok(ocel)
}
