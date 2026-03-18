import { type Component, type JSX } from "solid-js";

export interface BadgeProps {
  tone?: "ok" | "warn" | "error" | "info";
  children: JSX.Element;
}

export const Badge: Component<BadgeProps> = (props) => {
  const toneColor = () => {
    switch (props.tone) {
      case "ok": return "var(--color-success)";
      case "warn": return "var(--color-warning)";
      case "error": return "var(--color-error)";
      case "info": return "var(--color-info)";
      default: return "var(--color-text-tertiary)";
    }
  };

  return (
    <span
      style={{
        display: "inline-flex",
        "align-items": "center",
        padding: "2px 8px",
        "border-radius": "var(--radius-full)",
        "font-size": "var(--font-size-xs)",
        "font-weight": "600",
        color: toneColor(),
        background: `color-mix(in srgb, ${toneColor()} 10%, transparent)`,
      }}
    >
      {props.children}
    </span>
  );
};

export interface MetaPillProps {
  label: string;
  value: string;
}

export const MetaPill: Component<MetaPillProps> = (props) => {
  return (
    <span
      style={{
        display: "inline-flex",
        "align-items": "center",
        gap: "4px",
        padding: "2px 8px",
        "border-radius": "var(--radius-full)",
        background: "var(--color-bg-elevated)",
        "font-size": "var(--font-size-xs)",
        color: "var(--color-text-secondary)",
      }}
    >
      <span style={{ "font-weight": "600", color: "var(--color-text-tertiary)" }}>
        {props.label}
      </span>
      <span>{props.value}</span>
    </span>
  );
};

export interface ToastProps {
  message: string;
  tone?: "success" | "error" | "warning" | "info";
}

export const Toast: Component<ToastProps> = (props) => {
  const toneColor = () => {
    switch (props.tone) {
      case "success": return "var(--color-success)";
      case "error": return "var(--color-error)";
      case "warning": return "var(--color-warning)";
      default: return "var(--color-info)";
    }
  };

  return (
    <div
      style={{
        position: "fixed",
        bottom: "24px",
        right: "24px",
        "z-index": "var(--z-toast)",
        padding: "12px 20px",
        "border-radius": "var(--radius)",
        background: "var(--color-bg-elevated)",
        "box-shadow": "var(--shadow-lg)",
        "border-left": `3px solid ${toneColor()}`,
        "font-size": "var(--font-size-sm)",
      }}
    >
      {props.message}
    </div>
  );
};
