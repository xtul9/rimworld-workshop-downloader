import { useAccessError } from "../contexts/AccessErrorContext";
import "./AccessErrorBanner.css";

export default function AccessErrorBanner() {
  const { error, isDismissed, dismissError } = useAccessError();

  if (!error || isDismissed) {
    return null;
  }

  const getErrorMessage = () => {
    if (!error.canRead && !error.canWrite) {
      return `No read or write access to mods directory: ${error.path}`;
    } else if (!error.canRead) {
      return `No read access to mods directory: ${error.path}`;
    } else if (!error.canWrite) {
      return `No write access to mods directory: ${error.path}. Updates and downloads will fail.`;
    }
    return `Access error: ${error.reason}`;
  };

  return (
    <div className="access-error-banner">
      <div className="access-error-content">
        <div className="access-error-icon">⚠️</div>
        <div className="access-error-text">
          <div className="access-error-title">Directory Access Error</div>
          <div className="access-error-message">{getErrorMessage()}</div>
          {error.reason && (
            <div className="access-error-details">Details: {error.reason}</div>
          )}
        </div>
        <button
          className="access-error-close"
          onClick={dismissError}
          aria-label="Dismiss error"
        >
          ×
        </button>
      </div>
    </div>
  );
}

