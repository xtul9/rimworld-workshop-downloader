import { useContextMenu, ContextMenuItem } from "../contexts/ContextMenuContext";
import { useEffect, useState, useRef } from "react";
import "./ModList.css";

export default function ContextMenu() {
  const { contextMenu, hideContextMenu } = useContextMenu();
  const menuRef = useRef<HTMLDivElement>(null);
  const [adjustedPosition, setAdjustedPosition] = useState<{ x: number; y: number } | null>(null);

  // Calculate adjusted position to prevent menu from being cut off
  useEffect(() => {
    if (!contextMenu) {
      setAdjustedPosition(null);
      return;
    }

    // Calculate position after menu is rendered
    const calculateAdjustedPosition = () => {
      const menu = menuRef.current;
      if (!menu) {
        // If menu not yet rendered, use initial position
        setAdjustedPosition({ x: contextMenu.position.x, y: contextMenu.position.y });
        return;
      }

      const menuRect = menu.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;
      
      let x = contextMenu.position.x;
      let y = contextMenu.position.y;

      // Adjust X position if menu would be cut off on the right
      if (x + menuRect.width > viewportWidth) {
        x = viewportWidth - menuRect.width - 10; // 10px margin from edge
        // If still too wide, align to left edge
        if (x < 10) {
          x = 10;
        }
      }

      // Adjust Y position if menu would be cut off at the bottom
      if (y + menuRect.height > viewportHeight) {
        y = viewportHeight - menuRect.height - 10; // 10px margin from edge
        // If still too tall, align to top edge
        if (y < 10) {
          y = 10;
        }
      }

      // Ensure menu doesn't go off the left edge
      if (x < 10) {
        x = 10;
      }

      // Ensure menu doesn't go off the top edge
      if (y < 10) {
        y = 10;
      }

      setAdjustedPosition({ x, y });
    };

    // Use requestAnimationFrame to ensure menu is rendered and measured
    requestAnimationFrame(() => {
      calculateAdjustedPosition();
    });
  }, [contextMenu]);

  // Prevent default context menu when our menu is open
  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const handleContextMenu = (e: MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      // Close current menu when right-clicking elsewhere
      hideContextMenu();
    };

    // Add global listener to prevent system context menu
    document.addEventListener('contextmenu', handleContextMenu, true);

    return () => {
      document.removeEventListener('contextmenu', handleContextMenu, true);
    };
  }, [contextMenu, hideContextMenu]);

  if (!contextMenu) {
    return null;
  }

  const handleAction = async (item: ContextMenuItem) => {
    if (item.disabled || item.separator || !item.action) {
      return;
    }

    if (contextMenu.onAction) {
      await contextMenu.onAction(item.action, contextMenu.data);
    }
    hideContextMenu();
  };

  const handleOverlayContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    hideContextMenu();
  };

  return (
    <>
      <div
        className="context-menu-overlay"
        onClick={hideContextMenu}
        onContextMenu={handleOverlayContextMenu}
      />
      <div
        ref={menuRef}
        className="context-menu"
        style={{ 
          left: adjustedPosition ? `${adjustedPosition.x}px` : `${contextMenu.position.x}px`, 
          top: adjustedPosition ? `${adjustedPosition.y}px` : `${contextMenu.position.y}px`
        }}
      >
        {contextMenu.items.map((item, index) => {
          if (item.separator) {
            return <div key={`separator-${index}`} className="context-menu-separator" />;
          }

          return (
            <div
              key={`item-${index}`}
              className={`context-menu-item ${item.disabled ? "disabled" : ""}`}
              onClick={() => handleAction(item)}
            >
              {item.label}
            </div>
          );
        })}
      </div>
    </>
  );
}

