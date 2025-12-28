import { createContext, useContext, useState, ReactNode, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BaseMod } from "../types";
import { useSettings } from "./SettingsContext";
import { ModState } from "./ModsContext";

interface InstalledModsContextType {
  mods: BaseMod[];
  setMods: (mods: BaseMod[] | ((prev: BaseMod[]) => BaseMod[])) => void;
  isLoading: boolean;
  isUpdatingDetails: boolean;
  error: string | null;
  // Single source of truth: modStates contains all state information
  modStates: Map<string, ModState>;
  modErrors: Map<string, string>;
  hasLoaded: boolean;
  // Computed: isUpdating is true if any mod has an active state
  isUpdating: boolean;
  // Computed: isModUpdating returns true if specific mod is updating
  isModUpdating: (modId: string) => boolean;
  loadInstalledMods: (modsPath: string) => Promise<void>;
  updateMods: (modsToUpdate: BaseMod[]) => Promise<void>;
  removeMods: (modsToRemove: BaseMod[]) => void;
  ignoreFromList: (modsToIgnore: BaseMod[]) => void;
  ignoreThisUpdate: (modsToIgnore: BaseMod[]) => Promise<void>;
  ignorePermanently: (modsToIgnore: BaseMod[]) => Promise<void>;
}

const InstalledModsContext = createContext<InstalledModsContextType | undefined>(undefined);

export function InstalledModsProvider({ children }: { children: ReactNode }) {
  const [mods, setMods] = useState<BaseMod[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isUpdatingDetails, setIsUpdatingDetails] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [modStates, setModStates] = useState<Map<string, ModState>>(new Map());
  const [modErrors, setModErrors] = useState<Map<string, string>>(new Map());
  const [hasLoaded, setHasLoaded] = useState(false);
  const { updateSetting, settings } = useSettings();
  
  // Computed: isUpdating is true if any mod has an active state (not null, not "completed", not "failed")
  const isUpdating = useMemo(() => {
    for (const state of modStates.values()) {
      if (state !== null && state !== "completed" && state !== "failed") {
        return true;
      }
    }
    return false;
  }, [modStates]);
  
  // Computed: isModUpdating returns true if specific mod is updating
  const isModUpdating = (modId: string) => {
    const state = modStates.get(modId);
    return state !== null && state !== "completed" && state !== "failed";
  };

  const loadInstalledMods = async (modsPath: string) => {
    if (isLoading) return;
    
    // Validate modsPath before making request
    if (!modsPath || modsPath.trim().length === 0) {
      setError("Error loading mods: Mods folder path is not set. Please configure it in Settings.");
      setIsLoading(false);
      return;
    }
    
    setIsLoading(true);
    setError(null);
    setHasLoaded(false);
    
    try {
      // Call Tauri command to list all installed mods (fast version - returns immediately with local data)
      const mods = await invoke<BaseMod[]>("list_installed_mods", {
        modsPath: modsPath
      });
      
      console.log(`[INSTALLED_MODS] Received ${mods.length} mods from Rust backend (fast load)`);
      
      // Always update the mods list with new query results (may not have details yet)
      setMods(mods);
      setError(null);
      setHasLoaded(true);
      
      // Start updating details in background if there are mods without details
      if (mods.length > 0 && mods.some(m => !m.details)) {
        setIsUpdatingDetails(true);
        
        // Update details in background (non-blocking)
        invoke<BaseMod[]>("update_mod_details", { mods })
          .then((updatedMods) => {
            console.log(`[INSTALLED_MODS] Updated details for ${updatedMods.length} mods`);
            setMods(updatedMods);
            setIsUpdatingDetails(false);
          })
          .catch((error) => {
            console.error("Failed to update mod details:", error);
            // Don't set error here - mods are already loaded, just without details
            setIsUpdatingDetails(false);
          });
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error("Failed to load installed mods:", error);
      setError(`Error loading installed mods: ${errorMessage}`);
      setMods([]);
      setHasLoaded(false);
    } finally {
      setIsLoading(false);
    }
  };

  // Listen for real-time download and update events
  // Events are the ONLY source of truth for mod states
  useEffect(() => {
    let unlistenState: (() => void) | undefined;
    let unlistenUpdated: (() => void) | undefined;

    const setupListeners = async () => {
      // Listen for mod-state events - this is the PRIMARY event for all state changes
      unlistenState = await listen<{ modId: string; state: string; error?: string; retryAttempt?: number; maxRetries?: number }>("mod-state", (event) => {
        const { modId, state, error: eventError } = event.payload;
        console.log(`[EVENT] Mod state changed: ${modId} -> ${state}`);
        
        setModStates(prev => {
          const newMap = new Map(prev);
          // Map backend states to frontend states
          if (state === "queued" || state === "retry-queued" || state === "downloading" || 
              state === "installing" || state === "failed") {
            newMap.set(modId, state as ModState);
          } else if (state === "completed") {
            // Backend doesn't emit "completed" yet, but we'll handle it if it does
            newMap.set(modId, "completed");
          } else {
            // Unknown state or null - remove from map
            newMap.delete(modId);
          }
          return newMap;
        });
        
        // Handle error messages
        if (state === "failed") {
          const errorMessage = eventError || "Operation failed";
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

      // Listen for mod-updated events - this marks the end of installation
      unlistenUpdated = await listen<{ modId: string; success: boolean; error?: string }>("mod-updated", (event) => {
        const { modId, success, error } = event.payload;
        console.log(`[EVENT] Mod updated: ${modId}, success: ${success}`);
        
        // Only update state if mod is not in retry-queued state
        // This prevents mod-updated from overwriting retry-queued state
        setModStates(prev => {
          const currentState = prev.get(modId);
          // Don't overwrite retry-queued with failed - let mod-state events handle retry states
          if (currentState === "retry-queued") {
            console.log(`[EVENT] Mod ${modId} is in retry-queued state, ignoring mod-updated event`);
            return prev;
          }
          
          const newMap = new Map(prev);
          if (success) {
            // Mark as completed and remove from active states
            newMap.set(modId, "completed");
            // After a short delay, remove from map entirely (mod is done)
            setTimeout(() => {
              setModStates(prevMap => {
                const finalMap = new Map(prevMap);
                finalMap.delete(modId);
                return finalMap;
              });
            }, 1000);
          } else {
            // Only mark as failed if not in retry-queued state
            // Failed state should only be set by mod-state events after all retries are exhausted
            if (currentState !== "retry-queued") {
              newMap.set(modId, "failed");
            }
          }
          return newMap;
        });
        
        if (success) {
          // In Installed Mods tab, mods should stay in the list after update
          // Just clear errors - mod stays visible
          setModErrors(prev => {
            const newMap = new Map(prev);
            newMap.delete(modId);
            return newMap;
          });
        } else {
          // Handle error - but only if not in retry-queued state
          const currentState = modStates.get(modId);
          if (currentState !== "retry-queued") {
            console.error(`[EVENT] Mod update failed: ${modId}, error: ${error}`);
            // Store error for this mod
            setModErrors(prev => {
              const newMap = new Map(prev);
              newMap.set(modId, error || "Update failed");
              return newMap;
            });
          }
        }
      });
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenState?.();
      unlistenUpdated?.();
    };
  }, []);

  const updateMods = async (modsToUpdate: BaseMod[]) => {
    if (modsToUpdate.length === 0) return;
    
    setError(null);
    
    // Set initial states for all mods being updated
    // Events will update these states as progress happens
    setModStates(prev => {
      const newMap = new Map(prev);
      for (const mod of modsToUpdate) {
        // Only set state if mod is a Steam mod (non-Steam mods can't be updated)
        if (!mod.nonSteamMod) {
          newMap.set(mod.modId, "queued");
        }
      }
      return newMap;
    });
    
    try {
      // Call Tauri command - events will update UI in real-time
      const updated = await invoke<BaseMod[]>("update_mods", {
        mods: modsToUpdate,
        backupMods: settings.backupMods || false,
        backupDirectory: settings.backupDirectory || undefined
      });
      
      console.log(`[UPDATE] Received ${updated.length} updated mod(s) from Rust backend`);
      
      // In Installed Mods tab, mods should stay in the list after update
      // Events have already updated modStates, so we just need to handle errors
      
      if (updated.length === 0 && modsToUpdate.length > 0) {
        setError('No mods were updated. Check backend logs for details.');
      } else {
        setError(null);
      }
    } catch (error) {
      console.error("Failed to update mods:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error updating mods: ${errorMessage}`);
      
      // On error, clear states for all mods that were being updated
      setModStates(prev => {
        const newMap = new Map(prev);
        for (const mod of modsToUpdate) {
          newMap.delete(mod.modId);
        }
        return newMap;
      });
    }
  };

  const removeMods = (modsToRemove: BaseMod[]) => {
    setMods(prev => prev.filter(m => !modsToRemove.some(removed => removed.modId === m.modId)));
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
        currentIgnored.map((mod: any) => typeof mod === 'string' ? mod : mod.modId)
      );
      
      const newModsToAdd = modsToIgnore
        .filter(m => !existingModIds.has(m.modId))
        .map(m => ({
          modId: m.modId,
          title: m.details?.title || m.folder || m.modId
        }));
      
      // Migrate old format (string[]) to new format (IgnoredMod[])
      const migratedIgnored = currentIgnored.map((mod: any) => 
        typeof mod === 'string' ? { modId: mod, title: mod } : mod
      );
      
      const newIgnored = [...migratedIgnored, ...newModsToAdd];
      
      await updateSetting("ignoredMods", newIgnored as any);
      
      // In Installed Mods tab, mods should stay in the list even after being ignored
      // They will just be filtered out in Query & Update tab
    } catch (error) {
      console.error("Failed to ignore permanently:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error ignoring permanently: ${errorMessage}`);
    }
  };

  return (
    <InstalledModsContext.Provider
      value={{
        mods,
        setMods,
        isLoading,
        isUpdatingDetails,
        error,
        modStates,
        modErrors,
        hasLoaded,
        isUpdating,
        isModUpdating,
        loadInstalledMods,
        updateMods,
        removeMods,
        ignoreFromList,
        ignoreThisUpdate,
        ignorePermanently,
      }}
    >
      {children}
    </InstalledModsContext.Provider>
  );
}

export function useInstalledMods() {
  const context = useContext(InstalledModsContext);
  if (context === undefined) {
    throw new Error("useInstalledMods must be used within an InstalledModsProvider");
  }
  return context;
}
