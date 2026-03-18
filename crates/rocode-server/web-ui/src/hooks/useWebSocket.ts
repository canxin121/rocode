// ── useWebSocket Hook ──────────────────────────────────────────────────────
// Manages a WebSocket connection for terminal PTY sessions.

import { onCleanup } from "solid-js";

export interface UseWebSocketOptions {
  url: string;
  onMessage: (data: string | ArrayBuffer) => void;
  onOpen?: () => void;
  onClose?: () => void;
  onError?: (error: Event) => void;
  protocols?: string[];
}

export function useWebSocket(options: UseWebSocketOptions) {
  let ws: WebSocket | null = null;
  let active = true;

  const connect = () => {
    if (!active) return null;

    ws = new WebSocket(options.url, options.protocols);
    ws.binaryType = "arraybuffer";

    ws.addEventListener("open", () => {
      options.onOpen?.();
    });

    ws.addEventListener("message", (event) => {
      if (event.data instanceof ArrayBuffer) {
        const bytes = new Uint8Array(event.data);
        if (bytes.length > 0 && bytes[0] === 0x00) {
          // Metadata frame — first byte 0x00, rest is JSON
          const decoder = new TextDecoder();
          const json = decoder.decode(bytes.slice(1));
          try {
            const meta = JSON.parse(json);
            // Metadata contains cursor position — handled by caller
            options.onMessage(JSON.stringify({ __meta: true, ...meta }));
          } catch {
            // Ignore malformed metadata
          }
        } else {
          const decoder = new TextDecoder();
          options.onMessage(decoder.decode(bytes));
        }
      } else {
        options.onMessage(event.data);
      }
    });

    ws.addEventListener("close", () => {
      options.onClose?.();
    });

    ws.addEventListener("error", (event) => {
      options.onError?.(event);
    });

    return ws;
  };

  const send = (data: string | ArrayBuffer) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(data);
    }
  };

  const close = () => {
    active = false;
    if (ws) {
      ws.close();
      ws = null;
    }
  };

  onCleanup(close);

  return { connect, send, close };
}
