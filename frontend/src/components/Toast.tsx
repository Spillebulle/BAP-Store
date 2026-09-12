import { X } from "lucide-react";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
import { ICON } from "./icons";
import { useToasts, type Toast as ToastItem } from "./toastStore";

const MAX_VISIBLE = 3;

function ToastView({ toast }: { toast: ToastItem }) {
  const dismiss = useToasts((s) => s.dismiss);
  const dot = toast.tone === "neutral" ? "bs-dot" : `bs-dot bs-dot--${toast.tone === "error" ? "critical" : toast.tone}`;
  return (
    <div className="bs-toast" role={toast.tone === "error" ? "alert" : "status"}>
      <span className={dot} aria-hidden="true" />
      <span className="bs-toast-text">{toast.text}</span>
      {toast.action ? (
        <Button kind="ghost" onClick={toast.action.onClick}>
          {toast.action.label}
        </Button>
      ) : null}
      <IconButton label="Dismiss" size="sm" icon={<X {...ICON} size={14} aria-hidden="true" />} onClick={() => dismiss(toast.id)} />
    </div>
  );
}

/** The stack at the bottom right: never more than three, the rest folded into a count. */
export function Toasts() {
  const toasts = useToasts((s) => s.toasts);
  if (toasts.length === 0) return null;
  const visible = toasts.slice(-MAX_VISIBLE);
  const hidden = toasts.length - visible.length;
  return (
    <div className="bs-toasts">
      {hidden > 0 ? (
        <span className="bs-toast-more">
          {hidden} more {hidden === 1 ? "message" : "messages"}
        </span>
      ) : null}
      {visible.map((t) => (
        <ToastView key={t.id} toast={t} />
      ))}
    </div>
  );
}
