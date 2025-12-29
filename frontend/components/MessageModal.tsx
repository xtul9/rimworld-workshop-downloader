import { useModal } from "../contexts/ModalContext";
import "./ModList.css";

interface MessageModalProps {
  title: string;
  message: string;
  type?: "error" | "info" | "warning";
}

export default function MessageModal({ title, message, type = "info" }: MessageModalProps) {
  const { closeModal } = useModal();

  return (
    <div className="restore-modal-overlay" onClick={closeModal}>
      <div className="restore-modal" onClick={(e) => e.stopPropagation()}>
        <div className="restore-modal-header">
          <h3>{title}</h3>
          <button 
            className="close-modal-button"
            onClick={closeModal}
          >
            ×
          </button>
        </div>
        <div className="restore-modal-content">
          <p>{message}</p>
        </div>
        <div className="restore-modal-actions">
          <button
            onClick={closeModal}
            className="restore-button"
          >
            OK
          </button>
        </div>
      </div>
    </div>
  );
}

