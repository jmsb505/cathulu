import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./style.css";

if (new URLSearchParams(window.location.search).get("window") === "monitor") {
  document.documentElement.classList.add("monitor-document");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
