import { useState, useCallback, useEffect, useRef } from "react";
import { FixedSizeList as List } from "react-window";
import { openUrl, revealItemInDir, openPath } from "@tauri-apps/plugin-opener";
import { BaseMod } from "../types";
import { useMods } from "../contexts/ModsContext";
import { useSettings } from "../contexts/SettingsContext";
import { useModal } from "../contexts/ModalContext";
import { useContextMenu, ContextMenuItem } from "../contexts/ContextMenuContext";
import { useFormatting } from "../hooks/useFormatting";
import { API_BASE_URL } from "../utils/api";
import "./ModList.css";

interface ModListProps {
  onUpdateSelected: (mods: BaseMod[]) => void;
  modsPath: string;
}

// Height of each mod item (including margin-bottom)
const ITEM_HEIGHT = 80; // 72px min-height + 8px margin-bottom

export default function ModList({ onUpdateSelected, modsPath }: ModListProps) {
  const { mods, isQuerying, error, updatingMods, hasQueried, ignoreFromList, ignoreThisUpdate, ignorePermanently } = useMods();
  const { settings } = useSettings();
  const { openModal } = useModal();
  const { showContextMenu } = useContextMenu();
  const { formatSize, formatDate } = useFormatting();
  const [selectedMods, setSelectedMods] = useState<Set<string>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number | null>(null);
  const [modBackups, setModBackups] = useState<Map<string, boolean>>(new Map());
  const [backupDates, setBackupDates] = useState<Map<string, Date>>(new Map());
  const [listHeight, setListHeight] = useState(600);
  const listContainerRef = useRef<HTMLDivElement>(null);

  // Update list height when container size changes
  useEffect(() => {
    const updateHeight = () => {
      if (listContainerRef.current) {
        const height = listContainerRef.current.clientHeight;
        if (height > 0) {
          setListHeight(height);
        }
      }
    };

    updateHeight();
    window.addEventListener("resize", updateHeight);
    return () => window.removeEventListener("resize", updateHeight);
  }, [mods.length]);

  // Check backup availability for mods - using useCallback to ensure it's available in other functions
  const checkBackups = useCallback(async () => {
    if (mods.length === 0 || !modsPath || !settings.backupDirectory) {
      // Clear backups if no backup directory is configured
      setModBackups(new Map());
      return;
    }

    const backupChecks = mods.map(async (mod) => {
      if (!mod.modPath) return null;
      
      try {
        // Use modPath and backupDirectory to check for backup
        const response = await fetch(
          `${API_BASE_URL}/mod/check-backup?modPath=${encodeURIComponent(mod.modPath)}&backupDirectory=${encodeURIComponent(settings.backupDirectory)}`
        );
        
        if (response.ok) {
          const data = await response.json();
          return { 
            modId: mod.modId, 
            hasBackup: data.hasBackup,
            backupDate: data.backupDate ? new Date(data.backupDate) : null
          };
        }
      } catch (error) {
        console.warn(`Failed to check backup for mod ${mod.modId}:`, error);
      }
      return null;
    });

    const results = await Promise.all(backupChecks);
    const backupMap = new Map<string, boolean>();
    const datesMap = new Map<string, Date>();
    results.forEach(result => {
      if (result) {
        backupMap.set(result.modId, result.hasBackup);
        if (result.backupDate) {
          datesMap.set(result.modId, result.backupDate);
        }
      }
    });
    setModBackups(backupMap);
    setBackupDates(datesMap);
  }, [mods, modsPath, settings.backupDirectory]);

  // Check backup availability for mods
  useEffect(() => {
    checkBackups();
  }, [checkBackups]);

  const handleSelectMod = useCallback((modId: string, ctrlKey: boolean, shiftKey: boolean, index: number) => {
    setSelectedMods(prev => {
      const newSet = new Set(prev);
      
      if (shiftKey && lastSelectedIndex !== null) {
        // Range selection with Shift
        const start = Math.min(lastSelectedIndex, index);
        const end = Math.max(lastSelectedIndex, index);
        
        // Add all mods in the range
        for (let i = start; i <= end; i++) {
          newSet.add(mods[i].modId);
        }
        setLastSelectedIndex(index);
      } else if (ctrlKey || shiftKey) {
        // Toggle single mod with Ctrl/Cmd or Shift (first click)
        if (newSet.has(modId)) {
          newSet.delete(modId);
        } else {
          newSet.add(modId);
        }
        setLastSelectedIndex(index);
      } else {
        // Single click without modifiers
        if (newSet.size === 1 && newSet.has(modId)) {
          // If only this mod is selected, deselect it
          newSet.clear();
          setLastSelectedIndex(null);
        } else {
          // Clear all and select only this one
          newSet.clear();
          newSet.add(modId);
          setLastSelectedIndex(index);
        }
      }
      
      return newSet;
    });
  }, [mods, lastSelectedIndex]);

  const handleContextMenu = useCallback((e: React.MouseEvent, mod: BaseMod) => {
    e.preventDefault();
    
    const selected = mods.filter(m => selectedMods.has(m.modId));
    const hasBackup = modBackups.get(mod.modId) || false;
    const canRestoreBackup = mod.modPath && settings.backupDirectory && hasBackup;
    
    const items: ContextMenuItem[] = [];
    
    if (selected.length > 1) {
      // Multiple mods selected
      items.push(
        { label: "Update selected mods", action: "update" },
        { separator: true },
        { label: "Hide for now", action: "ignore-from-list" },
        { label: "Ignore this update", action: "ignore-this-update" },
        { label: "Ignore mods completely", action: "ignore-permanently" }
      );
    } else {
      // Single mod
      items.push(
        { label: "Update", action: "update" },
        { 
          label: hasBackup ? "Restore Backup" : "No backup available", 
          action: "restore-backup",
          disabled: !canRestoreBackup
        },
        { separator: true },
        { label: "Open mod folder", action: "open-folder" },
        { label: "Open workshop page", action: "open-workshop" },
        { label: "Open changelog page", action: "open-changelog" },
        { separator: true },
        { label: "Hide for now", action: "ignore-from-list" },
        { label: "Ignore this update", action: "ignore-this-update" },
        { label: "Ignore mod completely", action: "ignore-permanently" }
      );
    }
    
    showContextMenu(
      { x: e.clientX, y: e.clientY },
      { mod, selected },
      items,
      handleContextAction
    );
  }, [mods, selectedMods, modBackups, settings.backupDirectory, showContextMenu]);

  const handleContextAction = useCallback(async (action: string, data: { mod: BaseMod; selected: BaseMod[] }) => {
    const { mod, selected } = data;

    switch (action) {
      case "update":
        if (selected.length > 1) {
          onUpdateSelected(selected);
        } else {
          onUpdateSelected([mod]);
        }
        break;
      case "restore-backup":
        if (!mod.modPath || !settings.backupDirectory) {
          openModal("restore-backup", {
            mod,
            error: "Cannot restore backup: mod path or backup directory not configured"
          });
          break;
        }
        
        if (!modBackups.get(mod.modId)) {
          openModal("restore-backup", {
            mod,
            error: "No backup available for this mod"
          });
          break;
        }
        
        // Open global restore backup modal
        openModal("restore-backup", {
          mod,
          backupDate: backupDates.get(mod.modId),
          onRestoreComplete: checkBackups
        });
        break;
      case "open-folder":
        // Open mod folder using Tauri
        if (mod.modPath) {
          console.log(`[ModList] Attempting to open folder: ${mod.modPath}`);
          // Try revealItemInDir first (shows folder in file manager)
          revealItemInDir(mod.modPath)
            .then(() => {
              console.log(`[ModList] Successfully opened folder: ${mod.modPath}`);
            })
            .catch((error) => {
              console.warn(`[ModList] revealItemInDir failed, trying openPath:`, error);
              // Fallback to openPath (opens folder with default app)
              openPath(mod.modPath)
                .then(() => {
                  console.log(`[ModList] Successfully opened folder with openPath: ${mod.modPath}`);
                })
                .catch((openError) => {
                  console.error("[ModList] Both revealItemInDir and openPath failed:", openError);
                  alert(`Failed to open folder: ${mod.modPath}\nError: ${openError instanceof Error ? openError.message : String(openError)}`);
                });
            });
        } else {
          console.warn("[ModList] Cannot open folder: modPath is not set");
          alert("Mod folder path is not available");
        }
        break;
      case "open-workshop":
        openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.modId}`).catch((error) => {
          console.error("Failed to open workshop page:", error);
          alert(`Failed to open workshop page for mod ${mod.modId}`);
        });
        break;
      case "open-changelog":
        openUrl(`https://steamcommunity.com/sharedfiles/filedetails/changelog/${mod.modId}`).catch((error) => {
          console.error("Failed to open changelog page:", error);
          alert(`Failed to open changelog page for mod ${mod.modId}`);
        });
        break;
      case "ignore-from-list":
        if (selected.length > 0) {
          ignoreFromList(selected);
        } else {
          ignoreFromList([mod]);
        }
        break;
      case "ignore-this-update":
        if (selected.length > 0) {
          await ignoreThisUpdate(selected);
        } else {
          await ignoreThisUpdate([mod]);
        }
        break;
      case "ignore-permanently":
        if (selected.length > 0) {
          await ignorePermanently(selected);
        } else {
          await ignorePermanently([mod]);
        }
        break;
    }
  }, [mods, modBackups, backupDates, settings, openModal, checkBackups, onUpdateSelected, ignoreFromList, ignoreThisUpdate, ignorePermanently]);


  return (
    <div className="mod-list-container">
      {isQuerying ? (
        <div className="mod-list-loading">
          <div className="loader-spinner"></div>
          <div className="loader-text">Querying mods for updates...</div>
        </div>
      ) : error ? (
        <div className="mod-list-error">
          <div className="error-icon">⚠️</div>
          <div className="error-text">{error}</div>
        </div>
      ) : mods.length === 0 ? (
        <div className="mod-list-empty">
          {hasQueried ? (
            "All mods are up to date"
          ) : (
            <div className="mod-list-empty-content">
              <div className="mod-list-empty-icon">📋</div>
              <div className="mod-list-empty-title">No mods queried yet</div>
              <div className="mod-list-empty-message">
                {!modsPath ? (
                  <>
                    Please set the mods folder path in the <strong>Settings</strong> tab first.
                  </>
                ) : (
                  <>
                    Click the <strong>"Query Mods"</strong> button above to check for mod updates.
                  </>
                )}
              </div>
            </div>
          )}
        </div>
      ) : (
        <div 
          className="mod-list"
          ref={listContainerRef}
        >
          <List
            height={listHeight}
            itemCount={mods.length}
            itemSize={ITEM_HEIGHT}
            width="100%"
            style={{ paddingTop: "10px" }}
            innerElementType="div"
          >
            {({ index, style }) => {
              const mod = mods[index];
              const isUpdating = updatingMods.has(mod.modId);
              
              return (
                <div
                  style={{
                    ...style,
                    height: `${parseInt(style.height as string) - 8}px`,
                    marginBottom: "8px",
                  }}
                  className={`mod-item ${selectedMods.has(mod.modId) ? "selected" : ""} ${mod.updated ? "updated" : ""} ${isUpdating ? "updating" : ""}`}
                  onClick={(e) => !isUpdating && handleSelectMod(mod.modId, e.ctrlKey || e.metaKey, e.shiftKey, index)}
                  onContextMenu={(e) => !isUpdating && handleContextMenu(e, mod)}
                >
                  {isUpdating ? (
                    <div className="mod-item-updating">
                      <div className="mod-updating-spinner"></div>
                      <div className="mod-updating-text">Update in progress...</div>
                      <div className="mod-updating-name">{mod.details?.title || mod.modId}</div>
                    </div>
                  ) : (
                    <>
                      <div className="mod-item-header">
                        <span className="mod-name">{mod.details?.title || mod.modId}</span>
                        {mod.updated && <span className="mod-updated-badge">Updated</span>}
                      </div>
                      <div className="mod-item-details">
                        <div className="mod-detail">
                          <span className="mod-detail-label">ID:</span>
                          <span className="mod-detail-value">{mod.modId}</span>
                        </div>
                        <div className="mod-detail">
                          <span className="mod-detail-label">Folder:</span>
                          <span className="mod-detail-value">{mod.folder || mod.modPath}</span>
                        </div>
                        {mod.details && (
                          <>
                            <div className="mod-detail">
                              <span className="mod-detail-label">Size:</span>
                              <span className="mod-detail-value">{formatSize(mod.details.file_size)}</span>
                            </div>
                            <div className="mod-detail">
                              <span className="mod-detail-label">Updated:</span>
                              <span className="mod-detail-value">{formatDate(mod.details.time_updated)}</span>
                            </div>
                          </>
                        )}
                      </div>
                    </>
                  )}
                </div>
              );
            }}
          </List>
        </div>
      )}

    </div>
  );
}

