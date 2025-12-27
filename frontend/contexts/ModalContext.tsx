import { createContext, useContext, useState, ReactNode, useCallback } from "react";

export type ModalType = "restore-backup" | null;

interface ModalContextType {
  openModal: (type: ModalType, data?: any) => void;
  closeModal: () => void;
  modalType: ModalType;
  modalData: any;
  isModalOpen: boolean;
}

const ModalContext = createContext<ModalContextType | undefined>(undefined);

export function ModalProvider({ children }: { children: ReactNode }) {
  const [modalType, setModalType] = useState<ModalType>(null);
  const [modalData, setModalData] = useState<any>(null);

  const openModal = useCallback((type: ModalType, data?: any) => {
    setModalType(type);
    setModalData(data || null);
  }, []);

  const closeModal = useCallback(() => {
    setModalType(null);
    setModalData(null);
  }, []);

  return (
    <ModalContext.Provider
      value={{
        openModal,
        closeModal,
        modalType,
        modalData,
        isModalOpen: modalType !== null,
      }}
    >
      {children}
    </ModalContext.Provider>
  );
}

export function useModal() {
  const context = useContext(ModalContext);
  if (context === undefined) {
    throw new Error("useModal must be used within a ModalProvider");
  }
  return context;
}

