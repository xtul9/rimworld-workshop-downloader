import { createContext, useContext, useState, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BaseMod } from "../types";
import { useSettings } from "./SettingsContext";

interface InstalledModsContextType {
  mods: BaseMod[];
  setMods: (mods: BaseMod[] | ((prev: BaseMod[]) => BaseMod[])) => void;
  isLoading: boolean;
  isUpdating: boolean;
  error: string | null;
  updatingMods: Set<string>;
  hasLoaded: boolean;
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
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatingMods, setUpdatingMods] = useState<Set<string>>(new Set());
  const [hasLoaded, setHasLoaded] = useState(false);
  const { updateSetting, settings } = useSettings();

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
      // Call Tauri command to list all installed mods
      const mods = await invoke<BaseMod[]>("list_installed_mods", {
        modsPath: modsPath
      });
      
      console.log(`[INSTALLED_MODS] Received ${mods.length} mods from Rust backend`);
      
      // Always update the mods list with new query results
      setMods(mods);
      setError(null);
      setHasLoaded(true);
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
        // Refresh the mods list after update
        // Reload installed mods to get fresh data
        const firstMod = modsToUpdate[0];
        if (firstMod.modPath) {
          const modsPath = firstMod.modPath.substring(0, firstMod.modPath.lastIndexOf('/'));
          await loadInstalledMods(modsPath);
        }
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
    <InstalledModsContext.Provider
      value={{
        mods,
        setMods,
        isLoading,
        isUpdating,
        error,
        updatingMods,
        hasLoaded,
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

