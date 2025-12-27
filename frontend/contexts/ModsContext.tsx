import { createContext, useContext, useState, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BaseMod } from "../types";
import { useSettings } from "./SettingsContext";

interface ModsContextType {
  mods: BaseMod[];
  setMods: (mods: BaseMod[] | ((prev: BaseMod[]) => BaseMod[])) => void;
  isQuerying: boolean;
  isUpdating: boolean;
  error: string | null;
  updatingMods: Set<string>;
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
  const [hasQueried, setHasQueried] = useState(false);
  const { updateSetting, settings } = useSettings();

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
    
    // Mark mods as updating
    const modIdsToUpdate = new Set(modsToUpdate.map(m => m.modId));
    setUpdatingMods(modIdsToUpdate);
    
    try {
      // Call Tauri command instead of fetch
      const updated = await invoke<BaseMod[]>("update_mods", {
        mods: modsToUpdate,
        backupMods: settings.backupMods || false,
        backupDirectory: settings.backupDirectory || undefined
      });
      
      console.log(`[UPDATE] Received ${updated.length} updated mod(s) from Rust backend`);
      
      if (updated.length === 0) {
        setError('No mods were updated. Check backend logs for details.');
      } else {
        const updatedModIds = new Set(updated.map((u: BaseMod) => u.modId));
        
        // Remove successfully updated mods from the list
        setMods(prev => prev.filter(m => !updatedModIds.has(m.modId)));
        setError(null);
      }
    } catch (error) {
      console.error("Failed to update mods:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setError(`Error updating mods: ${errorMessage}`);
    } finally {
      setIsUpdating(false);
      setUpdatingMods(new Set());
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

