import { type Component, For, Show } from "solid-js";
import { state } from "~/stores/app";
import styles from "./DomainTabs.module.css";

export type DomainId = "chat" | "scheduler" | "terminal" | "workspace";

interface DomainTab {
  id: DomainId;
  label: string;
}

const TABS: DomainTab[] = [
  { id: "chat", label: "Chat" },
  { id: "scheduler", label: "Scheduler" },
  { id: "terminal", label: "Terminal" },
  { id: "workspace", label: "Workspace" },
];

export interface DomainTabsProps {
  active: DomainId;
  onSelect: (id: DomainId) => void;
}

export const DomainTabs: Component<DomainTabsProps> = (props) => {
  return (
    <div class={styles.tabs}>
      <For each={TABS}>
        {(tab) => (
          <button
            class={styles.tab}
            classList={{ [styles.active]: props.active === tab.id }}
            onClick={() => props.onSelect(tab.id)}
          >
            {tab.label}
            <Show when={tab.id === "scheduler" && state.executionTopology?.active_count}>
              <span class={styles.badge}>
                {state.executionTopology!.active_count}
              </span>
            </Show>
          </button>
        )}
      </For>
    </div>
  );
};
