import { createContext, useContext, useState, ReactNode, useCallback } from "react";

export interface ContextMenuPosition {
  x: number;
  y: number;
}

export interface ContextMenuData {
  position: ContextMenuPosition;
  data: any;
  items: ContextMenuItem[];
  onAction?: (action: string, data: any) => void | Promise<void>;
}

export interface ContextMenuItem {
  label?: string;
  action?: string;
  disabled?: boolean;
  separator?: boolean;
}

interface ContextMenuContextType {
  showContextMenu: (position: ContextMenuPosition, data: any, items: ContextMenuItem[], onAction?: (action: string, data: any) => void | Promise<void>) => void;
  hideContextMenu: () => void;
  contextMenu: ContextMenuData | null;
  isContextMenuOpen: boolean;
}

const ContextMenuContext = createContext<ContextMenuContextType | undefined>(undefined);

export function ContextMenuProvider({ children }: { children: ReactNode }) {
  const [contextMenu, setContextMenu] = useState<ContextMenuData | null>(null);

  const showContextMenu = useCallback((position: ContextMenuPosition, data: any, items: ContextMenuItem[], onAction?: (action: string, data: any) => void | Promise<void>) => {
    setContextMenu({ position, data, items, onAction });
  }, []);

  const hideContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  return (
    <ContextMenuContext.Provider
      value={{
        showContextMenu,
        hideContextMenu,
        contextMenu,
        isContextMenuOpen: contextMenu !== null,
      }}
    >
      {children}
    </ContextMenuContext.Provider>
  );
}

export function useContextMenu() {
  const context = useContext(ContextMenuContext);
  if (context === undefined) {
    throw new Error("useContextMenu must be used within a ContextMenuProvider");
  }
  return context;
}

