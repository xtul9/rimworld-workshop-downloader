import { createContext, useContext, useState, ReactNode, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BaseMod } from "../types";
import { useSettings } from "./SettingsContext";

// Simplified state system: each mod has a single state that is managed ONLY by events
export type ModState = "queued" | "retry-queued" | "downloading" | "installing" | "completed" | "failed" | "cancelled" | null;

interface ModsContextType {
  mods: BaseMod[];
  setMods: (mods: BaseMod[] | ((prev: BaseMod[]) => BaseMod[])) => void;
  isQuerying: boolean;
  error: string | null;
  // Single source of truth: modStates contains all state information
  modStates: Map<string, ModState>;
  modErrors: Map<string, string>;
  hasQueried: boolean;
  // Computed: isUpdating is true if any mod has an active state
  isUpdating: boolean;
  // Computed: isModUpdating returns true if specific mod is updating
  isModUpdating: (modId: string) => boolean;
  queryMods: (modsPath: string) => Promise<void>;
  updateMods: (modsToUpdate: BaseMod[]) => Promise<void>;
  cancelUpdateMods: () => Promise<void>;
  removeMods: (modsToRemove: BaseMod[]) => void;
  ignoreFromList: (modsToIgnore: BaseMod[]) => void;
  ignoreThisUpdate: (modsToIgnore: BaseMod[]) => Promise<void>;
  ignorePermanently: (modsToIgnore: BaseMod[]) => Promise<void>;
}

const ModsContext = createContext<ModsContextType | undefined>(undefined);

export function ModsProvider({ children }: { children: ReactNode }) {
  const [mods, setMods] = useState<BaseMod[]>([]);
  const [isQuerying, setIsQuerying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [modStates, setModStates] = useState<Map<string, ModState>>(new Map());
  const [modErrors, setModErrors] = useState<Map<string, string>>(new Map());
  const [hasQueried, setHasQueried] = useState(false);
  const { updateSetting, settings } = useSettings();
  
  // Computed: isUpdating is true if any mod has an active state (not null, not "completed", not "failed", not "cancelled")
  const isUpdating = useMemo(() => {
    for (const state of modStates.values()) {
      if (state !== null && state !== "completed" && state !== "failed" && state !== "cancelled") {
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
              state === "installing" || state === "failed" || state === "cancelled") {
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
            const stateToCheck = currentState as ModState;
            if (stateToCheck !== undefined && stateToCheck !== "retry-queued") {
              newMap.set(modId, "failed");
            }
          }
          return newMap;
        });
        
        if (success) {
          // Remove mod from list immediately when updated (Query tab behavior)
          setMods(prev => prev.filter(m => m.modId !== modId));
          // Clear any errors
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
      const ignoredModIds = ignoredMods.map(mod => typeof mod === 'string' ? mod : mod.modId).filter(Boolean);
      
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
        backupDirectory: settings.backupDirectory || undefined,
        maxSteamcmdInstances: settings.maxSteamcmdInstances || 1
      });
      
      console.log(`[UPDATE] Received ${updated.length} updated mod(s) from Rust backend`);
      
      // Filter out successfully updated mods from the list
      // Events have already updated modStates, so we just need to clean up the list
      const successfullyUpdatedModIds = new Set(
        updated.filter((u: BaseMod) => u.updated === true).map((u: BaseMod) => u.modId)
      );
      setMods(prev => prev.filter(m => !successfullyUpdatedModIds.has(m.modId)));
      
      // If there were mods to update but none were successfully updated, set a global error
      if (successfullyUpdatedModIds.size === 0 && modsToUpdate.length > 0) {
        setError('No mods were successfully updated. Check individual mod statuses for details.');
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
    setMods(prev => prev.filter(m => !modsToRemove.includes(m)));
  };

  const ignoreFromList = (modsToIgnore: BaseMod[]) => {
    setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
  };

  const ignoreThisUpdate = async (modsToIgnore: BaseMod[]) => {
    try {
      await invoke("ignore_update", {
        mods: modsToIgnore
      });
      setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
    } catch (error) {
      console.error("Failed to ignore update:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error ignoring update: ${errorMessage}`);
    }
  };

  const ignorePermanently = async (modsToIgnore: BaseMod[]) => {
    try {
      const ignoredModIds = modsToIgnore.map(mod => mod.modId);
      const currentIgnored = settings.ignoredMods || [];
      // Handle both old format (string[]) and new format (IgnoredMod[])
      const currentIgnoredIds = currentIgnored.map((item: any) => typeof item === 'string' ? item : item.modId);
      const newIgnored = [...currentIgnoredIds, ...ignoredModIds];
      await updateSetting("ignoredMods", newIgnored as any);
      setMods(prev => prev.filter(m => !modsToIgnore.some(ignored => ignored.modId === m.modId)));
    } catch (error) {
      console.error("Failed to ignore permanently:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error ignoring permanently: ${errorMessage}`);
    }
  };

  const cancelUpdateMods = async () => {
    try {
      await invoke("cancel_update_mods");
      console.log("[UPDATE] Cancellation requested");
    } catch (error) {
      console.error("Failed to cancel update:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error cancelling update: ${errorMessage}`);
    }
  };

  return (
    <ModsContext.Provider
      value={{
        mods,
        setMods,
        isQuerying,
        error,
        modStates,
        modErrors,
        hasQueried,
        isUpdating,
        isModUpdating,
        queryMods,
        updateMods,
        cancelUpdateMods,
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
