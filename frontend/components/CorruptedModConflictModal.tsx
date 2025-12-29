import { useState, useCallback } from "react";
import { useModal } from "../contexts/ModalContext";
import { useAccessError } from "../contexts/AccessErrorContext";
import "./ModList.css";

interface CorruptedModConflictModalProps {
  folderName: string;
  modId: string;
  modTitle: string;
  onResolve: (overwrite: boolean) => Promise<void>;
  onReject?: () => void;
}

export default function CorruptedModConflictModal({ 
  folderName, 
  modId, 
  modTitle,
  onResolve,
  onReject
}: CorruptedModConflictModalProps) {
  const { closeModal, modalData } = useModal();
  const { permissions } = useAccessError();
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const queuePosition = modalData?.queuePosition || 0;
  const queueLength = modalData?.queueLength || 0;
  const showQueueInfo = queueLength > 1;

  const handleOverwrite = useCallback(async () => {
    if (!permissions.canWrite) {
      setError("Write access is required. Please check directory permissions in Settings.");
      return;
    }

    setIsProcessing(true);
    setError(null);

    try {
      await onResolve(true);
      closeModal();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    } finally {
      setIsProcessing(false);
    }
  }, [onResolve, closeModal, permissions.canWrite]);

  const handleRename = useCallback(async () => {
    if (!permissions.canWrite) {
      setError("Write access is required. Please check directory permissions in Settings.");
      return;
    }

    setIsProcessing(true);
    setError(null);

    try {
      await onResolve(false);
      closeModal();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    } finally {
      setIsProcessing(false);
    }
  }, [onResolve, closeModal, permissions.canWrite]);

  const handleCancel = useCallback(() => {
    if (!isProcessing) {
      if (onReject) {
        onReject();
      }
      closeModal();
    }
  }, [onReject, closeModal, isProcessing]);

  return (
    <div className="restore-modal-overlay" onClick={handleCancel}>
      <div className="restore-modal" onClick={(e) => e.stopPropagation()}>
        <div className="restore-modal-header">
          <h3>
            Corrupted Mod Detected
            {showQueueInfo && (
              <span style={{ fontSize: "0.8em", fontWeight: "normal", marginLeft: "10px", color: "#888" }}>
                ({queuePosition} of {queueLength})
              </span>
            )}
          </h3>
          {!isProcessing && (
            <button 
              className="close-modal-button"
              onClick={handleCancel}
            >
              ×
            </button>
          )}
        </div>
        <div className="restore-modal-content">
          {error ? (
            <div className="restore-error">
              <p>{error}</p>
            </div>
          ) : (
            <>
              <p>
                Local mod <strong>"{folderName}"</strong> is corrupted (missing About folder or About.xml file).
              </p>
              <p>
                Downloading mod: <strong>"{modTitle}"</strong> (ID: {modId})
              </p>
              <p className="restore-warning">
                What would you like to do?
              </p>
              <ul style={{ marginLeft: "20px", marginTop: "10px" }}>
                <li style={{ marginBottom: "8px" }}>
                  <strong>Overwrite</strong> - will replace the corrupted mod with the downloaded mod
                </li>
                <li style={{ marginBottom: "8px" }}>
                  <strong>Rename</strong> - will create a copy with a different folder name
                </li>
              </ul>
            </>
          )}
        </div>
        <div className="restore-modal-actions">
          {error ? (
            <button
              onClick={handleCancel}
              className="restore-button"
            >
              Close
            </button>
          ) : (
            <>
              <button
                onClick={handleCancel}
                className="cancel-button"
                disabled={isProcessing}
              >
                Cancel
              </button>
              <button
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleRename();
                }}
                className="cancel-button"
                disabled={isProcessing || !permissions.canWrite}
                style={{ marginRight: "8px" }}
                type="button"
              >
                {isProcessing ? "Processing..." : "Rename"}
              </button>
              <button
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleOverwrite();
                }}
                className="restore-button"
                disabled={isProcessing || !permissions.canWrite}
                title={!permissions.canWrite ? "Write access required" : undefined}
                type="button"
              >
                {isProcessing ? "Processing..." : "Overwrite"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

