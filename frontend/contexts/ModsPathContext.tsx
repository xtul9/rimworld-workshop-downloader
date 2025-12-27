import { createContext, useContext, ReactNode } from "react";
import { useSettings } from "./SettingsContext";

interface ModsPathContextType {
  modsPath: string;
  setModsPath: (path: string) => Promise<void>;
  selectModsPath: () => Promise<void>;
  error: string;
}

const ModsPathContext = createContext<ModsPathContextType | undefined>(undefined);

export function ModsPathProvider({ children }: { children: ReactNode }) {
  const { settings, updateSetting } = useSettings();

  const modsPath = settings.modsPath || "";
  const setModsPath = async (path: string) => {
    await updateSetting("modsPath", path);
  };

  const selectModsPath = async () => {
    // This function is kept for backward compatibility but should not be used
    // Settings should be managed through SettingsTab
    console.warn("selectModsPath should be called from SettingsTab");
  };

  return (
    <ModsPathContext.Provider value={{ modsPath, setModsPath, selectModsPath, error: "" }}>
      {children}
    </ModsPathContext.Provider>
  );
}

export function useModsPath() {
  const context = useContext(ModsPathContext);
  if (context === undefined) {
    throw new Error("useModsPath must be used within a ModsPathProvider");
  }
  return context;
}

