import App from "@/App";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App context={{
      runSqueue: async (cfg) => {
        return await invoke("run_squeue", { cfg });
      },
      extractOCEL: async (data) => {
        console.log({data});
        return await invoke("extract_ocel", {data});
      }
    }} />
  </React.StrictMode>,
);
