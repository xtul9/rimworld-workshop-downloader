import { useEffect, useState, useMemo } from "react";
import ModList from "./ModList";
import { useModsPath } from "../contexts/ModsPathContext";
import { useInstalledMods } from "../contexts/InstalledModsContext";
import { useFormatting } from "../hooks/useFormatting";
import { useModal } from "../contexts/ModalContext";
import Select from "./Select";
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
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"date" | "name">("date");
  const [sortOrder, setSortOrder] = useState<"desc" | "asc">("desc");

  // Filter and sort mods
  const filteredAndSortedMods = useMemo(() => {
    // First filter by search query
    let result = mods;
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase().trim();
      result = mods.filter(mod => {
        // Search by mod name (title)
        const nameMatch = mod.details?.title?.toLowerCase().includes(query);
        
        // Search by folder name
        const folderMatch = mod.folder?.toLowerCase().includes(query);
        
        // Search by mod ID
        const idMatch = mod.modId.toLowerCase().includes(query);
        
        return nameMatch || folderMatch || idMatch;
      });
    }

    // Then sort
    const sorted = [...result].sort((a, b) => {
      if (sortBy === "name") {
        const nameA = a.details?.title || a.folder || a.modId || "";
        const nameB = b.details?.title || b.folder || b.modId || "";
        const comparison = nameA.localeCompare(nameB, undefined, { sensitivity: "base" });
        return sortOrder === "asc" ? comparison : -comparison;
      } else {
        // Sort by date (time_updated)
        const dateA = a.details?.time_updated || 0;
        const dateB = b.details?.time_updated || 0;
        const comparison = dateA - dateB;
        return sortOrder === "asc" ? comparison : -comparison;
      }
    });

    return sorted;
  }, [mods, searchQuery, sortBy, sortOrder]);

  // Auto-load mods when tab is opened (if not already loaded)
  useEffect(() => {
    if (!hasLoaded && !isLoading && modsPath) {
      loadInstalledMods(modsPath);
    }
  }, [hasLoaded, isLoading, modsPath, loadInstalledMods]);

  const handleForceUpdateAll = () => {
    if (filteredAndSortedMods.length === 0) return;
    
    const modsToUpdate = filteredAndSortedMods.filter(m => !m.updated);
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

      {/* Search input and sort controls */}
      {hasLoaded && mods.length > 0 && (
        <div className="search-sort-container">
          <div className="search-container">
            <input
              type="text"
              className="search-input"
              placeholder="Search mods by name, folder, or mod ID..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
            {searchQuery && (
              <button
                className="search-clear-button"
                onClick={() => setSearchQuery("")}
                title="Clear search"
              >
                ×
              </button>
            )}
          </div>
          <div className="sort-container">
            <label htmlFor="sort-by" className="sort-label">Sort by:</label>
            <Select<"name" | "date">
              id="sort-by"
              value={sortBy}
              onChange={(value) => setSortBy(value)}
              options={[
                  { value: "date", label: "Update Date" },
                  { value: "name", label: "Name" }
              ]}
            />
            <button
              className="sort-order-button"
              onClick={() => setSortOrder(sortOrder === "asc" ? "desc" : "asc")}
              title={sortOrder === "asc" ? "Ascending" : "Descending"}
            >
              {sortOrder === "asc" ? "↑" : "↓"}
            </button>
          </div>
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
                {searchQuery ? (
                  <>
                    {filteredAndSortedMods.length > 0 ? (
                      <>
                        Found {filteredAndSortedMods.length} within all {mods.length} mod(s)
                      </>
                    ) : (
                      `No mods found matching "${searchQuery}"`
                    )}
                  </>
                ) : mods.length > 0 ? (
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
          {!isLoading && !error && filteredAndSortedMods.length > 0 && (
            <button
              onClick={handleForceUpdateAll}
              disabled={isLoading || isUpdating || filteredAndSortedMods.filter(m => !m.updated).length === 0}
              title="Force update all mods"
              className="force-update-all-button"
            >
              Force Update All ({filteredAndSortedMods.filter(m => !m.updated).length})
            </button>
          )}
        </div>
        <ModList
          onUpdateSelected={handleUpdateSelected}
          modsPath={modsPath}
          useInstalledModsContext={true}
          filteredMods={filteredAndSortedMods}
        />
      </div>
    </div>
  );
}

