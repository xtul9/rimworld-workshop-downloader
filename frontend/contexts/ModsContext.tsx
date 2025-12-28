import { createContext, useContext, useState, ReactNode, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BaseMod } from "../types";
import { useSettings } from "./SettingsContext";

export type ModState = "queued" | "downloading" | "download-complete" | "installing" | "failed" | null;

interface ModsContextType {
  mods: BaseMod[];
  setMods: (mods: BaseMod[] | ((prev: BaseMod[]) => BaseMod[])) => void;
  isQuerying: boolean;
  isUpdating: boolean;
  error: string | null;
  updatingMods: Set<string>;
  downloadedMods: Set<string>;
  modStates: Map<string, ModState>;
  modErrors: Map<string, string>;
  hasQueried: boolean;
  queryMods: (modsPath: string) => Promise<void>;
  updateMods: (modsToUpdate: BaseMod[]) => Promise<void>;
  removeMods: (modsToRemove: BaseMod[]) => void;
  ignoreFromList: (modsToIgnore: BaseMod[]) => void;
  ignoreThisUpdate: (modsToIgnore: BaseMod[]) => Promise<void>;
  ignorePermanently: (modsToIgnore: BaseMod[]) => Promise<void>;
}

const ModsContext = createContext<ModsContextType | undefined>(undefined);

export function ModsProvider({ children }: { children: ReactNode }) {
  const [mods, setMods] = useState<BaseMod[]>([]);
  const [isQuerying, setIsQuerying] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatingMods, setUpdatingMods] = useState<Set<string>>(new Set());
  const [downloadedMods, setDownloadedMods] = useState<Set<string>>(new Set());
  const [modStates, setModStates] = useState<Map<string, ModState>>(new Map());
  const [modErrors, setModErrors] = useState<Map<string, string>>(new Map());
  const [hasQueried, setHasQueried] = useState(false);
  const { updateSetting, settings } = useSettings();

  // Listen for real-time download and update events
  useEffect(() => {
    let unlistenDownloaded: (() => void) | undefined;
    let unlistenUpdated: (() => void) | undefined;
    let unlistenState: (() => void) | undefined;

    const setupListeners = async () => {
      // Listen for mod-state events (queued, downloading, installing, etc.)
      unlistenState = await listen<{ modId: string; state: string }>("mod-state", (event) => {
        const { modId, state } = event.payload;
        console.log(`[EVENT] Mod state changed: ${modId} -> ${state}`);
        
        setModStates(prev => {
          const newMap = new Map(prev);
          if (state === "queued" || state === "downloading" || state === "download-complete" || state === "installing" || state === "failed") {
            newMap.set(modId, state as ModState);
            // Add to updatingMods when state changes to any active state (except failed)
            if (state !== "failed") {
              setUpdatingMods(prevSet => {
                const newSet = new Set(prevSet);
                newSet.add(modId);
                return newSet;
              });
            } else {
              // Remove from updatingMods when failed
              setUpdatingMods(prevSet => {
                const newSet = new Set(prevSet);
                newSet.delete(modId);
                return newSet;
              });
            }
          } else {
            newMap.delete(modId);
          }
          return newMap;
        });
        
        // Handle download-complete state
        if (state === "download-complete") {
          setDownloadedMods(prev => new Set([...prev, modId]));
        }
        
        // Handle failed state - store error message for this specific mod
        if (state === "failed") {
          const errorMessage = (event.payload as any).error || "Download failed";
          setModErrors(prev => {
            const newMap = new Map(prev);
            newMap.set(modId, errorMessage);
            return newMap;
          });
        } else {
          // Clear error when mod state changes to non-failed
          setModErrors(prev => {
            const newMap = new Map(prev);
            newMap.delete(modId);
            return newMap;
          });
        }
      });

      // Listen for mod-downloaded events (backward compatibility)
      unlistenDownloaded = await listen<{ modId: string }>("mod-downloaded", (event) => {
        const modId = event.payload.modId;
        console.log(`[EVENT] Mod downloaded: ${modId}`);
        setDownloadedMods(prev => new Set([...prev, modId]));
      });

      // Listen for mod-updated events
      unlistenUpdated = await listen<{ modId: string; success: boolean; error?: string }>("mod-updated", (event) => {
        const { modId, success, error } = event.payload;
        console.log(`[EVENT] Mod updated: ${modId}, success: ${success}`);
        
        // Clear mod state when update completes
        setModStates(prev => {
          const newMap = new Map(prev);
          newMap.delete(modId);
          return newMap;
        });
        
                if (success) {
                  // Remove mod from list immediately when updated
                  setMods(prev => prev.filter(m => m.modId !== modId));
                  setUpdatingMods(prev => {
                    const newSet = new Set(prev);
                    newSet.delete(modId);
                    return newSet;
                  });
                  setDownloadedMods(prev => {
                    const newSet = new Set(prev);
                    newSet.delete(modId);
                    return newSet;
                  });
                  // Clear any errors
                  setModErrors(prev => {
                    const newMap = new Map(prev);
                    newMap.delete(modId);
                    return newMap;
                  });
                } else {
                  // Handle error - keep mod in list but mark as failed
                  console.error(`[EVENT] Mod update failed: ${modId}, error: ${error}`);
                  setUpdatingMods(prev => {
                    const newSet = new Set(prev);
                    newSet.delete(modId);
                    return newSet;
                  });
                  // Store error for this mod
                  setModErrors(prev => {
                    const newMap = new Map(prev);
                    newMap.set(modId, error || "Update failed");
                    return newMap;
                  });
                  // Set mod state to failed
                  setModStates(prev => {
                    const newMap = new Map(prev);
                    newMap.set(modId, "failed");
                    return newMap;
                  });
                }
      });
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenState?.();
      unlistenDownloaded?.();
      unlistenUpdated?.();
    };
  }, []);

  const queryMods = async (modsPath: string) => {
    if (isQuerying) return;
    
    // Validate modsPath before making request
    if (!modsPath || modsPath.trim().length === 0) {
      setError("Error querying mods: Mods folder path is not set. Please configure it in Settings.");
      setIsQuerying(false);
      return;
    }
    
    setIsQuerying(true);
    setError(null);
    setHasQueried(false);
    
    try {
      // Include ignoredMods in query (extract only IDs for backend)
      const ignoredMods = settings.ignoredMods || [];
      const ignoredModIds = ignoredMods.map(mod => typeof mod === 'string' ? mod : mod.modId).filter(Boolean); // Support both old format (string[]) and new format (IgnoredMod[])
      
      // Call Tauri command instead of fetch
      const mods = await invoke<BaseMod[]>("query_mods", {
        modsPath: modsPath,
        ignoredMods: ignoredModIds
      });
      
      console.log(`[QUERY] Received ${mods.length} mods from Rust backend`);
      
      // Always update the mods list with new query results
      setMods(mods);
      setError(null);
      setHasQueried(true);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error("Failed to query mods:", error);
      setError(`Error querying mods: ${errorMessage}`);
      setMods([]);
      setHasQueried(false);
    } finally {
      setIsQuerying(false);
    }
  };

  const updateMods = async (modsToUpdate: BaseMod[]) => {
    if (modsToUpdate.length === 0) return;
    
    setIsUpdating(true);
    setError(null);
    
    // Mark mods as updating and clear downloaded mods set
    const modIdsToUpdate = new Set(modsToUpdate.map(m => m.modId));
    setUpdatingMods(modIdsToUpdate);
    setDownloadedMods(new Set());
    
    try {
      // Call Tauri command - events will update UI in real-time
      const updated = await invoke<BaseMod[]>("update_mods", {
        mods: modsToUpdate,
        backupMods: settings.backupMods || false,
        backupDirectory: settings.backupDirectory || undefined
      });
      
      console.log(`[UPDATE] Received ${updated.length} updated mod(s) from Rust backend`);
      
      // Separate successful and failed updates
      const successfulModIds = new Set(
        updated.filter((u: BaseMod) => u.updated === true).map((u: BaseMod) => u.modId)
      );
      const failedMods = updated.filter((u: BaseMod) => u.updated === false);
      
      // Remove only successfully updated mods (they were removed by events, but this is a safety net)
      setMods(prev => prev.filter(m => !successfulModIds.has(m.modId)));
      
      // Keep failed mods in the list - they should already have error state set by events
      // But ensure they're still in the list if events didn't handle them
      if (failedMods.length > 0) {
        setMods(prev => {
          const existingModIds = new Set(prev.map(m => m.modId));
          const modsToAdd = failedMods.filter(m => !existingModIds.has(m.modId));
          if (modsToAdd.length > 0) {
            return [...prev, ...modsToAdd];
          }
          return prev;
        });
      }
      
      if (updated.length === 0 && modIdsToUpdate.size > 0) {
        setError('No mods were updated. Check backend logs for details.');
      } else {
        setError(null);
      }
    } catch (error) {
      console.error("Failed to update mods:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error updating mods: ${errorMessage}`);
    } finally {
      setIsUpdating(false);
      setUpdatingMods(new Set());
      setDownloadedMods(new Set());
    }
  };

  const removeMods = (modsToRemove: BaseMod[]) => {
    setMods(prev => prev.filter(m => !modsToRemove.includes(m)));
  };

  const ignoreFromList = (modsToIgnore: BaseMod[]) => {
    // Simply remove from current list (same as removeMods)
    setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
  };

  const ignoreThisUpdate = async (modsToIgnore: BaseMod[]) => {
    try {
      // Call Tauri command to update .lastupdated file with current remote timestamp
      await invoke("ignore_update", {
        mods: modsToIgnore
      });

      // Remove from list after successful ignore
      setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
    } catch (error) {
      console.error("Failed to ignore update:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error ignoring update: ${errorMessage}`);
    }
  };

  const ignorePermanently = async (modsToIgnore: BaseMod[]) => {
    try {
      // Add mod IDs and titles to ignoredMods in settings
      const currentIgnored = settings.ignoredMods || [];
      const existingModIds = new Set(
        currentIgnored.map(mod => typeof mod === 'string' ? mod : mod.modId)
      );
      
      const newModsToAdd = modsToIgnore
        .filter(m => !existingModIds.has(m.modId))
        .map(m => ({
          modId: m.modId,
          title: m.details?.title || m.folder || m.modId
        }));
      
      // Migrate old format (string[]) to new format (IgnoredMod[])
      const migratedIgnored = currentIgnored.map(mod => 
        typeof mod === 'string' ? { modId: mod, title: mod } : mod
      );
      
      const newIgnored = [...migratedIgnored, ...newModsToAdd];
      
      await updateSetting("ignoredMods", newIgnored);
      
      // Remove from list after successful ignore
      setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
    } catch (error) {
      console.error("Failed to ignore permanently:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error ignoring permanently: ${errorMessage}`);
    }
  };

  return (
    <ModsContext.Provider
      value={{
        mods,
        setMods,
        isQuerying,
        isUpdating,
        error,
        updatingMods,
        downloadedMods,
        modStates,
        modErrors,
        hasQueried,
        queryMods,
        updateMods,
        removeMods,
        ignoreFromList,
        ignoreThisUpdate,
        ignorePermanently,
      }}
    >
      {children}
    </ModsContext.Provider>
  );
}

export function useMods() {
  const context = useContext(ModsContext);
  if (context === undefined) {
    throw new Error("useMods must be used within a ModsProvider");
  }
  return context;
}

