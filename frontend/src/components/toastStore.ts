import { create } from "zustand";

export type ToastTone = "neutral" | "good" | "caution" | "error";

export interface Toast {
  id: number;
  /** One sentence, with its full stop. */
  text: string;
  tone: ToastTone;
  /** At most one action. */
  action?: { label: string; onClick: () => void };
  /** Stays until dismissed, like an error: a sentence the user has to act on later. */
  stays?: boolean;
}

interface ToastState {
  toasts: Toast[];
  show: (toast: Omit<Toast, "id">) => number;
  dismiss: (id: number) => void;
}

const LIFETIME_MS = 6000;
let nextId = 1;

/** Toasts leave after six seconds unless they report an error, which stays until dismissed (§7.17). */
export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  show: (toast) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { ...toast, id }] }));
    if (toast.tone !== "error" && !toast.stays) {
      window.setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), LIFETIME_MS);
    }
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export function toast(text: string, tone: ToastTone = "neutral", action?: Toast["action"], stays = false): number {
  return useToasts.getState().show({ text, tone, action, stays });
}
