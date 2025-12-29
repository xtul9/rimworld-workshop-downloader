import { useAccessError } from "../contexts/AccessErrorContext";
import "./AccessErrorBanner.css";

export default function AccessErrorBanner() {
  const { error, isDismissed, dismissError, permissions } = useAccessError();

  if (!error || isDismissed) {
    return null;
  }

  const isWarning = permissions.hasWarning; // canRead=true, canWrite=false
  const isError = permissions.hasError; // canRead=false

  const getTitle = () => {
    if (isError) {
      return "Directory Access Error";
    } else if (isWarning) {
      return "Directory Access Warning";
    }
    return "Directory Access Issue";
  };

  const getErrorMessage = () => {
    if (!error.canRead && !error.canWrite) {
      return `No read and write access to mods directory: ${error.path}`;
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
          <div className="access-error-title">{getTitle()}</div>
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

