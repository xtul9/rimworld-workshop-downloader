import { createContext, useContext, useState, ReactNode } from "react";
import { BaseMod } from "../types";
import { API_BASE_URL } from "../utils/api";
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
      // Create AbortController for timeout
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 300000); // 5 minutes timeout
      
      // Include ignoredMods in query (extract only IDs for backend)
      const ignoredMods = settings.ignoredMods || [];
      const ignoredModIds = ignoredMods.map(mod => typeof mod === 'string' ? mod : mod.modId).filter(Boolean); // Support both old format (string[]) and new format (IgnoredMod[])
      const ignoredModsParam = ignoredModIds.length > 0 ? `&ignoredMods=${ignoredModIds.join(',')}` : '';
      
      const response = await fetch(`${API_BASE_URL}/mod/query?modsPath=${encodeURIComponent(modsPath)}${ignoredModsParam}`, {
        signal: controller.signal
      });
      
      clearTimeout(timeoutId);
      
      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ error: response.statusText }));
        const errorMessage = errorData.error || errorData.message || `HTTP ${response.status}`;
        console.error("Error querying mods:", errorMessage);
        setError(`Error querying mods: ${errorMessage}`);
        setMods([]);
        setHasQueried(false);
        return;
      }

      // Parse response with better error handling
      let data;
      try {
        // Check if response has content
        const contentType = response.headers.get('content-type');
        if (!contentType || !contentType.includes('application/json')) {
          const text = await response.text();
          console.error(`[QUERY] Unexpected content type: ${contentType}, response: ${text.substring(0, 200)}`);
          setError(`Error querying mods: Unexpected response format`);
          setMods([]);
          setHasQueried(false);
          return;
        }
        
        const responseText = await response.text();
        console.log(`[QUERY] Response text length: ${responseText.length} characters`);
        
        if (!responseText || responseText.trim().length === 0) {
          console.error(`[QUERY] Empty response from server`);
          setError(`Error querying mods: Empty response from server`);
          setMods([]);
          setHasQueried(false);
          return;
        }
        
        data = JSON.parse(responseText);
        console.log(`[QUERY] Received ${data.mods?.length || 0} mods from backend`);
      } catch (parseError) {
        console.error("Error parsing response:", parseError);
        console.error("Parse error details:", {
          name: parseError instanceof Error ? parseError.name : 'Unknown',
          message: parseError instanceof Error ? parseError.message : String(parseError),
          stack: parseError instanceof Error ? parseError.stack : undefined
        });
        setError(`Error querying mods: Failed to parse response (${parseError instanceof Error ? parseError.message : String(parseError)})`);
        setMods([]);
        setHasQueried(false);
        return;
      }
      
      // Always update the mods list with new query results
      setMods(data.mods || []);
      setError(null);
      setHasQueried(true);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error("Failed to query mods:", error);
      console.error("Error details:", {
        message: errorMessage,
        type: error instanceof TypeError ? 'NetworkError' : 'Unknown',
        name: error instanceof Error ? error.name : 'Unknown',
        url: `${API_BASE_URL}/mod/query?modsPath=${encodeURIComponent(modsPath)}`
      });
      
      if (error instanceof Error && error.name === 'AbortError') {
        setError("Error querying mods: Request timeout (took too long)");
      } else if (error instanceof TypeError && (errorMessage.includes('Load failed') || errorMessage.includes('Failed to fetch') || errorMessage.includes('Connection refused'))) {
        // Network error - backend is not running or not accessible
        setError("Error querying mods: Cannot connect to backend server. Please make sure the backend is running on port 5000.");
      } else {
        setError(`Error querying mods: ${errorMessage}`);
      }
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
      const response = await fetch(`${API_BASE_URL}/mod/update`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ 
          mods: modsToUpdate,
          backupMods: settings.backupMods || false,
          backupDirectory: settings.backupDirectory || ""
        })
      });
      
      if (response.ok) {
        const updated = await response.json();
        console.log(`[UPDATE] Received ${updated.length} updated mod(s) from backend`);
        
        if (updated.length === 0) {
          setError('No mods were updated. Check backend logs for details.');
        } else {
          const updatedModIds = new Set(updated.map((u: BaseMod) => u.modId));
          
          // Remove successfully updated mods from the list
          setMods(prev => prev.filter(m => !updatedModIds.has(m.modId)));
          setError(null);
        }
      } else {
        const errorData = await response.json().catch(() => ({ error: response.statusText }));
        const errorMessage = errorData.error || errorData.message || `HTTP ${response.status}`;
        console.error(`[UPDATE] Error response:`, errorMessage);
        setError(`Error updating mods: ${errorMessage}`);
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
      // Call backend to update .lastupdated file with current remote timestamp
      const response = await fetch(`${API_BASE_URL}/mod/ignore-update`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mods: modsToIgnore })
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ error: response.statusText }));
        throw new Error(errorData.error || errorData.message || `HTTP ${response.status}`);
      }

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
          title: m.details?.title || m.modId
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

