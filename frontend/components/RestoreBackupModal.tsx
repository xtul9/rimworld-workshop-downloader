import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BaseMod } from "../types";
import { useModal } from "../contexts/ModalContext";
import { useSettings } from "../contexts/SettingsContext";
import { useAccessError } from "../contexts/AccessErrorContext";
import { useFormatting } from "../hooks/useFormatting";
import "./ModList.css";

interface RestoreBackupModalProps {
  mod: BaseMod;
  backupDate?: Date;
  onRestoreComplete?: () => void;
  error?: string;
}

export default function RestoreBackupModal({ mod, backupDate, onRestoreComplete, error: initialError }: RestoreBackupModalProps) {
  const { closeModal } = useModal();
  const { settings } = useSettings();
  const { permissions } = useAccessError();
  const { formatDate } = useFormatting();
  const [isRestoring, setIsRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(initialError || null);
  const [restoreSuccess, setRestoreSuccess] = useState<string | null>(null);

  const handleRestoreBackup = useCallback(async () => {
    if (!permissions.canWrite) {
      setRestoreError("Write access is required to restore backups. Please check directory permissions in Settings.");
      return;
    }
    
    if (!mod.modPath || !settings.backupDirectory) {
      setRestoreError("Missing required data to restore backup");
      return;
    }

    setIsRestoring(true);
    setRestoreError(null);
    setRestoreSuccess(null);

    try {
      // Call Tauri command to restore backup
      await invoke("restore_backup", {
        modPath: mod.modPath,
        backupDirectory: settings.backupDirectory
      });
      
      setRestoreSuccess(`Backup restored successfully for "${mod.details?.title || mod.folder || mod.modId}"`);
      
      // Call onRestoreComplete callback if provided
      if (onRestoreComplete) {
        setTimeout(() => {
          onRestoreComplete();
        }, 500);
      }
      
      // Close modal after showing success message
      setTimeout(() => {
        closeModal();
        setRestoreSuccess(null);
      }, 2000);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      setRestoreError(errorMessage);
    } finally {
      setIsRestoring(false);
    }
  }, [mod, settings.backupDirectory, onRestoreComplete, closeModal, permissions.canWrite]);

  return (
    <div className="restore-modal-overlay" onClick={() => {
      if (!isRestoring) {
        closeModal();
      }
    }}>
      <div className="restore-modal" onClick={(e) => e.stopPropagation()}>
        <div className="restore-modal-header">
          <h3>Restore Backup</h3>
          {!isRestoring && (
            <button 
              className="close-modal-button"
              onClick={closeModal}
            >
              ×
            </button>
          )}
        </div>
        <div className="restore-modal-content">
          {restoreSuccess ? (
            <div className="restore-success">
              <p>{restoreSuccess}</p>
            </div>
          ) : restoreError ? (
            <div className="restore-error">
              <p>{restoreError}</p>
            </div>
          ) : (
            <>
              <p>
                Are you sure you want to restore the backup for <strong>"{mod.details?.title || mod.folder || mod.modId}"</strong>?
              </p>
              {backupDate && (
                <p className="restore-backup-date">
                  Backup created: <strong>{formatDate(backupDate)}</strong>
                </p>
              )}
              <p className="restore-warning">
                This will replace the current mod with the backup version. The backup will be deleted after restoration.
              </p>
            </>
          )}
        </div>
        <div className="restore-modal-actions">
          {restoreSuccess || restoreError ? (
            <button
              onClick={closeModal}
              className="restore-button"
            >
              Close
            </button>
          ) : (
            <>
              <button
                onClick={closeModal}
                className="cancel-button"
                disabled={isRestoring}
              >
                Cancel
              </button>
              <button
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  handleRestoreBackup();
                }}
                className="restore-button"
                disabled={isRestoring || !permissions.canWrite}
                title={!permissions.canWrite ? "Write access required to restore backups" : undefined}
                type="button"
              >
                {isRestoring ? "Restoring..." : "Restore Backup"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

