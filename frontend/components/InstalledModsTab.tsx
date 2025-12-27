import { useEffect } from "react";
import ModList from "./ModList";
import { useModsPath } from "../contexts/ModsPathContext";
import { useInstalledMods } from "../contexts/InstalledModsContext";
import { useFormatting } from "../hooks/useFormatting";
import { useModal } from "../contexts/ModalContext";
import "./QueryTab.css";

export default function InstalledModsTab() {
  const { modsPath } = useModsPath();
  const {
    mods,
    isLoading,
    isUpdating,
    error,
    hasLoaded,
    loadInstalledMods,
    updateMods,
  } = useInstalledMods();
  const { formatSize } = useFormatting();
  const { openModal } = useModal();

  // Auto-load mods when tab is opened (if not already loaded)
  useEffect(() => {
    if (!hasLoaded && !isLoading && modsPath) {
      loadInstalledMods(modsPath);
    }
  }, [hasLoaded, isLoading, modsPath, loadInstalledMods]);

  const handleForceUpdateAll = () => {
    if (mods.length === 0) return;
    
    const modsToUpdate = mods.filter(m => !m.updated);
    if (modsToUpdate.length === 0) {
      alert("All mods are already up to date.");
      return;
    }

    const totalSize = modsToUpdate.reduce((sum, m) => sum + (m.details?.file_size || 0), 0);
    const sizeText = formatSize(totalSize);
    
    // Open confirmation modal
    openModal("force-update-all", {
      modsCount: modsToUpdate.length,
      totalSize: sizeText,
      onConfirm: async () => {
        await updateMods(modsToUpdate);
      }
    });
  };

  const handleUpdateSelected = async (selectedMods: typeof mods) => {
    await updateMods(selectedMods);
  };

  return (
    <div className="query-tab">
      {!modsPath && (
        <div className="query-path-warning-container">
          <span className="query-path-warning">
            Please set the mods folder path in Settings tab
          </span>
        </div>
      )}

      <div className="mods-section">
        <div className="mods-header">
          <span>
            {isLoading ? (
              "Loading mods..."
            ) : error ? (
              "Error"
            ) : (
              <>
                {mods.length > 0 ? (
                  <>
                    Installed mods: {mods.length}
                    {mods.filter(m => m.updated).length > 0 && (
                      <span className="updated-count"> ({mods.filter(m => m.updated).length} updated)</span>
                    )}
                  </>
                ) : hasLoaded ? (
                  "No mods found"
                ) : (
                  "Mods list"
                )}
              </>
            )}
          </span>
          {!isLoading && !error && mods.length > 0 && (
            <button
              onClick={handleForceUpdateAll}
              disabled={isLoading || isUpdating || mods.filter(m => !m.updated).length === 0}
              title="Force update all mods"
              className="force-update-all-button"
            >
              Force Update All ({mods.filter(m => !m.updated).length})
            </button>
          )}
        </div>
        <ModList
          onUpdateSelected={handleUpdateSelected}
          modsPath={modsPath}
          useInstalledModsContext={true}
        />
      </div>
    </div>
  );
}

