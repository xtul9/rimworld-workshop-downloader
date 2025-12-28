import { Store } from "@tauri-apps/plugin-store";

const SETTINGS_STORE_NAME = "settings.json";
const SETTINGS_KEY = "app-settings";

export type Theme = "light" | "dark" | "system";

export interface IgnoredMod {
  modId: string;
  title: string;
}

export interface AppSettings {
  modsPath: string;
  backupMods: boolean;
  backupDirectory: string; // Directory where backups are stored
  theme: Theme;
  isFirstRun: boolean;
  ignoredMods: IgnoredMod[]; // Array of mod IDs and titles that should be permanently ignored
  installedModsSortBy?: "date" | "name"; // Sort preference for installed mods tab
  installedModsSortOrder?: "desc" | "asc"; // Sort order preference for installed mods tab
  maxSteamcmdInstances?: number; // Maximum number of parallel SteamCMD instances (default: 4)
  // Add more settings here in the future
  [key: string]: any;
}

const defaultSettings: AppSettings = {
  modsPath: "",
  backupMods: false,
  backupDirectory: "",
  theme: "system",
  isFirstRun: true,
  ignoredMods: [],
  installedModsSortBy: "date",
  installedModsSortOrder: "desc",
  maxSteamcmdInstances: 1,
};

// Initialize store instance
let storeInstance: Store | null = null;
let storeInitializing: Promise<Store> | null = null;

async function getStore(): Promise<Store> {
  // If store is already initialized, return it
  if (storeInstance) {
    return storeInstance;
  }
  
  // If store is being initialized, wait for it
  if (storeInitializing) {
    return await storeInitializing;
  }
  
  // Start initialization
  storeInitializing = (async () => {
    try {
      const { Store } = await import("@tauri-apps/plugin-store");
      console.log("Loading Tauri Store with name:", SETTINGS_STORE_NAME);
      const store = await Store.load(SETTINGS_STORE_NAME);
      console.log("Tauri Store loaded successfully");
      
      // Try to get the store path for debugging
      try {
        const { appDataDir } = await import("@tauri-apps/api/path");
        const dataDir = await appDataDir();
        console.log("Store data directory:", dataDir);
      } catch (pathError) {
        console.warn("Could not get app data directory:", pathError);
      }
      
      storeInstance = store;
      storeInitializing = null;
      return store;
    } catch (error) {
      console.error("Failed to load Tauri Store:", error);
      if (error instanceof Error) {
        console.error("Store load error details:", error.message, error.stack);
      }
      storeInitializing = null;
      throw error;
    }
  })();
  
  return await storeInitializing;
}

export const settingsStorage = {
  async get(): Promise<AppSettings> {
    try {
      const store = await getStore();
      const stored = await store.get<AppSettings>(SETTINGS_KEY);
      
      if (stored) {
        console.log("Settings loaded from store:", stored);
        // Merge with defaults to ensure all settings exist
        return { ...defaultSettings, ...stored };
      } else {
        console.log("No settings found in store, using defaults");
      }
    } catch (error) {
      console.error("Failed to load settings:", error);
      if (error instanceof Error) {
        console.error("Error details:", error.message, error.stack);
      }
    }
    return { ...defaultSettings };
  },

  async set(settings: Partial<AppSettings>): Promise<void> {
    try {
      const store = await getStore();
      const current = await this.get();
      const updated = { ...current, ...settings };
      console.log("[settingsStorage] Saving settings:", updated);
      
      // Set the value in store
      await store.set(SETTINGS_KEY, updated);
      console.log("[settingsStorage] Settings set in store");
      
      // Save the store - this is critical for persistence
      console.log("[settingsStorage] Calling store.save()...");
      await store.save();
      console.log("[settingsStorage] store.save() completed");
      
      // Wait a bit to ensure the save is flushed
      await new Promise(resolve => setTimeout(resolve, 100));
      
      // Verify the save worked by reading back
      console.log("[settingsStorage] Verifying save...");
      const verify = await store.get<AppSettings>(SETTINGS_KEY);
      if (verify) {
        console.log("[settingsStorage] Settings verified after save:", verify);
        // Double-check that the key we saved is actually there
        if (verify.modsPath !== updated.modsPath) {
          console.error("[settingsStorage] ERROR: modsPath mismatch! Expected:", updated.modsPath, "Got:", verify.modsPath);
          throw new Error("Settings verification failed: modsPath mismatch");
        }
      } else {
        console.error("[settingsStorage] ERROR: Settings not found after save!");
        throw new Error("Settings verification failed: settings not found after save");
      }
      
      console.log("[settingsStorage] Settings saved and verified successfully");
    } catch (error) {
      console.error("[settingsStorage] Failed to save settings:", error);
      if (error instanceof Error) {
        console.error("[settingsStorage] Error details:", error.message, error.stack);
      }
      // Re-throw the error so the caller knows the save failed
      throw error;
    }
  },

  async getSetting<K extends keyof AppSettings>(key: K): Promise<AppSettings[K]> {
    const settings = await this.get();
    return settings[key];
  },

  async setSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]): Promise<void> {
    await this.set({ [key]: value });
  },

  async reset(): Promise<void> {
    try {
      const store = await getStore();
      await store.delete(SETTINGS_KEY);
      await store.save();
    } catch (error) {
      console.error("Failed to reset settings:", error);
    }
  },
};

