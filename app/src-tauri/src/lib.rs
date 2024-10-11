use anyhow::Error;
use rust_slurm::{self, get_squeue_res, login_with_cfg, ConnectionConfig};
use serde::Serialize;
// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet(name: String) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn test_squeue(cfg: ConnectionConfig) -> Result<String, CmdError> {
    let client = login_with_cfg(&cfg).await?;
    let (time, jobs) = get_squeue_res(&client).await?;
    Ok(format!("Got {} jobs at {}.",jobs.len(),time.to_rfc3339()))
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
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet, test_squeue])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
