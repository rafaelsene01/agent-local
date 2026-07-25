import { useEffect } from "react";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { ChatPanel } from "./components/Chat/ChatPanel";
import { SettingsPanel } from "./components/Settings/SettingsPanel";
import { ConnectionsPanel } from "./components/Connections/ConnectionsPanel";
import { Wizard } from "./components/Onboarding/Wizard";
import { useConfigStore } from "./store/configStore";
import { useUiStore } from "./store/uiStore";

function App() {
  const { status, loadConfig } = useConfigStore();
  const activeView = useUiStore((s) => s.activeView);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  if (status === "loading") {
    return <div className="h-screen w-screen bg-[var(--bg-app)]" />;
  }

  if (status === "needs-onboarding") {
    return <Wizard />;
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--bg-app)]">
      <Sidebar />
      {activeView === "settings" ? (
        <SettingsPanel />
      ) : activeView === "connections" ? (
        <ConnectionsPanel />
      ) : (
        <ChatPanel />
      )}
    </div>
  );
}

export default App;
