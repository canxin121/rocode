// ── Pure Utility Functions ──────────────────────────────────────────────────

export function timeGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 6) return "Good night";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

export function escapeHtml(input: string): string {
  return String(input)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function formatTime(ts: number | string | undefined | null): string {
  if (!ts) return "--";
  const date = new Date(Number(ts));
  if (Number.isNaN(date.getTime())) return "--";
  return date.toLocaleString();
}

export function short(text: string | undefined | null, max = 42): string {
  if (!text) return "(untitled)";
  const clean = String(text).trim();
  if (clean.length <= max) return clean;
  return clean.slice(0, max - 1) + "...";
}

export function baseName(path: string | undefined | null): string {
  if (!path) return "workspace";
  const chunks = String(path).split(/[\\/]/).filter(Boolean);
  return chunks[chunks.length - 1] || "workspace";
}

export function compactPath(path: string | undefined | null, max = 52): string {
  if (!path) return "workspace";
  let normalized = String(path).trim();
  normalized = normalized.replace(/^\/home\/[^/]+/, "~");
  if (normalized.length <= max) return normalized;
  return `...${normalized.slice(-(max - 3))}`;
}
