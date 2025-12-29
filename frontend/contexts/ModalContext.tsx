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
  const [totalQueueLength, setTotalQueueLength] = useState<number>(0);
  const [modalType, setModalType] = useState<ModalType>(null);
  const [modalData, setModalData] = useState<any>(null);

  const openModal = useCallback((type: ModalType, data?: any) => {
    const newItem: ModalQueueItem = { type, data: data || null };
    
    setModalQueue(prev => {
      const newQueue = [...prev, newItem];
      const isQueueEmpty = prev.length === 0;
      
      // Update total queue length only if starting a new batch (queue was empty)
      // Otherwise, increment it to maintain correct position calculation
      if (isQueueEmpty) {
        // Starting a new batch - set total length to 1
        setTotalQueueLength(1);
      } else {
        // Adding to existing batch - increment total length
        setTotalQueueLength(prevTotal => prevTotal + 1);
      }
      
      // If no modal is currently shown, show this one immediately
      if (isQueueEmpty) {
        setModalType(type);
        setModalData({
          ...(data || {}),
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
        });
        return rest;
      } else {
        // Queue is empty
        setModalType(null);
        setModalData(null);
        setTotalQueueLength(0);
        return [];
      }
    });
  }, []);

  // Calculate current position: totalQueueLength - remaining items + 1
  // If queue is empty, position is 0
  const queuePosition = modalQueue.length > 0 
    ? totalQueueLength - modalQueue.length + 1 
    : 0;
  const queueLength = totalQueueLength;

  return (
    <ModalContext.Provider
      value={{
        openModal,
        closeModal,
        modalType,
        modalData,
        isModalOpen: modalType !== null,
        queueLength,
        queuePosition,
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

