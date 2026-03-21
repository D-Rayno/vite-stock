// src/components/ui/Toast.tsx
import React, { createContext, useContext, useState, useCallback } from "react";
import { clsx } from "clsx";

export type ToastType = "success" | "error" | "warning" | "info";

interface ToastMessage {
  id:      string;
  type:    ToastType;
  message: string;
}

interface ToastContextValue {
  toast:   (msg: string, type?: ToastType) => void;
  success: (msg: string) => void;
  error:   (msg: string) => void;
  warning: (msg: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const remove = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback((message: string, type: ToastType = "info") => {
    const id = crypto.randomUUID();
    setToasts((prev) => [...prev.slice(-4), { id, type, message }]);
    setTimeout(() => remove(id), 4000);
  }, [remove]);

  const ctx: ToastContextValue = {
    toast,
    success: (m) => toast(m, "success"),
    error:   (m) => toast(m, "error"),
    warning: (m) => toast(m, "warning"),
  };

  return (
    <ToastContext.Provider value={ctx}>
      {children}
      {/* Toast portal */}
      <div
        aria-live="polite"
        className="fixed bottom-5 right-5 z-50 flex flex-col gap-2 w-80"
        style={{ direction: "ltr" }}   // toasts always LTR regardless of app lang
      >
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} onClose={() => remove(t.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onClose }: { toast: ToastMessage; onClose: () => void }) {
  const icons: Record<ToastType, string> = {
    success: "✓",
    error:   "✕",
    warning: "⚠",
    info:    "ℹ",
  };

  const colorMap: Record<ToastType, string> = {
    success: "border-l-4 border-green-500 bg-green-50 text-green-900",
    error:   "border-l-4 border-red-500 bg-red-50 text-red-900",
    warning: "border-l-4 border-amber-500 bg-amber-50 text-amber-900",
    info:    "border-l-4 border-brand-500 bg-brand-50 text-brand-900",
  };

  return (
    <div
      className={clsx(
        "card py-3 px-4 shadow-lg flex items-start gap-3",
        "animate-in slide-in-from-right-5 duration-200",
        colorMap[toast.type],
      )}
    >
      <span className="text-lg font-bold shrink-0 mt-0.5">{icons[toast.type]}</span>
      <p className="text-sm flex-1 leading-snug">{toast.message}</p>
      <button onClick={onClose} className="text-current opacity-50 hover:opacity-100 text-lg leading-none">
        ×
      </button>
    </div>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be inside ToastProvider");
  return ctx;
}