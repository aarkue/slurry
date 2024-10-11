import App from "@/App";
import { invoke } from "@tauri-apps/api/core";
import React from "react";
import ReactDOM from "react-dom/client";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App context={{
      testSqueue: async( cfg ) => {
        return await invoke("test_squeue",{cfg});
      }
    }} />
  </React.StrictMode>,
);
