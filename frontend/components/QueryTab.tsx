import ModList from "./ModList";
import { useModsPath } from "../contexts/ModsPathContext";
import { useMods } from "../contexts/ModsContext";
import { useAccessError } from "../contexts/AccessErrorContext";
import { useModal } from "../contexts/ModalContext";
import { useFormatting } from "../hooks/useFormatting";
import "./QueryTab.css";

export default function QueryTab() {
  const { modsPath } = useModsPath();
  const {
    mods,
    isQuerying,
    isUpdating,
    error,
    hasQueried,
    queryMods,
    updateMods,
  } = useMods();
  const { permissions } = useAccessError();
  const { openModal } = useModal();
  const { formatSize } = useFormatting();

  const handleQueryMods = async () => {
    await queryMods(modsPath);
  };

  const handleUpdateAll = async () => {
    if (mods.length === 0) return;
    
    if (!permissions.canWrite) {
      openModal("message", {
        title: "Write Access Required",
        message: "Write access is required to update mods. Please check directory permissions in Settings.",
        type: "error"
      });
      return;
    }
    
    const modsToUpdate = mods.filter(m => !m.updated);
    if (modsToUpdate.length === 0) {
      openModal("message", {
        title: "All Mods Up to Date",
        message: "All mods are already up to date.",
        type: "info"
      });
      return;
    }

    const totalSize = modsToUpdate.reduce((sum, m) => sum + (m.details?.file_size || 0), 0);
    const sizeText = formatSize(totalSize);
    
    if (!confirm(`Are you sure you want to update these ${modsToUpdate.length} mod(s)?\nThe total download size will be ~${sizeText}`)) {
      return;
    }

    await updateMods(modsToUpdate);
  };

  const handleUpdateSelected = async (selectedMods: typeof mods) => {
    if (!permissions.canWrite) {
      openModal("message", {
        title: "Write Access Required",
        message: "Write access is required to update mods. Please check directory permissions in Settings.",
        type: "error"
      });
      return;
    }
    await updateMods(selectedMods);
  };

  return (
    <div className="query-tab">
          <button
            onClick={handleQueryMods}
            disabled={isQuerying || isUpdating || !modsPath}
            title="Query for outdated mods"
        className="query-mods-button"
          >
            Query Mods
          </button>
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
              {isQuerying ? (
                "Querying mods..."
              ) : error ? (
                "Error"
              ) : (
                <>
                  {mods.length > 0 ? (
                    <>
                      Mods with updates available: {mods.length}
                      {mods.filter(m => m.updated).length > 0 && (
                        <span className="updated-count"> ({mods.filter(m => m.updated).length} updated)</span>
                      )}
                    </>
                ) : hasQueried ? (
                  "All mods are up to date"
                  ) : (
                  "Mods list"
                  )}
                </>
              )}
            </span>
            {!isQuerying && !error && mods.length > 0 && (
              <button
                onClick={handleUpdateAll}
                disabled={isQuerying || isUpdating || mods.filter(m => !m.updated).length === 0 || !permissions.canWrite}
                title={!permissions.canWrite ? "Write access required to update mods" : "Update all mods with available updates"}
                className="update-all-button"
              >
                Update All ({mods.filter(m => !m.updated).length})
              </button>
            )}
          </div>
          <ModList
            onUpdateSelected={handleUpdateSelected}
            modsPath={modsPath}
          />
        </div>
    </div>
  );
}

