import { createContext, useContext, useState, ReactNode, useCallback } from "react";

export type ModalType = "restore-backup" | "force-update-all" | "message" | "corrupted-mod-conflict" | null;

interface ModalQueueItem {
  type: ModalType;
  data: any;
}

interface ModalContextType {
  openModal: (type: ModalType, data?: any) => void;
  closeModal: () => void;
  modalType: ModalType;
  modalData: any;
  isModalOpen: boolean;
  queueLength: number;
  queuePosition: number;
}

const ModalContext = createContext<ModalContextType | undefined>(undefined);

export function ModalProvider({ children }: { children: ReactNode }) {
  const [modalQueue, setModalQueue] = useState<ModalQueueItem[]>([]);
  const [modalType, setModalType] = useState<ModalType>(null);
  const [modalData, setModalData] = useState<any>(null);

  const openModal = useCallback((type: ModalType, data?: any) => {
    const newItem: ModalQueueItem = { type, data: data || null };
    
    setModalQueue(prev => {
      const newQueue = [...prev, newItem];
      const position = newQueue.length;
      const isQueueEmpty = prev.length === 0;
      
      // If no modal is currently shown, show this one immediately
      if (isQueueEmpty) {
        setModalType(type);
        setModalData({
          ...(data || {}),
          queuePosition: position,
          queueLength: position,
        });
      }
      
      return newQueue;
    });
  }, []);

  const closeModal = useCallback(() => {
    setModalQueue(prev => {
      const [, ...rest] = prev;
      
      if (rest.length > 0) {
        // Show next modal from queue
        const nextModal = rest[0];
        setModalType(nextModal.type);
        setModalData({
          ...(nextModal.data || {}),
          queuePosition: 1,
          queueLength: rest.length,
        });
        return rest;
      } else {
        // Queue is empty
        setModalType(null);
        setModalData(null);
        return [];
      }
    });
  }, []);

  return (
    <ModalContext.Provider
      value={{
        openModal,
        closeModal,
        modalType,
        modalData,
        isModalOpen: modalType !== null,
        queueLength: modalQueue.length,
        queuePosition: modalData?.queuePosition || 0,
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

