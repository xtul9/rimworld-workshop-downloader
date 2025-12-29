import { createContext, useContext, useState, ReactNode, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSettings } from "./SettingsContext";

interface AccessErrorPayload {
  path: string;
  canRead: boolean;
  canWrite: boolean;
  reason: string;
}

interface AccessPermissions {
  canRead: boolean;
  canWrite: boolean;
  hasError: boolean; // true if canRead === false (blocks everything)
  hasWarning: boolean; // true if canRead === true && canWrite === false (blocks write operations only)
}

interface AccessErrorContextType {
  error: AccessErrorPayload | null;
  isDismissed: boolean;
  dismissError: () => void;
  hasActiveError: boolean; // true if error exists and is not dismissed and canRead === false
  permissions: AccessPermissions; // Current access permissions
}

const AccessErrorContext = createContext<AccessErrorContextType | undefined>(undefined);

export function AccessErrorProvider({ children }: { children: ReactNode }) {
  const [error, setError] = useState<AccessErrorPayload | null>(null);
  const [isDismissed, setIsDismissed] = useState(false);
  const { settings } = useSettings();

  // Clear error when modsPath changes (new path might have access)
  useEffect(() => {
    if (error && settings.modsPath) {
      if (error.path !== settings.modsPath) {
        setError(null);
        setIsDismissed(false);
      }
    }
  }, [settings.modsPath, error]);

  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<AccessErrorPayload>("no-access-error", (event) => {
        setError(event.payload);
        setIsDismissed(false); // Reset dismissed state when new error arrives
      });

      return unlisten;
    };

    let unlistenFn: (() => void) | undefined;

    setupListener().then((fn) => {
      unlistenFn = fn;
    });

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, []);

  const dismissError = () => {
    setIsDismissed(true);
  };

  // Calculate permissions based on error state
  // If no error, assume full access (default state)
  const permissions: AccessPermissions = error
    ? {
        canRead: error.canRead,
        canWrite: error.canWrite,
        hasError: !error.canRead,
        hasWarning: error.canRead && !error.canWrite,
      }
    : {
        canRead: true,
        canWrite: true,
        hasError: false,
        hasWarning: false,
      };

  // Only block for true errors (no read access), not warnings (read-only access)
  // Backend emits warnings when canRead=true but canWrite=false, which should not block the app
  const hasActiveError = error !== null && !isDismissed && !error.canRead;

  return (
    <AccessErrorContext.Provider
      value={{
        error,
        isDismissed,
        dismissError,
        hasActiveError,
        permissions,
      }}
    >
      {children}
    </AccessErrorContext.Provider>
  );
}

export function useAccessError() {
  const context = useContext(AccessErrorContext);
  if (context === undefined) {
    throw new Error("useAccessError must be used within an AccessErrorProvider");
  }
  return context;
}

