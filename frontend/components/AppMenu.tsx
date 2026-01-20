import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./AppMenu.css";
import packageJson from "../package.json";
import { useSettings } from "../contexts/SettingsContext";
import { useInstalledMods } from "../contexts/InstalledModsContext";

export default function AppMenu() {
  const [isOpen, setIsOpen] = useState(false);
  const [isExportSubmenuOpen, setIsExportSubmenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const exportItemRef = useRef<HTMLDivElement>(null);
  const submenuRef = useRef<HTMLDivElement>(null);
  const hoverTimeoutRef = useRef<number | null>(null);
  const closeTimeoutRef = useRef<number | null>(null);
  const { settings } = useSettings();
  const { mods: installedMods, hasLoaded } = useInstalledMods();
  const [isExporting, setIsExporting] = useState(false);
  const [exportedSuccessfully, setExportedSuccessfully] = useState(false);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setIsOpen(false);
        setIsExportSubmenuOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  // Close submenu when main menu closes
  useEffect(() => {
    if (!isOpen) {
      setIsExportSubmenuOpen(false);
    }
  }, [isOpen]);

  // Keep submenu open when exporting or showing success message
  useEffect(() => {
    if (isExporting || exportedSuccessfully) {
      setIsExportSubmenuOpen(true);
    }
  }, [isExporting, exportedSuccessfully]);

  // Cleanup timeouts on unmount
  useEffect(() => {
    return () => {
      if (hoverTimeoutRef.current) {
        clearTimeout(hoverTimeoutRef.current);
      }
      if (closeTimeoutRef.current) {
        clearTimeout(closeTimeoutRef.current);
      }
    };
  }, []);

  const handleExportMouseEnter = () => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
    hoverTimeoutRef.current = setTimeout(() => {
      setIsExportSubmenuOpen(true);
    }, 300); // 300ms delay
  };

  const handleExportMouseLeave = () => {
    if (hoverTimeoutRef.current) {
      clearTimeout(hoverTimeoutRef.current);
      hoverTimeoutRef.current = null;
    }
    // Don't close submenu if exporting or showing success message
    if (isExporting || exportedSuccessfully) {
      return;
    }
    // Close submenu after a short delay when mouse leaves
    closeTimeoutRef.current = setTimeout(() => {
      const isOverItem = exportItemRef.current?.matches(':hover');
      const isOverSubmenu = submenuRef.current?.matches(':hover');
      if (!isOverItem && !isOverSubmenu && !isExporting && !exportedSuccessfully) {
        setIsExportSubmenuOpen(false);
      }
    }, 100);
  };

  const handleSubmenuMouseEnter = () => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
  };

  const handleSubmenuMouseLeave = () => {
    // Don't close submenu if exporting or showing success message
    if (isExporting || exportedSuccessfully) {
      return;
    }
    closeTimeoutRef.current = setTimeout(() => {
      const isOverItem = exportItemRef.current?.matches(':hover');
      const isOverSubmenu = submenuRef.current?.matches(':hover');
      if (!isOverItem && !isOverSubmenu && !isExporting && !exportedSuccessfully) {
        setIsExportSubmenuOpen(false);
      }
    }, 100);
  };

  const handleExportClick = () => {
    setIsExportSubmenuOpen(!isExportSubmenuOpen);
  };

  const handleCopyToClipboard = async () => {
    if (!settings.modsPath) {
      console.error("Mods path is not set");
      return;
    }

    setIsExporting(true);
    try {
      // If frontend has loaded mods, use them (they're already in memory with Steam details)
      // Otherwise, let backend fetch them
      // Backend will copy to clipboard itself
      if (hasLoaded && installedMods.length > 0) {
        // Use mods from frontend context - they're already loaded and have details
        await invoke("export_mods_to_clipboard", {
          mods: installedMods,
        });
      } else {
        // Backend will fetch mods itself if frontend doesn't have them
        await invoke("export_mods_to_clipboard", {
          modsPath: settings.modsPath,
        });
      }
    } catch (error) {
      console.error("Failed to copy mod list to clipboard:", error);
      // Optionally show error message to user
    } finally {
      setIsExporting(false);
      setExportedSuccessfully(true);
      setTimeout(() => {
        setExportedSuccessfully(false);
      }, 1000);
    }
  };

  return (
    <div className="app-menu-container" ref={menuRef}>
      <button
        className="app-menu-button"
        onClick={() => setIsOpen(!isOpen)}
        aria-expanded={isOpen}
        aria-haspopup="menu"
        title="Menu"
      >
        <span className="app-menu-icon">☰</span>
      </button>
      {isOpen && (
        <div className="app-menu-dropdown">
          <div className="app-menu-item">
            <span className="app-menu-label">Version:</span>
            <span className="app-menu-value">{packageJson.version}</span>
          </div>
          <div className="app-menu-separator"></div>
          <div 
            ref={exportItemRef}
            className="app-menu-item app-menu-item-clickable app-menu-item-with-submenu"
            onMouseEnter={handleExportMouseEnter}
            onMouseLeave={handleExportMouseLeave}
            onClick={handleExportClick}
          >
            <span className="app-menu-arrow">◀</span>
            <span className="app-menu-label">Export mod list</span>
            {isExportSubmenuOpen && (
              <div 
                ref={submenuRef}
                className="app-menu-submenu"
                onMouseEnter={handleSubmenuMouseEnter}
                onMouseLeave={handleSubmenuMouseLeave}
              >
                <div 
                  className={`app-menu-item app-menu-item-clickable ${isExporting || exportedSuccessfully ? "app-menu-item-disabled" : ""}`}
                  onClick={isExporting ? undefined : handleCopyToClipboard}
                >
                  <span className="app-menu-label">
                    {isExporting ? "Please wait..." : exportedSuccessfully ? "Copied!" : "To clipboard"}
                  </span>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
