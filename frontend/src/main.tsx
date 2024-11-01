import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { DEFAULT_NO_CONTEXT } from "./AppContext";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App context={DEFAULT_NO_CONTEXT} />
  </React.StrictMode>,
);
