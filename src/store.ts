import { create } from 'zustand';

export type CallState = 
  | 'UNINITIALIZED'
  | 'INITIALIZED'
  | 'REGISTERING'
  | 'REGISTERED'
  | 'INCOMING'
  | 'OUTGOING'
  | 'ACTIVE'
  | 'HELD'
  | 'TERMINATED';

interface PhoneState {
  callState: CallState;
  phoneNumber: string;
  isRegistered: boolean;
  currentCallId: string | null;
  
  setCallState: (state: CallState) => void;
  setPhoneNumber: (number: string) => void;
  setIsRegistered: (registered: boolean) => void;
  setCurrentCallId: (id: string | null) => void;
}

export const usePhoneStore = create<PhoneState>((set) => ({
  callState: 'UNINITIALIZED',
  phoneNumber: '',
  isRegistered: false,
  currentCallId: null,
  
  setCallState: (state) => set({ callState: state }),
  setPhoneNumber: (number) => set({ phoneNumber: number }),
  setIsRegistered: (registered) => set({ isRegistered: registered }),
  setCurrentCallId: (id) => set({ currentCallId: id }),
}));
