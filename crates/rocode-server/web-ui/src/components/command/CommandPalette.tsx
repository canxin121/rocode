import { type Component, For, Show, createSignal, createMemo, onMount, onCleanup } from "solid-js";
import { state, runtimeStatusLabel, runtimeStatusTone } from "~/stores/app";
import type { UiCommand } from "~/api/types";
import styles from "./CommandPalette.module.css";

export interface CommandPaletteProps {
  onClose: () => void;
  onExecute: (command: string) => void;
}

export const CommandPalette: Component<CommandPaletteProps> = (props) => {
  const [query, setQuery] = createSignal("");
  const [activeIndex, setActiveIndex] = createSignal(0);
  let inputRef: HTMLInputElement | undefined;

  const filtered = createMemo(() => {
    const q = query().toLowerCase().trim();
    const commands = state.uiCommands ?? [];
    if (!q) return commands;
    return commands.filter(
      (cmd) =>
        cmd.name.toLowerCase().includes(q) ||
        cmd.id.toLowerCase().includes(q) ||
        cmd.description?.toLowerCase().includes(q),
    );
  });

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, filtered().length - 1));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const cmd = filtered()[activeIndex()];
      if (cmd) {
        props.onExecute(`/${cmd.id}`);
        props.onClose();
      } else if (query().startsWith("/")) {
        props.onExecute(query());
        props.onClose();
      }
    }
  };

  // Global keyboard shortcut
  const globalKeyHandler = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  onMount(() => {
    inputRef?.focus();
    document.addEventListener("keydown", globalKeyHandler);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", globalKeyHandler);
  });

  return (
    <div class={styles.overlay} onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class={styles.palette}>
        <div class={styles.searchWrap}>
          <input
            ref={inputRef}
            class={styles.searchInput}
            type="text"
            placeholder="Type a command..."
            value={query()}
            onInput={(e) => {
              setQuery(e.currentTarget.value);
              setActiveIndex(0);
            }}
            onKeyDown={handleKeyDown}
          />
        </div>
        <div class={styles.list}>
          <Show
            when={filtered().length > 0}
            fallback={<div class={styles.empty}>No matching commands</div>}
          >
            <For each={filtered()}>
              {(cmd, idx) => (
                <button
                  class={styles.item}
                  classList={{ [styles.active]: activeIndex() === idx() }}
                  onClick={() => {
                    props.onExecute(`/${cmd.id}`);
                    props.onClose();
                  }}
                  onMouseEnter={() => setActiveIndex(idx())}
                >
                  <span class={styles.itemName}>/{cmd.id}</span>
                  <Show when={cmd.description}>
                    <span class={styles.itemDesc}>{cmd.description}</span>
                  </Show>
                </button>
              )}
            </For>
          </Show>
        </div>
        <div class={styles.footer}>
          <span
            class={styles.badge}
            classList={{
              [styles.ok]: runtimeStatusTone() === "ok",
              [styles.warn]: runtimeStatusTone() === "warn",
            }}
          >
            {runtimeStatusLabel()}
          </span>
          <span>Ctrl+K to toggle</span>
        </div>
      </div>
    </div>
  );
};
