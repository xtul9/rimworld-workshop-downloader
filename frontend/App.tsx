import { useState, useEffect } from "react";
import QueryTab from "./components/QueryTab";
import DownloadTab from "./components/DownloadTab";
import SettingsTab from "./components/SettingsTab";
import InstalledModsTab from "./components/InstalledModsTab";
import { ModsPathProvider, useModsPath } from "./contexts/ModsPathContext";
import { ModsProvider } from "./contexts/ModsContext";
import { InstalledModsProvider } from "./contexts/InstalledModsContext";
import { SettingsProvider, useSettings } from "./contexts/SettingsContext";
import { ModalProvider, useModal } from "./contexts/ModalContext";
import { ContextMenuProvider } from "./contexts/ContextMenuContext";
import { AccessErrorProvider, useAccessError } from "./contexts/AccessErrorContext";
import { ModWatcherProvider } from "./contexts/ModWatcherContext";
import RestoreBackupModal from "./components/RestoreBackupModal";
import ForceUpdateAllModal from "./components/ForceUpdateAllModal";
import ContextMenu from "./components/ContextMenu";
import AccessErrorBanner from "./components/AccessErrorBanner";
import { Theme } from "./utils/settingsStorage";
import "./App.css";

function AppContent() {
  const { error } = useModsPath();
  const { settings, isLoading } = useSettings();
  const { modalType, modalData } = useModal();
  const { hasActiveError } = useAccessError();
  const [activeTab, setActiveTab] = useState<"query" | "download" | "installed" | "settings">("query");
  const [initialTabSet, setInitialTabSet] = useState(false);
  
  // If there's an active access error, force settings tab and prevent switching
  useEffect(() => {
    if (hasActiveError && activeTab !== "settings") {
      setActiveTab("settings");
    }
  }, [hasActiveError, activeTab]);
  
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
      <AccessErrorBanner />
      <div className="app-tabs">
        <button
          className={`tab-button ${activeTab === "query" ? "active" : ""} ${hasActiveError ? "disabled" : ""}`}
          onClick={() => !hasActiveError && setActiveTab("query")}
          disabled={hasActiveError}
        >
          Query & Update
        </button>
        <button
          className={`tab-button ${activeTab === "installed" ? "active" : ""} ${hasActiveError ? "disabled" : ""}`}
          onClick={() => !hasActiveError && setActiveTab("installed")}
          disabled={hasActiveError}
        >
          Installed Mods
        </button>
        <button
          className={`tab-button ${activeTab === "download" ? "active" : ""} ${hasActiveError ? "disabled" : ""}`}
          onClick={() => !hasActiveError && setActiveTab("download")}
          disabled={hasActiveError}
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

      <main className={`app-content ${hasActiveError ? "blocked" : ""}`}>
        {error && (
          <div className="error-message">
            <p>{error}</p>
          </div>
        )}

        {!hasActiveError && activeTab === "query" && <QueryTab />}

        {!hasActiveError && activeTab === "installed" && <InstalledModsTab />}

        {!hasActiveError && activeTab === "download" && <DownloadTab />}

        {activeTab === "settings" && <SettingsTab />}
        
        {hasActiveError && activeTab !== "settings" && (
          <div className="access-blocked-message">
            <p>Application is blocked due to directory access error. Please fix the issue in Settings or dismiss the error banner.</p>
          </div>
        )}
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
      {modalType === "force-update-all" && modalData && (
        <ForceUpdateAllModal
          modsCount={modalData.modsCount}
          totalSize={modalData.totalSize}
          onConfirm={modalData.onConfirm}
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
          <AccessErrorProvider>
            <ModWatcherProvider>
              <ModsPathProvider>
                <ModsProvider>
                  <InstalledModsProvider>
                    <AppContent />
                  </InstalledModsProvider>
                </ModsProvider>
              </ModsPathProvider>
            </ModWatcherProvider>
          </AccessErrorProvider>
        </ContextMenuProvider>
      </ModalProvider>
    </SettingsProvider>
  );
}

export default App;
