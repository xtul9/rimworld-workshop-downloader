import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import QueryTab from "./components/QueryTab";
import DownloadTab from "./components/DownloadTab";
import SettingsTab from "./components/SettingsTab";
import { ModsPathProvider, useModsPath } from "./contexts/ModsPathContext";
import { ModsProvider } from "./contexts/ModsContext";
import { SettingsProvider, useSettings } from "./contexts/SettingsContext";
import { ModalProvider, useModal } from "./contexts/ModalContext";
import { ContextMenuProvider } from "./contexts/ContextMenuContext";
import RestoreBackupModal from "./components/RestoreBackupModal";
import ContextMenu from "./components/ContextMenu";
import { Theme } from "./utils/settingsStorage";
import "./App.css";

function AppContent() {
  const { error } = useModsPath();
  const { settings, isLoading } = useSettings();
  const { modalType, modalData } = useModal();
  const [activeTab, setActiveTab] = useState<"query" | "download" | "settings">("query");
  const [initialTabSet, setInitialTabSet] = useState(false);
  
  // Set up keyboard shortcut to open devtools (F12 or Ctrl+Shift+I)
  useEffect(() => {
    const handleKeyDown = async (event: KeyboardEvent) => {
      // F12 or Ctrl+Shift+I (or Cmd+Option+I on Mac)
      if (
        event.key === "F12" ||
        (event.ctrlKey && event.shiftKey && event.key === "I") ||
        (event.metaKey && event.altKey && event.key === "I")
      ) {
        event.preventDefault();
        try {
          await invoke("open_devtools");
          console.log("Developer tools opened");
        } catch (error) {
          console.error("Failed to open developer tools:", error);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);
  
  // Set initial tab based on isFirstRun after settings load
  useEffect(() => {
    if (!isLoading && !initialTabSet) {
      if (settings.isFirstRun) {
        setActiveTab("settings");
      }
      setInitialTabSet(true);
    }
  }, [isLoading, settings.isFirstRun, initialTabSet]);

  // Apply theme
  useEffect(() => {
    const applyTheme = (theme: Theme) => {
      const root = document.documentElement;
      if (theme === "system") {
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        root.classList.toggle("dark", prefersDark);
      } else {
        root.classList.toggle("dark", theme === "dark");
      }
    };

    applyTheme(settings.theme);

    // Listen for system theme changes if theme is set to "system"
    if (settings.theme === "system") {
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = (e: MediaQueryListEvent) => {
        document.documentElement.classList.toggle("dark", e.matches);
      };
      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    }
  }, [settings.theme]);

  return (
    <div className="app-container">
      <div className="app-tabs">
        <button
          className={`tab-button ${activeTab === "query" ? "active" : ""}`}
          onClick={() => setActiveTab("query")}
        >
          Query & Update
        </button>
        <button
          className={`tab-button ${activeTab === "download" ? "active" : ""}`}
          onClick={() => setActiveTab("download")}
        >
          Download
        </button>
        <button
          className={`tab-button ${activeTab === "settings" ? "active" : ""}`}
          onClick={() => setActiveTab("settings")}
        >
          Settings
        </button>
      </div>

      <main className="app-content">
        {error && (
          <div className="error-message">
            <p>{error}</p>
          </div>
        )}

        {activeTab === "query" && <QueryTab />}

        {activeTab === "download" && <DownloadTab />}

        {activeTab === "settings" && <SettingsTab />}
      </main>

      {/* Global Modals */}
      {modalType === "restore-backup" && modalData && (
        <RestoreBackupModal
          mod={modalData.mod}
          backupDate={modalData.backupDate}
          onRestoreComplete={modalData.onRestoreComplete}
          error={modalData.error}
        />
        )}

      {/* Global Context Menu */}
      <ContextMenu />
    </div>
  );
}

function App() {
  return (
    <SettingsProvider>
      <ModalProvider>
        <ContextMenuProvider>
          <ModsPathProvider>
            <ModsProvider>
              <AppContent />
            </ModsProvider>
          </ModsPathProvider>
        </ContextMenuProvider>
      </ModalProvider>
    </SettingsProvider>
  );
}

export default App;
