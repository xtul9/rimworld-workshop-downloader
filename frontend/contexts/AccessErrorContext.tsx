import { createContext, useContext, useState, ReactNode, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSettings } from "./SettingsContext";

interface AccessErrorPayload {
  path: string;
  canRead: boolean;
  canWrite: boolean;
  reason: string;
}

interface AccessErrorContextType {
  error: AccessErrorPayload | null;
  isDismissed: boolean;
  dismissError: () => void;
  hasActiveError: boolean; // true if error exists and is not dismissed
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

  const hasActiveError = error !== null && !isDismissed;

  return (
    <AccessErrorContext.Provider
      value={{
        error,
        isDismissed,
        dismissError,
        hasActiveError,
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

