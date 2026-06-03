import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import App from "./App";
import { ChatWindow } from "./screens/ChatWindow";
import { NodeWindow } from "./screens/NodeWindow";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const windowType = params.get("window");

// Hide splash immediately for non-main windows
if (windowType) {
  const splash = document.getElementById("splash-screen");
  if (splash) splash.style.display = "none";
}

function Root() {
  if (windowType === "chat") {
    return (
      <ChatWindow
        sessionId={params.get("sessionId")!}
        projectPath={decodeURIComponent(params.get("projectPath")!)}
        sessionName={decodeURIComponent(params.get("sessionName") ?? "Chat")}
        projectId={params.get("projectId") ?? undefined}
      />
    );
  }
  if (windowType === "node") {
    const variant = (params.get("nodeVariant") ?? "module") as
      | "module"
      | "knowledge_node"
      | "lighthouse"
      | "buoy"
      | "cylinder"
    return (
      <NodeWindow
        projectPath={decodeURIComponent(params.get("projectPath")!)}
        moduleId={decodeURIComponent(params.get("moduleId")!)}
        moduleName={decodeURIComponent(params.get("moduleName") ?? "Nodo")}
        nodeVariant={variant}
      />
    );
  }
  return <App />;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);
