import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App context={{
      testSqueue: async () => {
        throw new Error("Server not implemented yet.")
      }
    }}/>
  </React.StrictMode>,
);
