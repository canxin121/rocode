// ── useSSE Hook ────────────────────────────────────────────────────────────
// Manages the global SSE event stream with auto-reconnect.

import { onCleanup } from "solid-js";
import { parseSSE } from "~/api/sse";

export interface UseSSEOptions {
  url: string;
  onEvent: (name: string, payload: unknown) => void;
  reconnectDelay?: number;
}

export function useSSE(options: UseSSEOptions) {
  const { url, onEvent, reconnectDelay = 1500 } = options;
  let active = true;
  let generation = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const start = () => {
    if (!active) return;
    generation++;
    const currentGen = generation;

    void (async () => {
      while (active && currentGen === generation) {
        try {
          const response = await fetch(url, {
            headers: { Accept: "text/event-stream" },
          });

          if (!response.ok) {
            throw new Error(`${response.status} ${response.statusText}`);
          }

          await parseSSE(response, (eventName, eventPayload) => {
            if (active && currentGen === generation) {
              onEvent(eventName, eventPayload);
            }
          });
        } catch (error) {
          console.warn("SSE stream disconnected", error);
        }

        if (!active || currentGen !== generation) break;

        await new Promise<void>((resolve) => {
          reconnectTimer = setTimeout(() => {
            reconnectTimer = null;
            resolve();
          }, reconnectDelay);
        });
      }
    })();
  };

  const stop = () => {
    active = false;
    generation++;
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  onCleanup(stop);

  return { start, stop };
}
