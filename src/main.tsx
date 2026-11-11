import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { logError } from "./lib/ipc";
import "./i18n";
import "./styles.css";

// Uncaught page errors go to the log file.
window.addEventListener("error", (e) => void logError(`${e.message} (${e.filename.split("/").pop()}:${e.lineno})`));
window.addEventListener("unhandledrejection", (e) => void logError(`unhandled rejection: ${e.reason}`));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
