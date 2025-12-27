import { createContext, useContext, useState, useEffect, ReactNode } from "react";
import { settingsStorage, AppSettings } from "../utils/settingsStorage";

interface SettingsContextType {
  settings: AppSettings;
  isLoading: boolean;
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>;
  updateSettings: (settings: Partial<AppSettings>) => Promise<void>;
  resetSettings: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings>({ ...defaultSettings });
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    // Load settings on mount
    const loadSettings = async () => {
      try {
        setIsLoading(true);
        const loadedSettings = await settingsStorage.get();
        setSettings(loadedSettings);
      } catch (error) {
        console.error("Failed to load settings:", error);
        setSettings({ ...defaultSettings });
      } finally {
        setIsLoading(false);
      }
    };
    
    loadSettings();
  }, []);

  const updateSetting = async <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    try {
      console.log(`[SettingsContext] Updating setting ${key} to:`, value);
      // If changing any setting other than isFirstRun itself, set isFirstRun to false
      if (key !== "isFirstRun" && settings.isFirstRun) {
        console.log("[SettingsContext] Also setting isFirstRun to false");
        await settingsStorage.set({ [key]: value, isFirstRun: false });
        setSettings((prev) => ({ ...prev, [key]: value, isFirstRun: false }));
        console.log("[SettingsContext] Setting updated successfully");
      } else {
        await settingsStorage.setSetting(key, value);
        setSettings((prev) => ({ ...prev, [key]: value }));
        console.log("[SettingsContext] Setting updated successfully");
      }
    } catch (error) {
      console.error("[SettingsContext] Failed to update setting:", error);
      if (error instanceof Error) {
        console.error("[SettingsContext] Error details:", error.message, error.stack);
      }
      // Re-throw to let the caller know it failed
      throw error;
    }
  };

  const updateSettings = async (newSettings: Partial<AppSettings>) => {
    try {
      // If changing any setting other than isFirstRun itself, set isFirstRun to false
      if (!("isFirstRun" in newSettings) && settings.isFirstRun) {
        const updatedSettings = { ...newSettings, isFirstRun: false };
        await settingsStorage.set(updatedSettings);
        setSettings((prev) => ({ ...prev, ...updatedSettings }));
      } else {
        await settingsStorage.set(newSettings);
        setSettings((prev) => ({ ...prev, ...newSettings }));
      }
    } catch (error) {
      console.error("Failed to update settings:", error);
    }
  };

  const resetSettings = async () => {
    try {
      await settingsStorage.reset();
      const defaultSettings = await settingsStorage.get();
      setSettings(defaultSettings);
    } catch (error) {
      console.error("Failed to reset settings:", error);
    }
  };

  return (
    <SettingsContext.Provider value={{ settings, isLoading, updateSetting, updateSettings, resetSettings }}>
      {children}
    </SettingsContext.Provider>
  );
}

// Helper to get default settings
const defaultSettings: AppSettings = {
  modsPath: "",
  backupMods: false,
  backupDirectory: "",
  theme: "system",
  isFirstRun: true,
  ignoredMods: [],
};

export function useSettings() {
  const context = useContext(SettingsContext);
  if (context === undefined) {
    throw new Error("useSettings must be used within a SettingsProvider");
  }
  return context;
}

