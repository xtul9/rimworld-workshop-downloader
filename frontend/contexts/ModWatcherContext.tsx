import { createContext, useContext, ReactNode, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "./SettingsContext";

interface ModWatcherContextType {
  // Context for managing mod watcher globally
  // No public API needed - watcher runs automatically based on settings.modsPath
}

const ModWatcherContext = createContext<ModWatcherContextType | undefined>(undefined);

export function ModWatcherProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();

  // Manage mod watcher globally - start/stop based on modsPath
  // This ensures watcher runs regardless of which tab is active
  useEffect(() => {
    if (settings.modsPath && settings.modsPath.trim().length > 0) {
      // Stop existing watcher first
      invoke("stop_mod_watcher")
        .then(() => {
          // Start watcher with new path
          return invoke("start_mod_watcher", { modsPath: settings.modsPath });
        })
        .then(() => {
          console.log("[MOD_WATCHER] Started mod watcher for path:", settings.modsPath);
        })
        .catch((error) => {
          console.error("[MOD_WATCHER] Failed to start mod watcher:", error);
        });
    } else {
      // Stop watcher if modsPath is empty
      invoke("stop_mod_watcher")
        .then(() => {
          console.log("[MOD_WATCHER] Stopped mod watcher (no modsPath)");
        })
        .catch(console.error);
    }
    
    // Cleanup: stop watcher when component unmounts or modsPath changes
    return () => {
      invoke("stop_mod_watcher").catch(console.error);
    };
  }, [settings.modsPath]);

  return (
    <ModWatcherContext.Provider value={{}}>
      {children}
    </ModWatcherContext.Provider>
  );
}

export function useModWatcher() {
  const context = useContext(ModWatcherContext);
  if (context === undefined) {
    throw new Error("useModWatcher must be used within a ModWatcherProvider");
  }
  return context;
}

