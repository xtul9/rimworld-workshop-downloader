import { useState, useRef, useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { BaseMod } from "../types";
import { useModsPath } from "../contexts/ModsPathContext";
import { useSettings } from "../contexts/SettingsContext";
import { useAccessError } from "../contexts/AccessErrorContext";
import { useModal } from "../contexts/ModalContext";
import { useFormatting } from "../hooks/useFormatting";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./DownloadTab.css";

interface ModInput {
  id: string;
  value: string;
  modId?: string;
  title?: string;
  size?: number;
  isCollection?: boolean;
  collectionMods?: any[];
  status: "empty" | "loading" | "ready" | "downloading" | "completed" | "error" | "cancelled";
  error?: string;
}

export default function DownloadTab() {
  const { modsPath } = useModsPath();
  const { settings } = useSettings();
  const { permissions } = useAccessError();
  const { openModal } = useModal();
  const { formatSize } = useFormatting();
  const [modInputs, setModInputs] = useState<ModInput[]>([
    { id: "1", value: "", status: "empty" }
  ]);
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadedMods, setDownloadedMods] = useState<BaseMod[]>([]);
  const [progress, setProgress] = useState(0);
  const [progressMax, setProgressMax] = useState(0);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [showImportModal, setShowImportModal] = useState(false);
  const [importText, setImportText] = useState("");
  const inputRefs = useRef<{ [key: string]: HTMLInputElement | null }>({});

  // Listen for real-time download events from backend
  useEffect(() => {
    let unlistenState: (() => void) | undefined;
    let unlistenUpdated: (() => void) | undefined;

    const setupListeners = async () => {
      // Listen for mod-state events - this is the PRIMARY event for all state changes
      unlistenState = await listen<{ modId: string; state: string; error?: string; retryAttempt?: number; maxRetries?: number }>("mod-state", (event) => {
        const { modId, state, error: eventError } = event.payload;
        console.log(`[DOWNLOAD_TAB] Mod state changed: ${modId} -> ${state}`);
        
        // Update mod input status based on backend events
        setModInputs(prev => {
          return prev.map(input => {
            // Find mod by modId
            if (input.modId === modId) {
              // Map backend states to frontend states
              if (state === "queued" || state === "retry-queued") {
                return { ...input, status: "downloading" as const };
              } else if (state === "downloading" || state === "installing") {
                return { ...input, status: "downloading" as const };
              } else if (state === "completed") {
                return { ...input, status: "completed" as const };
              } else if (state === "failed") {
                // Only mark as error if not already cancelled
                if (input.status !== "cancelled") {
                  return { 
                    ...input, 
                    status: "error" as const,
                    error: eventError || "Download failed"
                  };
                }
              } else if (state === "cancelled") {
                return { ...input, status: "cancelled" as const };
              }
            }
            return input;
          });
        });
      });

      // Listen for mod-updated events - this marks the end of installation
      unlistenUpdated = await listen<{ modId: string; success: boolean; error?: string }>("mod-updated", (event) => {
        const { modId, success, error } = event.payload;
        console.log(`[DOWNLOAD_TAB] Mod updated: ${modId}, success: ${success}`);
        
        setModInputs(prev => {
          return prev.map(input => {
            if (input.modId === modId) {
              if (success) {
                return { ...input, status: "completed" as const };
              } else {
                // Only mark as error if not already cancelled
                if (input.status !== "cancelled") {
                  return { 
                    ...input, 
                    status: "error" as const,
                    error: error || "Download failed"
                  };
                }
              }
            }
            return input;
          });
        });
      });
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenState?.();
      unlistenUpdated?.();
    };
  }, []);

  const extractModId = (text: string): string | null => {
    // Try to extract ID from URL (both /sharedfiles/ and /workshop/ paths)
    const urlMatch = text.match(/steamcommunity\.com\/(?:sharedfiles|workshop)\/filedetails\/\?id=(\d+)/);
    if (urlMatch) {
      return urlMatch[1];
    }
    
    // Try to extract ID from short URL
    const shortMatch = text.match(/id=(\d+)/);
    if (shortMatch) {
      return shortMatch[1];
    }
    
    // Check if it's just a number (mod ID)
    const numberMatch = text.match(/^(\d+)$/);
    if (numberMatch) {
      return numberMatch[1];
    }
    
    return null;
  };

  const handleInputChange = async (inputId: string, value: string) => {
    // Clear download history when user starts adding new mods (new download session)
    if (value.trim()) {
      setDownloadedMods([]);
      setProgress(0);
      setProgressMax(0);
      setDownloadStatus("");
    }

    setModInputs(prev => 
      prev.map(input => 
        input.id === inputId 
          ? { ...input, value, status: value.trim() ? ("loading" as const) : ("empty" as const) }
          : input
      )
    );

    // If input is not empty, fetch mod details
    if (value.trim()) {
      const modId = extractModId(value.trim());
      
      if (!modId) {
        setModInputs(prev => 
          prev.map(input => 
            input.id === inputId 
              ? { ...input, status: "error" as const, error: "Invalid URL/ID format" }
              : input
          )
        );
        return;
      }

      // Check for duplicates (but allow updating the same input)
      // Check duplicates using current state
      const isDuplicate = modInputs.some(input => 
        input.modId === modId && input.id !== inputId
      );

      if (isDuplicate) {
        setModInputs(prev => 
          prev.map(input => 
            input.id === inputId 
              ? { ...input, status: "error" as const, error: "This mod is already in the list" }
              : input
          )
        );
        return;
      }

      if (isDuplicate) {
        setModInputs(prev => 
          prev.map(input => 
            input.id === inputId 
              ? { ...input, status: "error" as const, error: "This mod is already in the list" }
              : input
          )
        );
        return;
      }

      try {
        // Call Tauri command to get mod details
        const details = await invoke<any>("get_file_details", {
          modId: modId
        });

        if (details.result === 1) {
          // Check if it's a collection
          const isCollection = await invoke<{ isCollection: boolean }>("is_collection", {
            modId: modId
          });

          if (isCollection.isCollection) {
            const files = await invoke<any[]>("get_collection_details", {
              modId: modId
            });

            setModInputs(prev => {
              const updated = prev.map(input => 
                input.id === inputId 
                  ? {
                      ...input,
                      modId,
                      title: details.title,
                      isCollection: true,
                      collectionMods: files,
                      status: "ready" as const
                    }
                  : input
              );

              // Check if we need to add a new empty input
              const hasEmptyInput = updated.some(inp => inp.status === "empty" && !inp.value.trim());
              if (!hasEmptyInput) {
                // Add new empty input at the end
                const newId = String(Date.now());
                updated.push({ id: newId, value: "", status: "empty" as const });
              }

              return updated;
            });
          } else {
            setModInputs(prev => {
              const updated = prev.map(input => 
                input.id === inputId 
                  ? {
                      ...input,
                      modId,
                      title: details.title,
                      size: details.file_size,
                      status: "ready" as const
                    }
                  : input
              );

              // Check if we need to add a new empty input
              const hasEmptyInput = updated.some(inp => inp.status === "empty" && !inp.value.trim());
              if (!hasEmptyInput) {
                // Add new empty input at the end
                const newId = String(Date.now());
                updated.push({ id: newId, value: "", status: "empty" as const });
              }

              return updated;
            });
          }
        } else {
        setModInputs(prev => 
          prev.map(input => 
            input.id === inputId 
              ? { ...input, status: "error" as const, error: "Invalid mod" }
              : input
          )
        );
        }
      } catch (error) {
        console.error(`Failed to fetch details for mod ${modId}:`, error);
        setModInputs(prev => 
          prev.map(input => 
            input.id === inputId 
              ? { ...input, status: "error" as const, error: "Error fetching information" }
              : input
          )
        );
      }
    } else {
      // Input is empty, remove it if it's not the first one and there are other inputs
      setModInputs(prev => {
        if (prev.length > 1) {
          return prev.filter(input => input.id !== inputId || input.value.trim() !== "");
        }
        return prev;
      });
    }
  };

  const handleRemoveInput = (inputId: string) => {
    setModInputs(prev => {
      const filtered = prev.filter(input => input.id !== inputId);
      // Ensure at least one empty input exists
      const hasEmptyInput = filtered.some(inp => inp.status === "empty" && !inp.value.trim());
      if (!hasEmptyInput) {
        // Add empty input if none exists
        filtered.push({ id: String(Date.now()), value: "", status: "empty" as const });
      }
      return filtered;
    });
  };

  const handleCancelDownload = async () => {
    try {
      await invoke("cancel_update_mods");
      console.log("[DOWNLOAD] Cancellation requested");
      
      // Mark all downloading mods as cancelled
      setModInputs(prev => 
        prev.map(m => 
          m.status === "downloading" 
            ? { ...m, status: "cancelled" as const }
            : m
        )
      );
      
      setIsDownloading(false);
      setDownloadStatus("Download cancelled");
    } catch (error) {
      console.error("Failed to cancel download:", error);
    }
  };

  const handleDownloadAll = async () => {
    // Include both "ready" and "cancelled" mods (cancelled mods can be retried)
    const readyMods = modInputs.filter(m => (m.status === "ready" || m.status === "cancelled") && m.modId);
    if (readyMods.length === 0 || isDownloading) return;

    if (!permissions.canWrite) {
      openModal("message", {
        title: "Write Access Required",
        message: "Write access is required to download mods. Please check directory permissions in Settings.",
        type: "error"
      });
      return;
    }

    // Reset cancellation flag before starting download
    try {
      await invoke("reset_update_cancel_flag_command");
    } catch (error) {
      console.warn("Failed to reset cancellation flag:", error);
    }

    setIsDownloading(true);
    setProgress(0);
    setProgressMax(readyMods.length);
    setDownloadStatus("Starting download...");
    
    let successfullyDownloaded = 0;
    let failedDownloads = 0;

    try {
      for (let i = 0; i < readyMods.length; i++) {
        // Check if download was cancelled before processing each mod
        try {
          const isCancelled = await invoke<boolean>("check_update_cancelled");
          if (isCancelled) {
            setDownloadStatus("Download cancelled");
            setIsDownloading(false);
            // Mark all downloading mods as cancelled
            setModInputs(prev => 
              prev.map(m => 
                m.status === "downloading" 
                  ? { ...m, status: "cancelled" as const }
                  : m
              )
            );
            break;
          }
        } catch (error) {
          // If command doesn't exist or fails, continue
          console.warn("Failed to check cancellation status:", error);
        }
        
        const mod = readyMods[i];
        
        // Set mod status to downloading when starting download
        setModInputs(prev => 
          prev.map(m => 
            m.id === mod.id 
              ? { ...m, status: "downloading" as const }
              : m
          )
        );

        setDownloadStatus(`Downloading: ${mod.title || mod.modId}...`);

        try {
          if (mod.isCollection && mod.collectionMods) {
            // Download collection - fetch all details in batch
            const collectionModIds = mod.collectionMods.map((f: any) => f.publishedfileid || f.modId).filter(Boolean);
            if (collectionModIds.length > 0) {
              const detailsMap = await invoke<Record<string, any>>("get_file_details_batch", {
                modIds: collectionModIds
              });
              
              // Download each mod in collection
              for (const file of mod.collectionMods) {
                // Check cancellation before each mod in collection
                try {
                  const isCancelled = await invoke<boolean>("check_update_cancelled");
                  if (isCancelled) {
                    setDownloadStatus("Download cancelled");
                    setIsDownloading(false);
                    // Mark all downloading mods as cancelled
                    setModInputs(prev => 
                      prev.map(m => 
                        m.status === "downloading" 
                          ? { ...m, status: "cancelled" as const }
                          : m
                      )
                    );
                    return;
                  }
                } catch (error) {
                  // Continue if check fails
                }
                
                const modId = file.publishedfileid || file.modId;
                if (modId && detailsMap[modId] && detailsMap[modId] !== null) {
                  await downloadMod(detailsMap[modId], modsPath);
                }
              }
            }
          } else if (mod.modId) {
            // Download single mod - use batch for consistency (even for single mod)
            const detailsMap = await invoke<Record<string, any>>("get_file_details_batch", {
              modIds: [mod.modId]
            });
            
            const details = detailsMap[mod.modId];
            if (details && details !== null && details.result === 1) {
              await downloadMod(details, modsPath);
            } else {
              throw new Error("Invalid mod");
            }
          }

          // Status will be updated by backend events (mod-updated or mod-state)
          // We don't manually set status here to avoid conflicts with event-driven updates
          successfullyDownloaded++;
        } catch (error) {
          console.error(`Failed to download mod ${mod.modId}:`, error);
          
          const errorMessage = error instanceof Error ? error.message : String(error);
          
          // Check if error is due to cancellation
          const isCancelled = errorMessage.includes("cancelled") || 
                             errorMessage.includes("Update cancelled") ||
                             errorMessage.includes("Update cancelled by user");
          
          // Update mod status - check current status to avoid overwriting cancelled
          setModInputs(prev => {
            const currentMod = prev.find(m => m.id === mod.id);
            // If mod was already marked as cancelled, don't overwrite it
            if (currentMod?.status === "cancelled") {
              return prev;
            }
            
            if (isCancelled) {
              // Mark as cancelled, not error
              return prev.map(m => 
                m.id === mod.id 
                  ? { ...m, status: "cancelled" as const }
                  : m
              );
            } else {
              // Mark as error for other failures
              return prev.map(m => 
                m.id === mod.id 
                  ? { ...m, status: "error" as const, error: errorMessage }
                  : m
              );
            }
          });
          
          // Only increment failedDownloads for non-cancelled errors
          if (!isCancelled) {
            failedDownloads++;
          }
        }

        setProgress(i + 1);
      }

      // Only clear mod inputs if all mods were successfully downloaded
      if (successfullyDownloaded > 0 && failedDownloads === 0) {
        setDownloadStatus("Download completed");
        // Clear mod inputs after successful download
        setModInputs([{ id: String(Date.now()), value: "", status: "empty" }]);
      } else if (successfullyDownloaded > 0) {
        setDownloadStatus(`Download completed with ${failedDownloads} error(s)`);
      } else if (failedDownloads > 0) {
        setDownloadStatus(`Download failed: ${failedDownloads} error(s)`);
      } else {
        setDownloadStatus("Download cancelled or no mods processed");
      }
    } catch (error) {
      console.error("Failed to download mods:", error);
      setDownloadStatus("Error during download");
    } finally {
      setIsDownloading(false);
    }
  };

  const downloadMod = async (details: any, modsPath: string) => {
    try {
      // Call Tauri command to download mod
      const result = await invoke<{ modId: string; modPath: string; folder: string }>("download_mod", {
        modId: details.publishedfileid,
        title: details.title,
        modsPath: modsPath,
        maxSteamcmdInstances: settings.maxSteamcmdInstances || 1
      });
      
      // Convert result to BaseMod format
      const mod: BaseMod = {
        modId: result.modId,
        modPath: result.modPath,
        folder: result.folder,
        details: details,
        updated: undefined
      };
      
      setDownloadedMods(prev => [...prev, mod]);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      // Check if this is a corrupted mod conflict error
      if (errorMessage.startsWith("CORRUPTED_MOD_CONFLICT:")) {
        // Parse the error: CORRUPTED_MOD_CONFLICT:folderName:modId:modTitle
        const parts = errorMessage.split(":");
        if (parts.length >= 4) {
          const folderName = parts[1];
          const modId = parts[2];
          const modTitle = parts.slice(3).join(":"); // In case title contains ":"
          
          // Show modal to ask user for decision
          return new Promise<void>((resolve, reject) => {
            openModal("corrupted-mod-conflict", {
              folderName,
              modId,
              modTitle: modTitle || details.title || modId,
              onResolve: async (overwrite: boolean) => {
                try {
                  const result = await invoke<{ modId: string; modPath: string }>("continue_download_with_decision", {
                    modId: modId,
                    modsPath: modsPath,
                    overwrite: overwrite
                  });
                  
                  // Convert result to BaseMod format
                  const mod: BaseMod = {
                    modId: result.modId,
                    modPath: result.modPath,
                    folder: folderName,
                    details: details,
                    updated: undefined
                  };
                  
                  setDownloadedMods(prev => [...prev, mod]);
                  resolve();
                } catch (err) {
                  console.error("Failed to continue download:", err);
                  reject(err);
                }
              },
              onReject: () => {
                reject(new Error("Download cancelled by user"));
              }
            });
          });
        }
      }
      
      console.error("Failed to download mod:", error);
      throw error;
    }
  };


  const getTotalSize = (mod: ModInput): string => {
    if (mod.isCollection && mod.collectionMods) {
      const totalSize = mod.collectionMods.reduce((sum: number, f: any) => {
        const size = typeof f.file_size === 'string' ? parseInt(f.file_size, 10) : (f.file_size || 0);
        return sum + (isNaN(size) ? 0 : size);
      }, 0);
      return formatSize(totalSize);
    }
    // Handle single mod size - might also be a string
    const size = typeof mod.size === 'string' ? parseInt(mod.size, 10) : mod.size;
    if (size === undefined || isNaN(size)) {
      return formatSize(undefined);
    }
    return formatSize(size);
  };

  const handleOpenWorkshop = async () => {
    await openUrl("https://steamcommunity.com/app/294100/workshop/");
  };

  const handleImportMods = async () => {
    if (!importText.trim()) {
      openModal("message", {
        title: "Import Mods",
        message: "Please paste a list of mod URLs or IDs",
        type: "info"
      });
      return;
    }

    // Parse the import text - split by newlines, commas, or spaces
    const lines = importText
      .split(/\n|,|\s+/)
      .map(line => line.trim())
      .filter(line => line.length > 0);

    if (lines.length === 0) {
      openModal("message", {
        title: "Import Mods",
        message: "No valid mod URLs or IDs found",
        type: "info"
      });
      return;
    }

    // Extract mod URLs/IDs from each line and remove duplicates
    const modUrls: string[] = [];
    const seenModIds = new Set<string>();
    
    // Get current mod IDs to check against
    const currentModIds = new Set(modInputs.map(input => input.modId).filter((id): id is string => !!id));
    
    for (const line of lines) {
      const modId = extractModId(line);
      if (modId && !seenModIds.has(modId)) {
        // Check if mod already exists in current inputs
        if (!currentModIds.has(modId)) {
          seenModIds.add(modId);
          // If line already contains a URL, use it; otherwise create URL from ID
          if (line.includes("steamcommunity.com") || line.includes("http")) {
            modUrls.push(line);
          } else {
            modUrls.push(`https://steamcommunity.com/sharedfiles/filedetails/?id=${modId}`);
          }
        }
      }
    }

    if (modUrls.length === 0) {
      const validModIds = lines.map(line => extractModId(line)).filter((id): id is string => !!id);
      
      if (validModIds.length === 0) {
        openModal("message", {
          title: "Import Mods",
          message: "No valid mod URLs or IDs found in the pasted text",
          type: "info"
        });
      } else {
        // Silently ignore if all are duplicates - no alert needed
        // Just close the modal
      }
      setShowImportModal(false);
      setImportText("");
      return;
    }

    // Close modal and clear import text first
    setShowImportModal(false);
    const urlsToImport = [...modUrls];
    setImportText("");

    // Extract all mod IDs from URLs
    const modIds = urlsToImport.map(url => extractModId(url)).filter((id): id is string => !!id);
    
    if (modIds.length === 0) {
      return;
    }

    // Create new inputs for imported mods with unique IDs
    const baseTime = Date.now();
    const newInputs: ModInput[] = urlsToImport.map((url, index) => ({
      id: `import-${baseTime}-${index}`,
      value: url,
      status: "loading" as const
    }));

    // Add new inputs to state first
    setModInputs(prev => {
      // Remove empty inputs except keep at least one empty input at the end
      const filtered = prev.filter(input => input.value.trim() !== "");
      
      // Add new inputs and ensure there's at least one empty input at the end
      const updated = [...filtered, ...newInputs];
      
      // Add empty input at the end if there isn't one
      const hasEmptyInput = updated.some(inp => inp.status === "empty" && !inp.value.trim());
      if (!hasEmptyInput) {
        updated.push({ id: `empty-${Date.now()}`, value: "", status: "empty" as const });
      }

      return updated;
    });

    // Fetch all mod details in batch
    try {
      const detailsMap = await invoke<Record<string, any>>("get_file_details_batch", {
        modIds: modIds
      });

      // Process results and check collections
      const modIdToInputId = new Map<string, string>();
      newInputs.forEach((input) => {
        const modId = extractModId(input.value);
        if (modId) {
          modIdToInputId.set(modId, input.id);
        }
      });

      // Check which mods are collections (batch)
      const collectionChecksMap = await invoke<Record<string, { isCollection: boolean }>>("is_collection_batch", {
        modIds: modIds
      });
      
      const collectionChecks = modIds.map((modId) => {
        if (!detailsMap[modId] || detailsMap[modId] === null) {
          return { modId, isCollection: false, details: null };
        }
        const check = collectionChecksMap[modId];
        const isCollection = check?.isCollection || false;
        return { modId, isCollection, details: detailsMap[modId] };
      });

      // Get collection details for collections (batch)
      const collectionIds = collectionChecks
        .filter(check => check.isCollection && check.details)
        .map(check => check.modId);
      
      let collectionDetailsMap = new Map<string, any[]>();
      
      if (collectionIds.length > 0) {
        try {
          const collectionDetailsBatch = await invoke<Record<string, any[]>>("get_collection_details_batch", {
            collectionIds: collectionIds
          });
          
          for (const [collectionId, files] of Object.entries(collectionDetailsBatch)) {
            collectionDetailsMap.set(collectionId, files);
          }
        } catch {
          // Fallback to individual queries if batch fails
          const collectionDetailsPromises = collectionIds.map(async (collectionId) => {
            try {
              const files = await invoke<any[]>("get_collection_details", {
                collectionId: collectionId
              });
              return { modId: collectionId, files };
            } catch {
              return { modId: collectionId, files: [] };
            }
          });
          
          const collectionDetails = await Promise.all(collectionDetailsPromises);
          collectionDetailsMap = new Map(collectionDetails.map(cd => [cd.modId, cd.files]));
        }
      }

      // Update all inputs with results
      setModInputs(prev => {
        return prev.map(input => {
          const modId = extractModId(input.value);
          if (!modId || !modIdToInputId.has(modId)) {
            return input;
          }

          const details = detailsMap[modId];
          if (!details || details === null) {
            return {
              ...input,
              status: "error" as const,
              error: "Invalid mod"
            };
          }

          if (details.result !== 1) {
            return {
              ...input,
              status: "error" as const,
              error: "Invalid mod"
            };
          }

          const check = collectionChecks.find(c => c.modId === modId);
          const isCollection = check?.isCollection || false;

          if (isCollection) {
            const files = collectionDetailsMap.get(modId) || [];
            return {
              ...input,
              modId,
              title: details.title,
              isCollection: true,
              collectionMods: files,
              status: "ready" as const
            };
          } else {
            return {
              ...input,
              modId,
              title: details.title,
              size: details.file_size,
              status: "ready" as const
            };
          }
        });
      });
    } catch (error) {
      console.error("Failed to fetch mod details:", error);
      // Mark all imported inputs as error
      setModInputs(prev => {
        return prev.map(input => {
          const modId = extractModId(input.value);
          if (modId && modIds.includes(modId)) {
            return {
              ...input,
              status: "error" as const,
              error: "Error fetching information"
            };
          }
          return input;
        });
      });
    }
  };

  const readyModsCount = modInputs.filter(m => m.status === "ready" || m.status === "cancelled").length;

  return (
    <div className="download-tab">
      <div className="download-input-section">
        <div className="input-header">
          <h3>Add mods to download</h3>
          <div className="header-buttons">
            <button 
              onClick={() => setShowImportModal(true)}
              className="import-mods-button"
              title="Import multiple mods from a list"
            >
              📋 Import Modlist
            </button>
            <button 
              onClick={handleOpenWorkshop}
              className="open-workshop-button"
              title="Open Steam Workshop in browser"
            >
              🌐 Open Steam Workshop
            </button>
          </div>
        </div>
        
        <div className="input-instructions">
          <p>Paste Steam Workshop mod URL or just mod ID. A new field will automatically appear below after pasting. Downloading collections is supported.</p>
          <p className="example-text">
            <strong>Examples:</strong><br />
            • https://steamcommunity.com/sharedfiles/filedetails/?id=123456789<br />
            • 123456789<br />
            • steamcommunity.com/sharedfiles/filedetails/?id=123456789
          </p>
        </div>

        <div className="mod-inputs-container">
          {modInputs.map((input) => (
            <div key={input.id} className="mod-input-wrapper">
              <div className="mod-input-row">
                <input
                  ref={(el) => { inputRefs.current[input.id] = el; }}
                  type="text"
                  className={`mod-input ${input.status}`}
                  value={input.value}
                  onChange={(e) => handleInputChange(input.id, e.target.value)}
                  placeholder="Paste Steam Workshop mod URL or ID..."
                  disabled={isDownloading && input.status !== "downloading"}
                />
                {input.value.trim() !== "" && (
                  <button
                    onClick={() => handleRemoveInput(input.id)}
                    className="remove-input-button"
                    title="Remove"
                    disabled={isDownloading}
                  >
                    🗑️
                  </button>
                )}
              </div>
              
              {input.status === "loading" && (
                <div className="mod-status loading">
                  <div className="loading-spinner"></div>
                  <span>Checking mod...</span>
                </div>
              )}
              
              {input.status === "ready" && (
                <div className="mod-status ready">
                  <div className="mod-preview">
                    <span className="mod-preview-title">
                      ✓ {input.title || `Mod ID: ${input.modId}`}
                      {input.isCollection && <span className="collection-text"> (collection)</span>}
                    </span>
                    <div className="mod-preview-details">
                      {input.isCollection && input.collectionMods && (
                        <span>{input.collectionMods.length} mods</span>
                      )}
                      <span>Size: {getTotalSize(input)}</span>
                    </div>
                  </div>
                </div>
              )}
              
              {input.status === "error" && (
                <div className="mod-status error">
                  ✗ {input.error || "Error"}
                </div>
              )}
              
              {input.status === "cancelled" && (
                <div className="mod-status cancelled">
                  ⏸ Cancelled
                </div>
              )}
              
              {input.status === "downloading" && (
                <div className="mod-status downloading">
                  ⏳ Downloading: {input.title || input.modId}...
                </div>
              )}
              
              {input.status === "completed" && (
                <div className="mod-status completed">
                  ✓ Downloaded: {input.title || input.modId}
                </div>
              )}
            </div>
          ))}
        </div>

        {readyModsCount > 0 && (
          <div className="download-actions">
            {isDownloading ? (
              <button
                onClick={handleCancelDownload}
                disabled={!isDownloading}
                title="Cancel ongoing download"
                className="download-all-button"
                style={{ backgroundColor: "#d32f2f" }}
              >
                Cancel Download
              </button>
            ) : (
              <button
                onClick={handleDownloadAll}
                disabled={isDownloading || !modsPath || !permissions.canWrite}
                title={!permissions.canWrite ? "Write access required to download mods" : undefined}
                className="download-all-button"
              >
                Download all ({readyModsCount})
              </button>
            )}
          </div>
        )}
      </div>

      {showImportModal && (
        <div className="import-modal-overlay" onClick={() => setShowImportModal(false)}>
          <div className="import-modal" onClick={(e) => e.stopPropagation()}>
            <div className="import-modal-header">
              <h3>Import Mods</h3>
              <button 
                className="close-modal-button"
                onClick={() => setShowImportModal(false)}
              >
                ×
              </button>
            </div>
            <div className="import-modal-content">
              <p>Paste a list of mod URLs or IDs (one per line, or separated by commas):</p>
              <textarea
                className="import-textarea"
                value={importText}
                onChange={(e) => setImportText(e.target.value)}
                placeholder="https://steamcommunity.com/sharedfiles/filedetails/?id=123456789&#10;https://steamcommunity.com/sharedfiles/filedetails/?id=987654321&#10;123456789&#10;987654321"
                rows={10}
              />
              <div className="import-modal-actions">
                <button
                  onClick={() => {
                    setShowImportModal(false);
                    setImportText("");
                  }}
                  className="cancel-button"
                >
                  Cancel
                </button>
                <button
                  onClick={handleImportMods}
                  className="import-button"
                  disabled={!importText.trim()}
                >
                  Import Mods
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {(progressMax > 0 || downloadStatus) && (
        <div className="download-progress-section">
          {progressMax > 0 && (
            <div className="progress-bar-container">
              <progress value={progress} max={progressMax} />
              <span className="progress-text">{progress} / {progressMax}</span>
            </div>
          )}
          {downloadStatus && (
            <div className="download-status">{downloadStatus}</div>
          )}
        </div>
      )}

      {downloadedMods.length > 0 && (
        <div className="downloaded-mods-section">
          <div className="downloaded-mods-header">
            <span>Downloaded mods: {downloadedMods.length}</span>
          </div>
          <div className="downloaded-mods-list">
            {downloadedMods.map((mod) => (
              <div key={mod.modId} className="downloaded-mod-item">
                <span className="mod-name">{mod.details?.title || mod.folder || mod.modId}</span>
                <div className="mod-actions">
                  <button onClick={() => window.open(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.modId}`, "_blank")}>
                    Workshop
                  </button>
                  <button onClick={() => window.open(`https://steamcommunity.com/sharedfiles/filedetails/changelog/${mod.modId}`, "_blank")}>
                    Changelog
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
