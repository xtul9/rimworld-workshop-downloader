import { useCallback } from "react";
import { useModal } from "../contexts/ModalContext";
import "./ModList.css";

interface ForceUpdateAllModalProps {
  modsCount: number;
  totalSize: string;
  onConfirm: () => Promise<void>;
}

export default function ForceUpdateAllModal({ modsCount, totalSize, onConfirm }: ForceUpdateAllModalProps) {
  const { closeModal } = useModal();

  const handleConfirm = useCallback(async () => {
    await onConfirm();
    closeModal();
  }, [onConfirm, closeModal]);

  return (
    <div className="restore-modal-overlay" onClick={closeModal}>
      <div className="restore-modal" onClick={(e) => e.stopPropagation()}>
        <div className="restore-modal-header">
          <h3>Force Update All Mods</h3>
          <button 
            className="close-modal-button"
            onClick={closeModal}
          >
            ×
          </button>
        </div>
        <div className="restore-modal-content">
          <p>
            You are about to force update <strong>{modsCount} mod(s)</strong>.
          </p>
          <p>
            The total download size will be approximately <strong>{totalSize}</strong>, 
            with no capability to download in parallel.
          </p>
          <p className="restore-warning">
            <strong>Warning:</strong> This is a potentially long-running operation, 
            and currently, there is no way to cancel it gracefully.
          </p>
          <p>
            Do you want to proceed?
          </p>
        </div>
        <div className="restore-modal-actions">
          <button
            onClick={closeModal}
            className="cancel-button"
          >
            Cancel
          </button>
          <button
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              handleConfirm();
            }}
            className="force-update-button"
            type="button"
          >
            Force Update All
          </button>
        </div>
      </div>
    </div>
  );
}

