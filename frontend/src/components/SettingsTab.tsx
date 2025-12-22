import { useState, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSettings } from "../contexts/SettingsContext";
import SettingField from "./SettingField";
import ThemeSelect from "./ThemeSelect";
import "./SettingsTab.css";

export default function SettingsTab() {
  const { settings, updateSetting } = useSettings();
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  
  // Local state for input fields to avoid saving on every keystroke
  const [localModsPath, setLocalModsPath] = useState(settings.modsPath);
  const [localBackupDirectory, setLocalBackupDirectory] = useState(settings.backupDirectory || "");
  
  // Sync local state with settings when they change externally
  useEffect(() => {
    setLocalModsPath(settings.modsPath);
  }, [settings.modsPath]);
  
  useEffect(() => {
    setLocalBackupDirectory(settings.backupDirectory || "");
  }, [settings.backupDirectory]);
  
  // Debounce timers
  const modsPathDebounceRef = useRef<number | null>(null);
  const backupDirectoryDebounceRef = useRef<number | null>(null);

  const handleSelectModsPath = async () => {
    try {
      setError("");
      setSuccess("");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select mods folder"
      });
      
      if (selected === null || typeof selected !== "string") {
        return;
      }
      
      const path: string = selected;
      
      if (path) {
        try {
          await updateSetting("modsPath", path);
          setSuccess("Mods folder path saved successfully!");
          setTimeout(() => setSuccess(""), 3000);
        } catch (err) {
          console.error("Failed to save mods path:", err);
          setError(`Failed to save mods folder path: ${err instanceof Error ? err.message : String(err)}`);
          setTimeout(() => setError(""), 5000);
        }
      }
    } catch (err) {
      console.error("Failed to select mods path:", err);
      setError(`Failed to select mods folder: ${err instanceof Error ? err.message : String(err)}`);
      setTimeout(() => setError(""), 5000);
    }
  };

  const handleModsPathChange = (value: string) => {
    // Update local state immediately for responsive UI
    setLocalModsPath(value);
    setError("");
    setSuccess("");
    
    // Clear existing debounce timer
    if (modsPathDebounceRef.current) {
      clearTimeout(modsPathDebounceRef.current);
    }
    
    // Set new debounce timer - save after 500ms of no typing
    modsPathDebounceRef.current = setTimeout(async () => {
      try {
        await updateSetting("modsPath", value);
      } catch (err) {
        console.error("Failed to save mods path:", err);
        setError(`Failed to save mods folder path: ${err instanceof Error ? err.message : String(err)}`);
        setTimeout(() => setError(""), 5000);
      }
    }, 500);
  };
  
  const handleModsPathBlur = async () => {
    // Save immediately when user leaves the field
    if (modsPathDebounceRef.current) {
      clearTimeout(modsPathDebounceRef.current);
      modsPathDebounceRef.current = null;
    }
    
    try {
      await updateSetting("modsPath", localModsPath);
    } catch (err) {
      console.error("Failed to save mods path:", err);
      setError(`Failed to save mods folder path: ${err instanceof Error ? err.message : String(err)}`);
      setTimeout(() => setError(""), 5000);
    }
  };

  const handleSelectBackupDirectory = async () => {
    try {
      setError("");
      setSuccess("");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select backup directory"
      });
      
      if (selected === null || typeof selected !== "string") {
        return;
      }
      
      const path: string = selected;
      
      if (path) {
        await updateSetting("backupDirectory", path);
        setSuccess("Backup directory path saved successfully!");
        setTimeout(() => setSuccess(""), 3000);
      }
    } catch (err) {
      console.error("Failed to select backup directory:", err);
      setError(`Failed to select backup directory: ${err instanceof Error ? err.message : String(err)}`);
      setTimeout(() => setError(""), 5000);
    }
  };

  const handleBackupDirectoryChange = (value: string) => {
    // Update local state immediately for responsive UI
    setLocalBackupDirectory(value);
    setError("");
    setSuccess("");
    
    // Clear existing debounce timer
    if (backupDirectoryDebounceRef.current) {
      clearTimeout(backupDirectoryDebounceRef.current);
    }
    
    // Set new debounce timer - save after 500ms of no typing
    backupDirectoryDebounceRef.current = setTimeout(async () => {
      try {
        await updateSetting("backupDirectory", value);
      } catch (err) {
        console.error("Failed to save backup directory:", err);
        setError(`Failed to save backup directory: ${err instanceof Error ? err.message : String(err)}`);
        setTimeout(() => setError(""), 5000);
      }
    }, 500);
  };
  
  const handleBackupDirectoryBlur = async () => {
    // Save immediately when user leaves the field
    if (backupDirectoryDebounceRef.current) {
      clearTimeout(backupDirectoryDebounceRef.current);
      backupDirectoryDebounceRef.current = null;
    }
    
    try {
      await updateSetting("backupDirectory", localBackupDirectory);
    } catch (err) {
      console.error("Failed to save backup directory:", err);
      setError(`Failed to save backup directory: ${err instanceof Error ? err.message : String(err)}`);
      setTimeout(() => setError(""), 5000);
    }
  };
  
  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      if (modsPathDebounceRef.current) {
        clearTimeout(modsPathDebounceRef.current);
      }
      if (backupDirectoryDebounceRef.current) {
        clearTimeout(backupDirectoryDebounceRef.current);
      }
    };
  }, []);

  return (
    <div className="settings-tab">
      <div className="settings-content">
        <h2 className="settings-title">Settings</h2>
        
        <SettingField
          title="Mods Folder"
          description="Set the path to your Rimworld mods folder. This path will be used for querying and updating mods."
          error={error}
          success={success}
        >
          <label htmlFor="mods-path" className="settings-label">
            Mods Folder Path
          </label>
          <div className="settings-input-group">
            <input
              id="mods-path"
              type="text"
              className="settings-input"
              value={localModsPath}
              onChange={(e) => handleModsPathChange(e.target.value)}
              onBlur={handleModsPathBlur}
              placeholder="C:/Games/Rimworld/Mods"
            />
            <button
              onClick={handleSelectModsPath}
              className="settings-browse-button"
              title="Browse for mods folder"
            >
              Browse
            </button>
          </div>
        </SettingField>

        <SettingField
          title="Backup Mods Before Updating"
          description="When enabled, a backup copy of each mod will be created before updating it. This helps you restore mods if something goes wrong during the update process."
        >
          <label className="settings-checkbox-label">
            <input
              id="backup-mods"
              type="checkbox"
              className="settings-checkbox"
              checked={settings.backupMods}
              onChange={async (e) => await updateSetting("backupMods", e.target.checked)}
            />
            <span>Enable backup before updating</span>
          </label>
        </SettingField>

        <SettingField
          title="Backup Directory"
          description="Directory where mod backups will be stored. Backups are stored with the same folder name as the mod, so they won't interfere with the game. This directory should be different from your mods folder."
        >
          <label htmlFor="backup-directory" className="settings-label">
            Backup Directory Path
          </label>
          <div className="settings-input-group">
            <input
              id="backup-directory"
              type="text"
              className="settings-input"
              value={localBackupDirectory}
              onChange={(e) => handleBackupDirectoryChange(e.target.value)}
              onBlur={handleBackupDirectoryBlur}
              placeholder="C:/Games/Rimworld/ModsBackup"
            />
            <button
              onClick={handleSelectBackupDirectory}
              className="settings-browse-button"
              title="Browse for backup directory"
            >
              Browse
            </button>
          </div>
        </SettingField>

        <SettingField
          title="Theme"
          description="Choose your preferred color theme. 'System' will follow your operating system's theme preference."
        >
          <label htmlFor="theme-select" className="settings-label">
            Color Theme
          </label>
          <ThemeSelect
            id="theme-select"
            value={settings.theme}
            onChange={async (value) => await updateSetting("theme", value)}
          />
        </SettingField>

        <SettingField
          title="Ignored Mods"
          description="Mods in this list will be permanently ignored and won't appear in update queries. You can remove mods from this list to start checking for updates again."
        >
          <div className="ignored-mods-list">
            {settings.ignoredMods && settings.ignoredMods.length > 0 ? (
              <div className="ignored-mods-items">
                {settings.ignoredMods.map((mod) => {
                  // Support both old format (string) and new format (IgnoredMod)
                  const modId = typeof mod === 'string' ? mod : mod.modId;
                  const title = typeof mod === 'string' ? mod : mod.title;
                  
                  return (
                    <div key={modId} className="ignored-mod-item">
                      <div className="ignored-mod-info">
                        <span className="ignored-mod-title">{title}</span>
                        <span className="ignored-mod-id">ID: {modId}</span>
                      </div>
                      <div className="ignored-mod-actions">
                        <button
                          className="ignored-mod-link"
                          onClick={() => openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${modId}`).catch(console.error)}
                          title="Open workshop page"
                        >
                          🔗
                        </button>
                        <button
                          className="ignored-mod-remove"
                          onClick={async () => {
                            const newIgnored = settings.ignoredMods.filter(m => {
                              const mId = typeof m === 'string' ? m : m.modId;
                              return mId !== modId;
                            });
                            await updateSetting("ignoredMods", newIgnored);
                          }}
                          title="Remove from ignored list"
                        >
                          ✕
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="ignored-mods-empty">No mods are currently ignored.</div>
            )}
          </div>
        </SettingField>

        {/* Future settings sections can be added here */}
      </div>
    </div>
  );
}

