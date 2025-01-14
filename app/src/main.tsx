import App from "@/App";
import { SqueueRow } from "@/AppContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import React from "react";
import ReactDOM from "react-dom/client";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App context={{
      runSqueue: async () => {
        return await invoke("run_squeue");
      },
      startSqueueLoop: async (loopingInterval) => {
        return await invoke("start_squeue_loop", { loopingInterval })
      },
      stopSqueueLoop: async () => {
        return await invoke("stop_squeue_loop")
      },
      getLoopInfo: async () => {
        return await invoke("get_loop_info")
      },
      getSqueue: async () => {
        return await invoke("get_squeue");
      },
      extractOCEL: async () => {
        return await invoke("extract_ocel");
      },
      login: async (cfg) => {
        return await invoke("login", { cfg });
      },
      logout: async () => {
        return await invoke("logout");
      },
      isLoggedIn: async () => {
        return await invoke("is_logged_in")
      },
      listenSqueue: (listener) => {
        return listen<[string,SqueueRow[]]>("squeue-rows", (e) => listener(e.payload))
      }
    }} />
  </React.StrictMode>,
);
