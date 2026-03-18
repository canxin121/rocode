import { type Component, For, Show, createSignal, createEffect, onMount } from "solid-js";
import {
  terminal,
  setActiveTerminal,
  createTerminalSession,
  appendTerminalOutput,
  registerTerminalSocket,
  loadTerminalSessions,
} from "~/stores/terminal";
import styles from "./TerminalDomain.module.css";

export const TerminalDomain: Component = () => {
  const [inputValue, setInputValue] = createSignal("");
  let viewportRef: HTMLDivElement | undefined;

  const activeBuffer = () => terminal.buffers.get(terminal.activeId ?? "") ?? "";

  // Auto-scroll viewport when buffer changes
  createEffect(() => {
    const _ = activeBuffer();
    if (viewportRef) {
      viewportRef.scrollTop = viewportRef.scrollHeight;
    }
  });

  const connectWebSocket = (sessionId: string) => {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${location.host}/pty/${sessionId}/connect?cursor=-1`;
    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";

    ws.addEventListener("message", (event) => {
      if (event.data instanceof ArrayBuffer) {
        const bytes = new Uint8Array(event.data);
        if (bytes.length > 0 && bytes[0] === 0x00) {
          // Metadata frame — skip
          return;
        }
        const decoder = new TextDecoder();
        appendTerminalOutput(sessionId, decoder.decode(bytes));
      } else {
        appendTerminalOutput(sessionId, event.data);
      }
    });

    registerTerminalSocket(sessionId, ws);
  };

  const handleNewTerminal = async () => {
    const session = await createTerminalSession();
    connectWebSocket(session.id);
  };

  const handleInput = (e: Event) => {
    e.preventDefault();
    const value = inputValue().trim();
    if (!value || !terminal.activeId) return;

    const ws = terminal.sockets.get(terminal.activeId);
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(value + "\n");
    }
    setInputValue("");
  };

  onMount(() => {
    // Connect WebSockets for existing sessions
    for (const session of terminal.sessions) {
      if (!terminal.sockets.has(session.id)) {
        connectWebSocket(session.id);
      }
    }
  });

  return (
    <div class={styles.domain}>
      <div class={styles.header}>
        <div class={styles.tabs}>
          <For each={terminal.sessions}>
            {(session) => (
              <button
                class={styles.tab}
                classList={{ [styles.active]: terminal.activeId === session.id }}
                onClick={() => setActiveTerminal(session.id)}
              >
                {session.command || "shell"}
              </button>
            )}
          </For>
        </div>
        <button class={styles.newBtn} onClick={handleNewTerminal}>
          + New
        </button>
      </div>
      <Show
        when={terminal.activeId}
        fallback={
          <div class={styles.empty}>
            No terminal sessions. Click "+ New" to create one.
          </div>
        }
      >
        <div class={styles.viewport} ref={viewportRef}>
          {activeBuffer()}
        </div>
        <form class={styles.inputForm} onSubmit={handleInput}>
          <input
            class={styles.input}
            type="text"
            placeholder="$ "
            value={inputValue()}
            onInput={(e) => setInputValue(e.currentTarget.value)}
          />
        </form>
      </Show>
    </div>
  );
};
